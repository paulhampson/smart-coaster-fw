use crate::application::application_state::{ApplicationState, MonitoringStateSubstates};
use crate::application::messaging::{
    ApplicationChannelPublisher, ApplicationData, ApplicationMessage,
};
use crate::hmi::messaging::{UiActionChannelSubscriber, UiActionsMessage};
use crate::weight::WeighingSystem;
use defmt::{debug, trace};
use embassy_futures::select::{select3, Either3};
use embassy_time::{Duration, Instant, Ticker, Timer};
use heapless::HistoryBuffer;
use micromath::F32Ext;

pub struct DrinkMonitoring<WS> {
    weighing_system: WS,
    app_publisher: ApplicationChannelPublisher<'static>,
}

impl<WS> DrinkMonitoring<WS>
where
    WS: WeighingSystem,
{
    const STABILISED_WEIGHT_MAX_DELTA: f32 = 5.0;

    pub fn new(app_publisher: ApplicationChannelPublisher<'static>, weighing_system: WS) -> Self {
        Self {
            app_publisher,
            weighing_system,
        }
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

    async fn send_application_data_update(&mut self, d: ApplicationData) {
        self.app_publisher
            .publish(ApplicationMessage::ApplicationDataUpdate(d))
            .await;
    }

    async fn get_weight_reading_managed_error(&mut self) -> f32 {
        match self.weighing_system.get_reading().await {
            Ok(w) => w,
            Err(_) => self.manage_error("Scale reading failed").await,
        }
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
                    return readings.as_slice().iter().sum::<f32>() / readings.len() as f32;
                }
            }
        }
    }

    async fn update_monitoring_substate(&mut self, state: MonitoringStateSubstates) {
        self.app_publisher
            .publish(ApplicationMessage::ApplicationDataUpdate(
                ApplicationData::MonitoringSubstate(state),
            ))
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

    /// Monitor the weight scale for large deltas. Compare the positives and negatives to estimate
    /// how much fluid has been added and consumed. Report the consumption rate.
    pub async fn run(
        &mut self,
        mut ui_action_receiver: UiActionChannelSubscriber<'_>,
    ) -> ApplicationState {
        const MINIMUM_DELTA_FOR_STATE_CHANGE: f32 = 10.0;
        let mut last_stable_weight = self.get_stabilised_weight().await;
        let mut vessel_placed_weight = last_stable_weight;
        let mut total_consumption = 0.0;
        let monitoring_start_time = Instant::now();
        let mut consumption_update_ticker = Ticker::every(Duration::from_secs(60));

        debug!("Drink monitoring running");

        loop {
            let activity_or_ticker_or_ui_action = select3(
                self.wait_for_weight_activity(),
                consumption_update_ticker.next(),
                ui_action_receiver.next_message_pure(),
            )
            .await;
            match activity_or_ticker_or_ui_action {
                Either3::First(_) => {
                    let new_stable_weight = self.get_stabilised_weight().await;
                    let stable_delta = new_stable_weight - last_stable_weight;

                    if stable_delta > MINIMUM_DELTA_FOR_STATE_CHANGE {
                        self.update_monitoring_substate(MonitoringStateSubstates::VesselPlaced)
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
                        self.update_monitoring_substate(MonitoringStateSubstates::VesselRemoved)
                            .await;
                        trace!("New removed weight {}", new_stable_weight);
                    }
                    last_stable_weight = new_stable_weight;
                }
                Either3::Second(_) => {
                    let consumption_rate = self
                        .update_consumption_rate(monitoring_start_time, total_consumption)
                        .await;
                    debug!("Consumption rate = {} ml/hr", consumption_rate.round());
                }
                Either3::Third(ui_action_message) => match ui_action_message {
                    UiActionsMessage::StateChangeRequest(new_state) => return new_state,
                    _ => {}
                },
            }
        }
    }
}
