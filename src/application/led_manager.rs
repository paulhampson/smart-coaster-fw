use crate::application::application_state::{ApplicationState, CalibrationStateSubstates};
use crate::application::messaging::{
    ApplicationChannelSubscriber, ApplicationData, ApplicationMessage,
};
use crate::led::led_control::{LedArrayMode, LedControl};
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker};
use smart_leds::RGB8;

pub struct LedManager<LC> {
    led_control: LC,
    app_channel: ApplicationChannelSubscriber<'static>,
}

impl<LC> LedManager<LC>
where
    LC: LedControl,
{
    pub fn new(led_control: LC, app_channel: ApplicationChannelSubscriber<'static>) -> Self {
        Self {
            led_control,
            app_channel,
        }
    }

    pub async fn run(&mut self) {
        let mut ticker = Ticker::every(Duration::from_millis(10));
        loop {
            let timer_or_state_change =
                select(ticker.next(), self.app_channel.next_message_pure()).await;
            match timer_or_state_change {
                Either::First(_) => {
                    self.led_control.led_update().await;
                }
                Either::Second(message) => {
                    match message {
                        ApplicationMessage::ApplicationStateUpdate(new_state) => match new_state {
                            ApplicationState::Startup => {
                                self.led_control.set_mode(LedArrayMode::Off);
                            }
                            ApplicationState::TestScreen => {
                                self.led_control.set_mode(LedArrayMode::RainbowWheel {
                                    speed: 2.0,
                                    repetitions: 0.5,
                                })
                            }
                            ApplicationState::ErrorScreenWithMessage(_) => {
                                self.led_control.set_mode(LedArrayMode::Pulse {
                                    colour: RGB8::new(255, 0, 0),
                                    speed: 6.0,
                                })
                            }
                            ApplicationState::Monitoring => {
                                self.led_control.set_mode(LedArrayMode::SingleColourWheel {
                                    colour: RGB8::new(62, 164, 240),
                                    repetitions: 3.0,
                                    speed: 0.8,
                                })
                            }
                            ApplicationState::Settings => {
                                self.led_control.set_mode(LedArrayMode::Pulse {
                                    colour: RGB8::new(4, 4, 74),
                                    speed: 0.5,
                                })
                            }
                            ApplicationState::HeapStatus => {}
                            ApplicationState::Calibration => {}
                            ApplicationState::SetLedBrightness => {
                                self.led_control.set_mode(LedArrayMode::RainbowWheel {
                                    speed: 1.0,
                                    repetitions: 1.0,
                                })
                            }
                        },
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
            }
        }
    }
}
