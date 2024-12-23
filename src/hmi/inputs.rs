use embassy_futures::select::{select, Either};
use embassy_rp::gpio::Level;
use crate::hmi::debouncer::Debouncer;
use crate::hmi::rotary_encoder::RotaryEncoder;
use crate::hmi::event_channels::HmiEventChannelSender;
use crate::hmi::event_channels::HmiEvents;

pub async fn hmi_input_handler(hmi_event_channel: HmiEventChannelSender, mut debounced_btn: Debouncer<'_>, mut rotary_encoder: RotaryEncoder<'_>) {
    loop {
        let hmi_io_event = select(rotary_encoder.state_change(), debounced_btn.debounce()).await;

        match hmi_io_event {
            Either::First(encoder_moved) => hmi_event_channel.send(HmiEvents::EncoderUpdate(encoder_moved)).await,
            Either::Second(button_level) => hmi_event_channel.send(HmiEvents::PushButtonPressed(button_level == Level::High)).await
        }
    }
}