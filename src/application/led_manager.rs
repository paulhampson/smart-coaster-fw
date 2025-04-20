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

use crate::application::application_state::{ApplicationState, CalibrationStateSubstates};
use crate::application::messaging::{
    ApplicationChannelSubscriber, ApplicationData, ApplicationMessage,
};
use crate::drink_monitor::messaging::{DrinkMonitorChannelSubscriber, DrinkMonitoringUpdate};
use crate::led::led_control::{LedArrayMode, LedControl};
use defmt::trace;
use embassy_futures::select::{select3, Either3};
use embassy_time::{Duration, Ticker};
use smart_leds::RGB8;

const UPDATES_PER_SECOND: u64 = 30;

const WATER_BLUE: RGB8 = RGB8::new(62, 164, 240);

pub struct LedManager<LC> {
    led_control: LC,
    app_channel: ApplicationChannelSubscriber<'static>,
    drink_monitor_channel: DrinkMonitorChannelSubscriber<'static>,
    application_state: ApplicationState,
    consumption_rate: f32,
    target_rate: f32,
}

impl<LC> LedManager<LC>
where
    LC: LedControl,
{
    pub fn new(
        led_control: LC,
        app_channel: ApplicationChannelSubscriber<'static>,
        drink_monitor_channel: DrinkMonitorChannelSubscriber<'static>,
    ) -> Self {
        Self {
            led_control,
            app_channel,
            drink_monitor_channel,
            application_state: ApplicationState::Startup,
            consumption_rate: 0.0,
            target_rate: 0.0,
        }
    }

    pub async fn run(&mut self) {
        let mut ticker = Ticker::every(Duration::from_millis(1000 / UPDATES_PER_SECOND));
        loop {
            let timer_or_state_change = select3(
                ticker.next(),
                self.app_channel.next_message_pure(),
                self.drink_monitor_channel.next_message_pure(),
            )
            .await;
            match timer_or_state_change {
                Either3::First(_) => {
                    self.led_control.led_update().await;
                }
                Either3::Second(message) => {
                    match message {
                        ApplicationMessage::ApplicationStateUpdate(new_state) => {
                            self.application_state = new_state;
                            match new_state {
                                ApplicationState::Startup => {
                                    self.led_control.set_mode(LedArrayMode::Off);
                                }
                                ApplicationState::TestScreen => {
                                    self.led_control.set_mode(LedArrayMode::RainbowWheel {
                                        speed: 1.0,
                                        repetitions: 1.0,
                                    })
                                }
                                ApplicationState::AboutScreen => {
                                    self.led_control.set_mode(LedArrayMode::RainbowWheel {
                                        speed: 0.5,
                                        repetitions: 1.0,
                                    })
                                }
                                ApplicationState::ErrorScreenWithMessage(_) => {
                                    self.led_control.set_mode(LedArrayMode::Pulse {
                                        colour: RGB8::new(255, 0, 0),
                                        speed: 3.0,
                                    })
                                }
                                ApplicationState::Monitoring => self
                                    .led_control
                                    .set_mode(LedArrayMode::StaticColour { colour: WATER_BLUE }),
                                ApplicationState::Settings => {
                                    self.led_control.set_mode(LedArrayMode::StaticColour {
                                        colour: RGB8::new(191, 64, 191),
                                    })
                                }
                                ApplicationState::HeapStatus => {}
                                ApplicationState::Calibration => {}
                                ApplicationState::SetDateTime => {}
                                ApplicationState::NumberEntry(_) => {}
                            }
                        }

                        ApplicationMessage::ApplicationDataUpdate(app_data) => match app_data {
                            ApplicationData::LedBrightness(brightness) => {
                                self.led_control.set_brightness(brightness).await
                            }
                            ApplicationData::CalibrationSubstate(s) => match s {
                                CalibrationStateSubstates::Tare => {
                                    self.led_control.set_mode(LedArrayMode::StaticColour {
                                        colour: RGB8::new(255, 0, 0),
                                    })
                                }
                                CalibrationStateSubstates::Calibration(_) => {
                                    self.led_control.set_mode(LedArrayMode::Pulse {
                                        colour: RGB8::new(177, 3, 252),
                                        speed: 3.0,
                                    })
                                }
                                CalibrationStateSubstates::CalibrationDone => {
                                    self.led_control.set_mode(LedArrayMode::SingleColourWheel {
                                        colour: RGB8::new(0, 255, 0),
                                        speed: 1.0,
                                        repetitions: 2.0,
                                    })
                                }
                                CalibrationStateSubstates::Wait => {}
                            },
                            _ => {}
                        },
                        _ => {}
                    }
                    self.led_control.led_update().await;
                }
                Either3::Third(drink_monitor_update) => {
                    match drink_monitor_update {
                        DrinkMonitoringUpdate::ConsumptionRate(new_rate) => {
                            self.consumption_rate = new_rate;
                        }
                        DrinkMonitoringUpdate::TargetRate(new_target_rate) => {
                            self.target_rate = new_target_rate;
                        }

                        DrinkMonitoringUpdate::Consumption(_) => {}
                        DrinkMonitoringUpdate::TotalConsumed(_) => {}
                        DrinkMonitoringUpdate::TargetConsumption(_) => {}
                        DrinkMonitoringUpdate::TargetMode(_) => {}
                        DrinkMonitoringUpdate::UpdateMonitoringSubstate(_) => {}
                    }
                    if self.application_state == ApplicationState::Monitoring {
                        self.rate_update(self.consumption_rate, self.target_rate)
                            .await;
                    }
                }
            }
        }
    }

    async fn rate_update(&mut self, consumption_rate: f32, target_rate: f32) {
        let rate_delta = consumption_rate - target_rate;
        trace!("Rate delta = {}", rate_delta);
        if rate_delta >= 0.0 {
            self.led_control.set_mode(LedArrayMode::SingleColourWheel {
                colour: RGB8::new(50, 168, 82), // green
                repetitions: 3.0,
                speed: 0.25,
            })
        } else if rate_delta < -250.0 {
            self.led_control.set_mode(LedArrayMode::SingleColourWheel {
                colour: RGB8::new(227, 54, 54), // red
                repetitions: 3.0,
                speed: 0.75, // faster
            })
        } else if rate_delta < 0.0 {
            self.led_control.set_mode(LedArrayMode::SingleColourWheel {
                colour: RGB8::new(245, 203, 66), // red
                repetitions: 3.0,
                speed: 0.25,
            })
        }
        self.led_control.led_update().await;
    }
}
