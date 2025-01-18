use crate::application::application_state::ApplicationState;
use crate::application::messaging::{
    ApplicationChannelPublisher, ApplicationData, ApplicationMessage,
};
use crate::hmi::messaging::HmiMessage::PushButtonPressed;
use crate::hmi::messaging::{HmiChannelSubscriber, UiActionChannelSubscriber, UiActionsMessage};
use crate::weight::WeighingSystem;
use defmt::{debug, trace};
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Instant, Ticker, Timer};
use heapless::HistoryBuffer;
use micromath::F32Ext;

pub struct ApplicationManager<WS> {
    hmi_subscriber: HmiChannelSubscriber<'static>,
    app_publisher: ApplicationChannelPublisher<'static>,
    ui_action_subscriber: UiActionChannelSubscriber<'static>,
    weighing_system: WS,
}

impl<WS> ApplicationManager<WS>
where
    WS: WeighingSystem,
{
    const LONG_LONG_PRESS_TIME: Duration = Duration::from_secs(2);
    const STABILISED_WEIGHT_MAX_DELTA: f32 = 5.0;

    pub fn new(
        hmi_subscriber: HmiChannelSubscriber<'static>,
        app_publisher: ApplicationChannelPublisher<'static>,
        ui_action_subscriber: UiActionChannelSubscriber<'static>,
        weighing_system: WS,
    ) -> Self {
        Self {
            hmi_subscriber,
            app_publisher,
            ui_action_subscriber,
            weighing_system,
        }
    }

    async fn clear_out_hmi_rx(&mut self) {
        while self.hmi_subscriber.try_next_message_pure().is_some() {}
    }

    async fn manage_error(&mut self, message: &'static str) -> ! {
        self.app_publisher
            .publish(ApplicationMessage::ApplicationStateUpdate(
                ApplicationState::ErrorScreenWithMessage(message),
            ))
            .await;
        loop {
            Timer::after(Duration::from_secs(1)).await;
        }
    }

    async fn update_application_state(&mut self, state: ApplicationState) {
        self.app_publisher
            .publish(ApplicationMessage::ApplicationStateUpdate(state))
            .await;
    }

    async fn wait_for_button(&mut self) {
        while self.hmi_subscriber.next_message_pure().await != PushButtonPressed(true) {}
    }

    async fn get_weight_reading_managed_error(&mut self) -> f32 {
        match self.weighing_system.get_reading().await {
            Ok(w) => w,
            Err(_) => self.manage_error("Scale reading failed").await,
        }
    }

    async fn get_stabilised_weight(&mut self) -> f32 {
        const BUFFER_SIZE: usize = 4;
        let mut readings = HistoryBuffer::<_, BUFFER_SIZE>::new();
        loop {
            readings.write(self.get_weight_reading_managed_error().await);

            if readings.len() == BUFFER_SIZE {
                let min_reading: f32 = *readings
                    .as_slice()
                    .iter()
                    .reduce(|a, b| if a < b { a } else { b })
                    .unwrap();
                let max_reading: f32 = *readings
                    .as_slice()
                    .iter()
                    .reduce(|a, b| if a > b { a } else { b })
                    .unwrap();
                let reading_delta = max_reading - min_reading;
                if reading_delta < Self::STABILISED_WEIGHT_MAX_DELTA {
                    return readings.as_slice().into_iter().sum::<f32>() / readings.len() as f32;
                }
            }
        }
    }

    async fn send_application_data_update(&mut self, d: ApplicationData) {
        self.app_publisher
            .publish(ApplicationMessage::ApplicationDataUpdate(d))
            .await;
    }

    async fn update_consumption_rate(
        &mut self,
        monitoring_start_time: Instant,
        total_consumption: f32,
    ) -> f32 {
        let elapsed_time_in_hours = f32::max(
            monitoring_start_time.elapsed().as_secs() as f32 / 3600.0,
            1.0,
        );
        let consumption_rate = total_consumption / elapsed_time_in_hours;
        self.send_application_data_update(ApplicationData::ConsumptionRate(consumption_rate))
            .await;
        consumption_rate
    }

    pub async fn run(&mut self) {
        match self.weighing_calibration_sequence().await {
            Ok(_) => {
                self.clear_out_hmi_rx().await;
            }
            Err(_) => {
                self.manage_error("Scale calibration failed").await;
            }
        }

        let mut next_state = self.settings_screen().await;
        loop {
            match next_state {
                ApplicationState::WaitingForActivity => {
                    self.coaster_activity_monitoring().await;
                }
                ApplicationState::TestScreen => {
                    next_state = self.test_screen(ApplicationState::Settings).await;
                }
                ApplicationState::Settings => {
                    next_state = self.settings_screen().await;
                }
                _ => {}
            }
        }
    }

    /// Run the weight scale calibration sequence
    async fn weighing_calibration_sequence(&mut self) -> Result<(), WS::Error> {
        self.update_application_state(ApplicationState::Tare).await;
        self.wait_for_button().await;
        self.update_application_state(ApplicationState::Wait).await;
        self.weighing_system.stabilize_measurements().await?;
        self.weighing_system.tare().await?;
        let calibration_mass = 500;
        self.update_application_state(ApplicationState::Calibration(calibration_mass))
            .await;
        self.wait_for_button().await;
        self.update_application_state(ApplicationState::Wait).await;
        self.weighing_system
            .calibrate(calibration_mass as f32)
            .await?;
        self.update_application_state(ApplicationState::CalibrationDone)
            .await;
        Timer::after(Duration::from_secs(2)).await;
        Ok(())
    }

    /// Manage the data and behaviour for the system test screen that shows button, encoder and
    /// weight. Long press will exit the state.
    async fn test_screen(&mut self, exit_to: ApplicationState) -> ApplicationState {
        self.update_application_state(ApplicationState::TestScreen)
            .await;
        let mut press_start = Instant::now();
        loop {
            let hmi_or_weight_message = select(
                self.hmi_subscriber.next_message_pure(),
                self.weighing_system.get_reading(),
            )
            .await;

            match hmi_or_weight_message {
                Either::First(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;
                    match hmi_message {
                        PushButtonPressed(true) => {
                            press_start = Instant::now();
                        }
                        PushButtonPressed(false) => {
                            if press_start.elapsed() >= Self::LONG_LONG_PRESS_TIME {
                                debug!("LONG_LONG_PRESS_TIME exceeded");
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                Either::Second(weight_reading) => {
                    match weight_reading {
                        Ok(w) => {
                            self.app_publisher
                                .publish(ApplicationMessage::ApplicationDataUpdate(
                                    ApplicationData::Weight(w),
                                ))
                                .await;
                        } // just send on weight for now so it updates on screen
                        Err(_) => self.manage_error("Scale reading failed").await,
                    }
                }
            }
        }
        exit_to
    }

    async fn settings_screen(&mut self) -> ApplicationState {
        self.update_application_state(ApplicationState::Settings)
            .await;
        let mut press_start = Instant::now();

        loop {
            let ui_or_hmi = select(
                self.ui_action_subscriber.next_message_pure(),
                self.hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => match ui_action_message {
                    UiActionsMessage::StateChangeRequest(new_state) => {
                        return new_state;
                    }
                },
                Either::Second(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;

                    match hmi_message {
                        PushButtonPressed(true) => {
                            press_start = Instant::now();
                        }
                        PushButtonPressed(false) => {
                            if press_start.elapsed() >= Self::LONG_LONG_PRESS_TIME {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        ApplicationState::WaitingForActivity
    }

    async fn wait_for_weight_activity(&mut self) -> f32 {
        const MINIMUM_DELTA_FOR_ACTIVITY: f32 = 10.0;
        let mut last_weight = self.get_weight_reading_managed_error().await;
        loop {
            let current_weight = self.get_weight_reading_managed_error().await;
            let weight_delta = current_weight - last_weight;
            last_weight = current_weight;
            if weight_delta.abs() > MINIMUM_DELTA_FOR_ACTIVITY {
                return weight_delta;
            }
        }
    }

    /// Monitor the weight scale for large deltas. Compare the positives and negatives to estimate
    /// how much fluid has been added and consumed. Report the consumption rate.
    async fn coaster_activity_monitoring(&mut self) {
        const MINIMUM_DELTA_FOR_STATE_CHANGE: f32 = 10.0;
        let mut last_stable_weight = self.get_stabilised_weight().await;
        let mut vessel_placed_weight = last_stable_weight;
        let mut total_consumption = 0.0;
        let monitoring_start_time = Instant::now();
        let mut consumption_update_ticker = Ticker::every(Duration::from_secs(60));

        self.update_application_state(ApplicationState::WaitingForActivity)
            .await;

        loop {
            let activity_or_ticker = select(
                self.wait_for_weight_activity(),
                consumption_update_ticker.next(),
            )
            .await;
            match activity_or_ticker {
                Either::First(_) => {
                    let new_stable_weight = self.get_stabilised_weight().await;
                    let stable_delta = new_stable_weight - last_stable_weight;

                    if stable_delta > MINIMUM_DELTA_FOR_STATE_CHANGE {
                        self.update_application_state(ApplicationState::VesselPlaced)
                            .await;
                        let consumption = vessel_placed_weight - new_stable_weight;
                        if consumption > 0.0 {
                            total_consumption += consumption;
                        }
                        let consumption_rate = self
                            .update_consumption_rate(monitoring_start_time, total_consumption)
                            .await;

                        vessel_placed_weight = new_stable_weight;
                        trace!("New placed weight {}", vessel_placed_weight);
                        debug!("Consumption = {} ml", consumption);
                        debug!("Total consumption = {} ml", total_consumption);
                        debug!("Consumption rate = {} ml/hr", consumption_rate.round());
                        self.send_application_data_update(ApplicationData::Consumption(f32::max(
                            0.0,
                            consumption,
                        )))
                        .await;
                        self.send_application_data_update(ApplicationData::TotalConsumed(
                            f32::max(0.0, total_consumption),
                        ))
                        .await;
                    } else if stable_delta < -MINIMUM_DELTA_FOR_STATE_CHANGE {
                        self.update_application_state(ApplicationState::VesselRemoved)
                            .await;
                        trace!("New removed weight {}", new_stable_weight);
                    }
                    last_stable_weight = new_stable_weight;
                }
                Either::Second(_) => {
                    let consumption_rate = self
                        .update_consumption_rate(monitoring_start_time, total_consumption)
                        .await;
                    debug!("Consumption rate = {} ml/hr", consumption_rate.round());
                }
            }
        }
    }
}
