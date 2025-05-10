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
    ApplicationState, CalibrationStateSubstates, ConfirmationId,
};
use crate::application::messaging::ApplicationData::MonitoringUpdate;
use crate::application::messaging::{
    ApplicationChannelPublisher, ApplicationData, ApplicationMessage,
};
use crate::drink_monitor::messaging::DrinkMonitorChannelSubscriber;
use crate::hmi::messaging::HmiMessage::PushButtonPressed;
use crate::hmi::messaging::{HmiChannelSubscriber, UiActionChannelSubscriber, UiRequestMessage};
use crate::storage::settings::SettingsAccessorId;
use crate::weight::WeighingSystem;
use crate::Heap;
use defmt::debug;
use embassy_futures::select::{select, select3, Either, Either3};
use embassy_time::{Duration, Instant, Timer};

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

    async fn send_application_data_update(&mut self, d: ApplicationData) {
        self.app_publisher
            .publish(ApplicationMessage::ApplicationDataUpdate(d))
            .await;
    }

    pub async fn run(
        &mut self,
        mut ui_action_receiver: UiActionChannelSubscriber<'_>,
        mut hmi_subscriber: HmiChannelSubscriber<'_>,
        mut drink_monitor_receiver: DrinkMonitorChannelSubscriber<'_>,
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
                        .coaster_activity_monitoring(
                            &mut ui_action_receiver,
                            &mut hmi_subscriber,
                            &mut drink_monitor_receiver,
                        )
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
                ApplicationState::TimeEntry(setting_id) => {
                    next_state = self
                        .time_entry_screen(&mut ui_action_receiver, &mut hmi_subscriber, setting_id)
                        .await;
                }
                ApplicationState::DateTimeEntry(_) => {
                    next_state = self
                        .set_date_time_screen(
                            &mut ui_action_receiver,
                            &mut hmi_subscriber,
                            next_state,
                        )
                        .await;
                }
                ApplicationState::ConfirmationScreen(confirmation_id) => {
                    next_state = self
                        .confirmation_screen(
                            &mut ui_action_receiver,
                            &mut hmi_subscriber,
                            confirmation_id,
                        )
                        .await
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
                ApplicationState::SetSystemDateTime => {
                    next_state = self
                        .set_date_time_screen(
                            &mut ui_action_receiver,
                            &mut hmi_subscriber,
                            next_state,
                        )
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

    async fn coaster_activity_monitoring(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
        drink_monitor_subscriber: &mut DrinkMonitorChannelSubscriber<'_>,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::Monitoring)
            .await;

        loop {
            let ui_or_hmi = select3(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
                drink_monitor_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either3::First(ui_action_message) => {
                    if let UiRequestMessage::ChangeState(new_state) = ui_action_message {
                        return new_state;
                    }
                }
                Either3::Second(hmi_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::HmiInput(hmi_message))
                        .await;
                }
                Either3::Third(drink_monitor_message) => {
                    self.app_publisher
                        .publish(ApplicationMessage::ApplicationDataUpdate(MonitoringUpdate(
                            drink_monitor_message,
                        )))
                        .await;
                }
            }
        }
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
                    UiRequestMessage::ChangeState(new_state) => {
                        return new_state;
                    }
                    UiRequestMessage::ChangeLedBrightness(new_brightness) => {
                        self.app_publisher
                            .publish(ApplicationMessage::ApplicationDataUpdate(
                                ApplicationData::LedBrightness(new_brightness),
                            ))
                            .await
                    }
                    UiRequestMessage::ChangeDisplayBrightness(new_brightness) => {
                        self.app_publisher
                            .publish(ApplicationMessage::ApplicationDataUpdate(
                                ApplicationData::DisplayBrightness(new_brightness),
                            ))
                            .await
                    }
                    UiRequestMessage::ChangeDisplayTimeout(new_timeout) => {
                        self.app_publisher
                            .publish(ApplicationMessage::ApplicationDataUpdate(
                                ApplicationData::DisplayTimeout(new_timeout),
                            ))
                            .await
                    }
                    UiRequestMessage::ClearHistoricalConsumptionLog() => {}
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
        enter_state: ApplicationState,
    ) -> ApplicationState {
        self.update_application_state(enter_state).await;
        loop {
            let ui_or_hmi = select(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => {
                    if let UiRequestMessage::ChangeState(new_state) = ui_action_message {
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
                    if let UiRequestMessage::ChangeState(new_state) = ui_action_message {
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
                    UiRequestMessage::ChangeState(new_state) => {
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

    async fn time_entry_screen(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
        setting_id: SettingsAccessorId,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::TimeEntry(setting_id))
            .await;
        loop {
            let ui_or_hmi = select(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => {
                    if let UiRequestMessage::ChangeState(new_state) = ui_action_message {
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
                    if let UiRequestMessage::ChangeState(new_state) = ui_action_message {
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

    async fn confirmation_screen(
        &mut self,
        ui_action_subscriber: &mut UiActionChannelSubscriber<'_>,
        hmi_subscriber: &mut HmiChannelSubscriber<'_>,
        confirmation_id: ConfirmationId,
    ) -> ApplicationState {
        self.update_application_state(ApplicationState::ConfirmationScreen(confirmation_id))
            .await;
        loop {
            let ui_or_hmi = select(
                ui_action_subscriber.next_message_pure(),
                hmi_subscriber.next_message_pure(),
            )
            .await;

            match ui_or_hmi {
                Either::First(ui_action_message) => {
                    if let UiRequestMessage::ClearHistoricalConsumptionLog() = ui_action_message {
                        self.app_publisher
                            .publish(ApplicationMessage::ClearHistoricalConsumptionLog)
                            .await
                    }
                    if let UiRequestMessage::ChangeState(new_state) = ui_action_message {
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
}
