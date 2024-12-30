use crate::application::application_state::ProductState;
use crate::hmi::event_channels::{HmiEventChannelReceiver, HmiEvents};
use crate::led::led_control::{LedArrayMode, LedControl};
use embassy_futures::select::{select, Either};
use embassy_rp::pio_programs::ws2812::PioWs2812;
use embassy_time::{Duration, Ticker};
use smart_leds::RGB8;

pub async fn led_manager<const LED_COUNT:usize, PIO, const SM:usize>(pio_ws2812: PioWs2812<'_, PIO, SM, LED_COUNT>, mut hmi_event_channel: HmiEventChannelReceiver<'_>)
where
    PIO: embassy_rp::pio::Instance
{
    let mut led_control = LedControl::new(pio_ws2812);

    let mut ticker = Ticker::every(Duration::from_millis(10));
    loop {
        let timer_or_state_change = select(ticker.next(), hmi_event_channel.next_message_pure()).await;
        match timer_or_state_change {
            Either::First(_) => {
                led_control.led_update().await;
            },
            Either::Second(message) => {
                match message {
                    HmiEvents::ChangeProductState(new_state) => {
                        match new_state {
                            ProductState::Startup => { led_control.set_mode(LedArrayMode::Off); }
                            ProductState::TestScreen => led_control.set_mode(LedArrayMode::RainbowWheel { speed: 2.0, repetitions: 0.5 }),
                            ProductState::Tare => led_control.set_mode(LedArrayMode::StaticColour { colour: RGB8::new(255, 0, 0) }),
                            ProductState::Calibration(_) => led_control.set_mode(LedArrayMode::Pulse {
                                colour: RGB8::new(177, 3, 252),
                                speed: 3.0,
                            }),
                            ProductState::CalibrationDone => led_control.set_mode(LedArrayMode::SingleColourWheel {
                                colour: RGB8::new(0, 255, 0),
                                speed: 1.0,
                                repetitions: 2.0,
                            }),
                            ProductState::Wait => {}
                            ProductState::ErrorScreenWithMessage(_) => led_control.set_mode(LedArrayMode::Pulse {
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