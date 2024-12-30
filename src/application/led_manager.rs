use crate::application::application_state::ApplicationState;
use crate::application::messaging::{ApplicationChannelSubscriber, ApplicationMessage};
use crate::led::led_control::{LedArrayMode, LedControl};
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker};
use smart_leds::RGB8;

pub async fn led_manager<LC>(mut led_control: LC, mut app_channel: ApplicationChannelSubscriber<'_>)
where
    LC: LedControl
{
    let mut ticker = Ticker::every(Duration::from_millis(10));
    loop {
        let timer_or_state_change = select(ticker.next(), app_channel.next_message_pure()).await;
        match timer_or_state_change {
            Either::First(_) => {
                led_control.led_update().await;
            },
            Either::Second(message) => {
                match message {
                    ApplicationMessage::ApplicationStateUpdate(new_state) => {
                        match new_state {
                            ApplicationState::Startup => { led_control.set_mode(LedArrayMode::Off); }
                            ApplicationState::TestScreen => led_control.set_mode(LedArrayMode::RainbowWheel { speed: 2.0, repetitions: 0.5 }),
                            ApplicationState::Tare => led_control.set_mode(LedArrayMode::StaticColour { colour: RGB8::new(255, 0, 0) }),
                            ApplicationState::Calibration(_) => led_control.set_mode(LedArrayMode::Pulse {
                                colour: RGB8::new(177, 3, 252),
                                speed: 3.0,
                            }),
                            ApplicationState::CalibrationDone => led_control.set_mode(LedArrayMode::SingleColourWheel {
                                colour: RGB8::new(0, 255, 0),
                                speed: 1.0,
                                repetitions: 2.0,
                            }),
                            ApplicationState::Wait => {}
                            ApplicationState::ErrorScreenWithMessage(_) => led_control.set_mode(LedArrayMode::Pulse {
                                colour: RGB8::new(255, 0, 0),
                                speed: 6.0,
                            }),
                        }
                    }
                    _ => {}
                }
                led_control.led_update().await;
            }
        }
    }
}