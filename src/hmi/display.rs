use core::fmt::Write;
use embassy_rp::i2c::{Async, I2c};
use embassy_rp::peripherals::I2C0;
use embassy_time::Instant;
use embedded_graphics::Drawable;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::{Baseline, Text};
use heapless::String;
use sh1106::interface::I2cInterface;
use sh1106::mode::GraphicsMode;
use crate::{HmiEventChannelReceiver};
use crate::hmi::event_channels::HmiEvents;
use crate::hmi::rotary_encoder::Direction;

const FRAME_TIMING_MS:u64 = 1000 / 30;

pub async fn display_update_handler(hmi_event_channel: HmiEventChannelReceiver, display: &mut GraphicsMode<I2cInterface<I2c<'_, I2C0, Async>>>) {
    display.init().unwrap();
    display.flush().unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    let mut cw_count = 0;
    let mut ccw_count = 0;
    let mut btn_press_count = 0;
    let mut count_string = String::<32>::new();
    let mut last_weight = 0u32;

    loop {

        display.clear();

        count_string.clear();
        write!(&mut count_string, "CW Count = {}", cw_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 0), text_style, Baseline::Top)
            .draw(display)
            .unwrap();

        count_string.clear();
        write!(&mut count_string, "CCW Count = {}", ccw_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 16), text_style, Baseline::Top)
            .draw(display)
            .unwrap();

        count_string.clear();
        write!(&mut count_string, "Press Count = {}", btn_press_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 32), text_style, Baseline::Top)
            .draw(display)
            .unwrap();

        count_string.clear();
        write!(&mut count_string, "Weight = {}", last_weight).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 48), text_style, Baseline::Top)
            .draw(display)
            .unwrap();

        display.flush().unwrap();
        let last_update = Instant::now();

        loop {
            let event = hmi_event_channel.receive().await;
            match event {
                HmiEvents::EncoderUpdate(direction) => {
                    if direction == Direction::Clockwise {
                        cw_count += 1;
                    }
                    if direction == Direction::CounterClockwise {
                        ccw_count += 1;
                    }
                }
                HmiEvents::PushButtonPressed(is_pressed) => {
                    if is_pressed {
                        btn_press_count += 1;
                    }
                }
                HmiEvents::WeightUpdate(weight) => {
                    last_weight = weight;
                }
            }

            if last_update.elapsed().as_millis() > FRAME_TIMING_MS {
                break;
            }
        }
    }
}