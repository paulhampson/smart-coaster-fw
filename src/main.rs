#![no_std]
#![no_main]

mod led;
mod hmi;
mod weight;
mod application;

use embassy_executor::{Executor, Spawner};
use embassy_time::{Duration, Timer};
#[allow(unused_imports)]
use {defmt_rtt as _, panic_probe as _};

use crate::hmi::event_channels::{HmiEventChannel, HmiEventChannelReceiver, HmiEventChannelSender, HmiEvents};
use crate::hmi::inputs::hmi_input_handler;
use crate::hmi::rotary_encoder::DebouncedRotaryEncoder;
use crate::weight::interface::hx711async::{Hx711Async, Hx711Gain};
use assign_resources::assign_resources;
use defmt::warn;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{self, Config};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::{I2C0, PIO0};
use embassy_rp::pio::Pio;
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::{bind_interrupts, peripherals, pio};
use embassy_sync::pubsub::PubSubChannel;
use hmi::debouncer::Debouncer;
use hmi::display::display_update_handler;
use sh1106::{prelude::*, Builder};

use crate::application::application_state::ProductState;
use crate::hmi::event_channels::HmiEvents::PushButtonPressed;
use crate::led::led_control::led_update_handler;
use crate::weight::weight::WeightScale;
use static_cell::StaticCell;

static HMI_EVENT_CHANNEL: HmiEventChannel = PubSubChannel::new(); // Channel::new();

const LED_COUNT: usize = 8;

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
    led_control: LedControlResources {
        pio: PIO0,
        dma_channel: DMA_CH0,
        data_pin: PIN_16,
    }
    strain_gauge_io: StrainGaugeResources {
        clk_pin: PIN_14,
        data_pin: PIN_15,
    }
}

struct Core0Resources {
    hmi_inputs: HmiInputPins,
    led_control: LedControlResources,
    strain_gauge_io: StrainGaugeResources,
}

struct Core1Resources {
    display_i2c: DisplayI2cPins,
}

bind_interrupts!(struct PioIrqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

bind_interrupts!(struct I2cIrqs {
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
});

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    let resources = split_resources!{p};

    let core0_resources = Core0Resources { hmi_inputs: resources.hmi_inputs, led_control: resources.led_control, strain_gauge_io: resources.strain_gauge_io };
    let core1_resources = Core1Resources { display_i2c: resources.display_i2c };

    warn!("Launching application across cores");

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| core1_main(spawner, core1_resources));
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| core0_main(spawner, core0_resources));
}

fn core0_main(spawner: Spawner, resources: Core0Resources)
{
    spawner.spawn(hmi_input_task(resources.hmi_inputs, HMI_EVENT_CHANNEL.publisher().unwrap())).unwrap();
    spawner.spawn(led_task(resources.led_control, HMI_EVENT_CHANNEL.subscriber().unwrap())).unwrap();
    spawner.spawn(weighing_task(resources.strain_gauge_io, HMI_EVENT_CHANNEL.publisher().unwrap(), HMI_EVENT_CHANNEL.subscriber().unwrap())).unwrap()
}

fn core1_main(spawner: Spawner, resources: Core1Resources)
{
    spawner.spawn(display_task(resources.display_i2c, HMI_EVENT_CHANNEL.subscriber().unwrap())).unwrap();
}

#[embassy_executor::task]
async fn hmi_input_task(hmi_input_pins: HmiInputPins, hmi_event_channel: HmiEventChannelSender<'static>)
{
    let rotary_dt = hmi_input_pins.rotary_dt_pin;
    let rotary_clk = hmi_input_pins.rotary_clk_pin;
    let debounced_btn = Debouncer::new(Input::new(hmi_input_pins.push_btn_pin, Pull::Up), Duration::from_millis(20));

    let mut rotary_encoder = DebouncedRotaryEncoder::new(
        Input::new(rotary_dt, Pull::Up),
        Input::new(rotary_clk, Pull::Up),
        Duration::from_millis(6),
    );

    hmi_input_handler(hmi_event_channel, debounced_btn, &mut rotary_encoder).await;

}

#[embassy_executor::task]
async fn display_task(display_i2c_pins: DisplayI2cPins, hmi_event_channel: HmiEventChannelReceiver<'static>)
{
    let i2c = i2c::I2c::new_async(display_i2c_pins.i2c_peripheral,
                                     display_i2c_pins.scl_pin, display_i2c_pins.sda_pin,
                                     I2cIrqs, Config::default());

    let mut display: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();

    display_update_handler(hmi_event_channel, &mut display).await;
}


#[embassy_executor::task]
async fn led_task(led_pio_resources: LedControlResources, hmi_event_channel_receiver: HmiEventChannelReceiver<'static>)
{
    let Pio { mut common, sm0, .. } = Pio::new(led_pio_resources.pio, PioIrqs);
    let program = PioWs2812Program::new(&mut common);
    let pio_ws2812: PioWs2812<'_, PIO0, 0, LED_COUNT> = PioWs2812::new(&mut common, sm0, led_pio_resources.dma_channel, led_pio_resources.data_pin, &program);

    led_update_handler(pio_ws2812, hmi_event_channel_receiver).await;
}

#[embassy_executor::task]
async fn weighing_task(strain_gauge_resources: StrainGaugeResources, hmi_event_channel_sender: HmiEventChannelSender<'static>, mut hmi_event_channel_receiver: HmiEventChannelReceiver<'static>)
{
    let clk_pin_out = Output::new(strain_gauge_resources.clk_pin, Level::Low);
    let data_pin = Input::new(strain_gauge_resources.data_pin, Pull::Up);
    let strain_gauge = Hx711Async::new(clk_pin_out, data_pin, Hx711Gain::Gain128);
    let mut weight_scale = WeightScale::new(strain_gauge).await.unwrap();

    hmi_event_channel_sender.publish_immediate(HmiEvents::ChangeProductState(ProductState::Tare));
    while hmi_event_channel_receiver.next_message_pure().await != PushButtonPressed(true) {}
    hmi_event_channel_sender.publish_immediate(HmiEvents::ChangeProductState(ProductState::Wait));
    weight_scale.stabilize_measurements().await.unwrap();
    weight_scale.tare().await.unwrap();
    // TODO Handle tare failure
    let calibration_mass = 250;
    hmi_event_channel_sender.publish_immediate(HmiEvents::ChangeProductState(ProductState::Calibration(calibration_mass)));
    while hmi_event_channel_receiver.next_message_pure().await != PushButtonPressed(true) {}
    hmi_event_channel_sender.publish_immediate(HmiEvents::ChangeProductState(ProductState::Wait));
    weight_scale.calibrate(calibration_mass as f32).await.unwrap();
    // TODO Handle calibration failure
    hmi_event_channel_sender.publish_immediate(HmiEvents::ChangeProductState(ProductState::CalibrationDone));
    Timer::after(Duration::from_secs(2)).await;
    hmi_event_channel_sender.publish_immediate(HmiEvents::ChangeProductState(ProductState::Home));

    loop {
        let new_weight = weight_scale.get_instantaneous_weight_grams().await.unwrap();
        hmi_event_channel_sender.publish_immediate(HmiEvents::WeightUpdate(new_weight));
        Timer::after(Duration::from_millis(250)).await;
    }
}