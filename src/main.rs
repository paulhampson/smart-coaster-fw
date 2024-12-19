#![no_std]
#![no_main]

mod rotary_encoder;
mod debouncer;

use core::fmt::Write;
use heapless::String;

use embassy_executor::Spawner;
use embassy_time::Duration;
#[allow(unused_imports)]
use {defmt_rtt as _, panic_probe as _};

use assign_resources::assign_resources;
use embassy_futures::select::{select, Either};
use embassy_rp as hal;
use embassy_rp::gpio::{Input, Level, Pull};
use embassy_rp::peripherals;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use hal::i2c::{self, Config};

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

use crate::debouncer::Debouncer;
use crate::rotary_encoder::{Direction, RotaryEncoder};
use sh1106::{prelude::*, Builder};

enum HmiEvents {
    EncoderUpdate(Direction),
    PushButtonPressed(bool)
}
type HmiEventChannel = Channel<CriticalSectionRawMutex, HmiEvents, 5>;
type HmiEventChannelReceiver = Receiver<'static, CriticalSectionRawMutex, HmiEvents, 5>;
type HmiEventChannelSender = Sender<'static, CriticalSectionRawMutex, HmiEvents, 5>;

assign_resources! {
    display_i2c: DisplayI2cPins{
        sda_pin: PIN_4,
        scl_pin: PIN_5,
        i2c_peripheral: I2C0
    },
    hmi_inputs: HmiInputPins {
        rotary_dt_pin: PIN_7,
        rotary_clk_pin: PIN_8,
        push_btn_pin: PIN_6,
    },
}

static HMI_EVENT_CHANNEL: HmiEventChannel = Channel::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let resources = split_resources!{p};

    spawner.spawn(hmi_input_task(resources.hmi_inputs, HMI_EVENT_CHANNEL.sender())).unwrap();
    spawner.spawn(display_task(resources.display_i2c, HMI_EVENT_CHANNEL.receiver())).unwrap();

}


#[embassy_executor::task]
async fn hmi_input_task(hmi_input_pins: HmiInputPins, hmi_event_channel: HmiEventChannelSender)
{
    let rotary_dt = hmi_input_pins.rotary_dt_pin;
    let rotary_clk = hmi_input_pins.rotary_clk_pin;
    let mut debounced_btn = Debouncer::new(Input::new(hmi_input_pins.push_btn_pin, Pull::Up), Duration::from_millis(20));

    let mut rotary_encoder = RotaryEncoder::new(
        Input::new(rotary_dt, Pull::Up),
        Input::new(rotary_clk, Pull::Up)
    );

    loop {
        let hmi_io_event = select(rotary_encoder.state_change(), debounced_btn.debounce()).await;

        match hmi_io_event {
            Either::First(encoder_moved) => hmi_event_channel.send(HmiEvents::EncoderUpdate(encoder_moved)).await,
            Either::Second(button_level) => hmi_event_channel.send(HmiEvents::PushButtonPressed(button_level == Level::High)).await
        }
    }

}

#[embassy_executor::task]
async fn display_task(display_i2c_pins: DisplayI2cPins, hmi_event_channel: HmiEventChannelReceiver)
{
    let i2c = i2c::I2c::new_blocking(display_i2c_pins.i2c_peripheral,
                                     display_i2c_pins.scl_pin, display_i2c_pins.sda_pin,
                                     Config::default());
    let mut display: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();
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
        }

        display.clear();

        count_string.clear();
        write!(&mut count_string, "CW Count = {}", cw_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 0), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        count_string.clear();
        write!(&mut count_string, "CCW Count = {}", ccw_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 16), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        count_string.clear();
        write!(&mut count_string, "Press Count = {}", btn_press_count).unwrap();
        Text::with_baseline(count_string.as_str(), Point::new(0, 32), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        display.flush().unwrap();
    }
}