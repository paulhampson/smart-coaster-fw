// Copyright (C) 2025 Paul Hampson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License version 3 as  published by the
// Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::application::application_state::{
    ApplicationState, CalibrationStateSubstates, MonitoringStateSubstates,
};
use crate::application::messaging::{
    ApplicationChannelPublisher, ApplicationData, ApplicationMessage,
};
use crate::application::storage::settings::SettingsAccessorId;
use crate::hmi::messaging::HmiMessage::PushButtonPressed;
use crate::hmi::messaging::{HmiChannelSubscriber, UiActionChannelSubscriber, UiActionsMessage};
use crate::weight::WeighingSystem;
use crate::Heap;
use defmt::{debug, trace};
use embassy_futures::select::{select, select4, Either, Either4};
use embassy_time::{Duration, Instant, Ticker, Timer};
use heapless::HistoryBuffer;
use micromath::F32Ext;

pub struct ApplicationManager<WS> {
    app_publisher: ApplicationChannelPublisher<'static>,
    weighing_system: WS,
    heap: &'static Heap,
}

impl<WS> ApplicationManager<WS>
where
    WS: WeighingSystem,
{
    const LONG_LONG_PRESS_TIME: Duration = Duration::from_secs(2);
    const STABILISED_WEIGHT_MAX_DELTA: f32 = 5.0;

    pub fn new(
        app_publisher: ApplicationChannelPublisher<'static>,
        weighing_system: WS,
        heap: &'static Heap,
    ) -> Self {
        Self {
            app_publisher,
            weighing_system,
            heap,
        }
    }

    async fn clear_out_hmi_rx(&mut self, hmi_subscriber: &mut HmiChannelSubscriber<'_>) {
        while hmi_subscriber.try_next_message_pure().is_some() {}
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

    async fn update_monitoring_substate(&mut self, state: MonitoringStateSubstates) {
        self.app_publisher
            .publish(ApplicationMessage::ApplicationDataUpdate(
                ApplicationData::MonitoringSubstate(state),
            ))
            .await;
    }

    async fn update_calibration_substate(&mut self, state: CalibrationStateSubstates) {
        self.app_publisher
            .publish(ApplicationMessage::ApplicationDataUpdate(
                ApplicationData::CalibrationSubstate(state),
            ))
            .await;
    }

    async fn wait_for_button(&mut self, hmi_subscriber: &mut HmiChannelSubscriber<'_>) {
        while hmi_subscriber.next_message_pure().await != PushButtonPressed(true) {}
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
                    return readings.as_slice().iter().sum::<f32>() / readings.len() as f32;
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

    pub async fn run(
        &mut self,
        mut ui_action_receiver: UiActionChannelSubscriber<'_>,
        mut hmi_subscriber: HmiChannelSubscriber<'_>,
    ) {
        self.update_application_state(ApplicationState::Startup)
            .await;

        self.clear_out_hmi_rx(&mut hmi_subscriber).await;

        let mut next_state = ApplicationState::Monitoring;
        loop {
            match next_state {
                ApplicationState::Startup | ApplicationState::ErrorScreenWithMessage(_) => {}
                ApplicationState::Monitoring => {
                    next_state = self
                        .coaster_activity_monitoring(&mut ui_action_receiver, &mut hmi_subscriber)
                        .await;
                }
                ApplicationState::TestScreen => {
                    next_state = self
                        .test_screen(ApplicationState::Settings, &mut hmi_subscriber)
                        .await;
                }
                ApplicationState::Settings => {
                    next_state = self
                        .settings_screen(&mut ui_action_receiver, &mut hmi_subscriber)
                        .await;
                }
                ApplicationState::NumberEntry(setting_id) => {
                    next_state = self
                        .number_entry_screen(
                            &mut ui_action_receiver,
                            &mut hmi_subscriber,
                            setting_id,
                        )
                        .await;
                }
                ApplicationState::HeapStatus => {
                    next_state = self
                        .heap_status_screen(&mut ui_action_receiver, &mut hmi_subscriber)
                        .await
                }
                ApplicationState::Calibration => {
                    match self
                        .weighing_calibration_sequence(&mut hmi_subscriber)
                        .await
                    {
                        Ok(_) => {
                            self.clear_out_hmi_rx(&mut hmi_subscriber).await;
                        }
                        Err(_) => {
                            self.manage_error("Scale calibration failed").await;
                        }
                    }
                    next_state = ApplicationState::Settings;
                }
                ApplicationState::SetDateTime => {
                    next_state = self
                        .set_date_time_screen(&mut ui_action_receiver, &mut hmi_subscriber)
                        .await;
                }
                ApplicationState::AboutScreen => {
                    next_state = self
                        .about_screen(&mut ui_action_receiver, &mut hmi_subscriber)
                        .await;
                }
            }
            debug!("Changing to next_state: {:?}", next_state);
        }
    }

    /// Run the weight scale calibration sequence
    async fn weighing_calibration_sequence(
        &mut self,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
    ) -> Result<(), WS::Error> {
        self.update_application_state(ApplicationState::Calibration)
            .await;
        self.update_calibration_substate(CalibrationStateSubstates::Tare)
            .await;
        self.wait_for_button(hmi_subscriber).await;
        self.update_calibration_substate(CalibrationStateSubstates::Wait)
            .await;
        self.weighing_system.stabilize_measurements().await?;
        self.weighing_system.tare().await?;
        let calibration_mass = 500;
        self.update_calibration_substate(CalibrationStateSubstates::Calibration(calibration_mass))
            .await;
        self.wait_for_button(hmi_subscriber).await;
        self.update_calibration_substate(CalibrationStateSubstates::Wait)
            .await;
        self.weighing_system
            .calibrate(calibration_mass as f32)
            .await?;
        self.update_calibration_substate(CalibrationStateSubstates::CalibrationDone)
            .await;
        Timer::after(Duration::from_secs(2)).await;
        Ok(())
    }

    /// Manage the data and behaviour for the system test screen that shows button, encoder and
    /// weight. Long press will exit the state.
    async fn test_screen(
        &mut self,
        exit_to: ApplicationState,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::TestScreen)
            .await;
        let mut press_start = Instant::now();
        loop {
            let hmi_or_weight_message = select(
                hmi_subscriber.next_message_pure(),
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

    async fn settings_screen(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::Settings)
            .await;

        loop {
            let ui_or_hmi = select(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => match ui_action_message {
                    UiActionsMessage::StateChangeRequest(new_state) => {
                        return new_state;
                    }
                    UiActionsMessage::LedBrightnessChangeRequest(new_brightness) => {
                        self.app_publisher
                            .publish(ApplicationMessage::ApplicationDataUpdate(
                                ApplicationData::LedBrightness(new_brightness),
                            ))
                            .await
                    }
                    UiActionsMessage::DisplayBrightnessChangeRequest(new_brightness) => {
                        self.app_publisher
                            .publish(ApplicationMessage::ApplicationDataUpdate(
                                ApplicationData::DisplayBrightness(new_brightness),
                            ))
                            .await
                    }
                    UiActionsMessage::MonitoringModeChangeRequest(_period_option) => {
                        // TODO use the period option - pass it to the monitoring functionality
                    }
                    UiActionsMessage::DisplayTimeoutChangeRequest(new_timeout) => {
                        self.app_publisher
                            .publish(ApplicationMessage::ApplicationDataUpdate(
                                ApplicationData::DisplayTimeout(new_timeout),
                            ))
                            .await
                    }
                },
                Either::Second(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;
                }
            }
        }
    }

    async fn set_date_time_screen(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::SetDateTime)
            .await;
        loop {
            let ui_or_hmi = select(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => {
                    if let UiActionsMessage::StateChangeRequest(new_state) = ui_action_message {
                        return new_state;
                    }
                }
                Either::Second(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;
                }
            }
        }
    }

    async fn about_screen(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::AboutScreen)
            .await;
        loop {
            let ui_or_hmi = select(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => {
                    if let UiActionsMessage::StateChangeRequest(new_state) = ui_action_message {
                        return new_state;
                    }
                }
                Either::Second(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;
                }
            }
        }
    }

    async fn number_entry_screen(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
        setting_id: SettingsAccessorId,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::NumberEntry(setting_id))
            .await;
        loop {
            let ui_or_hmi = select(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => {
                    if let UiActionsMessage::StateChangeRequest(new_state) = ui_action_message {
                        return new_state;
                    }
                }
                Either::Second(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;
                }
            }
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

    /// Monitor the weight scale for large deltas. Compare the positives and negatives to estimate
    /// how much fluid has been added and consumed. Report the consumption rate.
    async fn coaster_activity_monitoring(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
    ) -> ApplicationState {
        const MINIMUM_DELTA_FOR_STATE_CHANGE: f32 = 10.0;
        let mut last_stable_weight = self.get_stabilised_weight().await;
        let mut vessel_placed_weight = last_stable_weight;
        let mut total_consumption = 0.0;
        let monitoring_start_time = Instant::now();
        let mut consumption_update_ticker = Ticker::every(Duration::from_secs(60));

        self.update_application_state(ApplicationState::Monitoring)
            .await;

        loop {
            let activity_or_ticker_or_ui_action = select4(
                self.wait_for_weight_activity(),
                consumption_update_ticker.next(),
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;
            match activity_or_ticker_or_ui_action {
                Either4::First(_) => {
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
                Either4::Second(_) => {
                    let consumption_rate = self
                        .update_consumption_rate(monitoring_start_time, total_consumption)
                        .await;
                    debug!("Consumption rate = {} ml/hr", consumption_rate.round());
                }
                Either4::Third(ui_action_message) => match ui_action_message {
                    UiActionsMessage::StateChangeRequest(new_state) => return new_state,
                    _ => {}
                },
                Either4::Fourth(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;
                }
            }
        }
    }

    async fn heap_status_screen(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::HeapStatus)
            .await;

        self.send_application_data_update(ApplicationData::HeapStatus {
            used: self.heap.used(),
            free: self.heap.free(),
        })
        .await;

        loop {
            let ui_or_hmi = select(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => match ui_action_message {
                    UiActionsMessage::StateChangeRequest(new_state) => {
                        return new_state;
                    }
                    _ => {}
                },
                Either::Second(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;
                }
            }
        }
    }
}
