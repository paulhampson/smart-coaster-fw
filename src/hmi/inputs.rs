use crate::hmi::debouncer::Debouncer;
use crate::hmi::messaging::HmiChannelPublisher;
use crate::hmi::messaging::HmiMessage;
use crate::hmi::rotary_encoder::RotaryEncoder;
use embassy_futures::select::{select, Either};
use embassy_rp::gpio::Level;

const PRESSED_LEVEL: Level = Level::Low;

pub async fn hmi_input_handler(hmi_event_channel: HmiChannelPublisher<'_>, mut debounced_btn: Debouncer<'_>, rotary_encoder: &mut impl RotaryEncoder) {
    let mut next_btn_level = PRESSED_LEVEL;
    loop {
        let hmi_io_event = select(rotary_encoder.state_change(), debounced_btn.wait_for_change_to(next_btn_level)).await;

        match hmi_io_event {
            Either::First(encoder_moved) => hmi_event_channel.publish_immediate(HmiMessage::EncoderUpdate(encoder_moved)),
            Either::Second(_) => {
                hmi_event_channel.publish_immediate(HmiMessage::PushButtonPressed(next_btn_level == PRESSED_LEVEL));
                match next_btn_level {
                    Level::High => {next_btn_level = Level::Low},
                    Level::Low => {next_btn_level = Level::High}
                }
            },
        }
    }
}