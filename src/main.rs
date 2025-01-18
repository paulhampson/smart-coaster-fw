#![no_std]
#![no_main]

mod application;
mod hmi;
mod led;
mod weight;

use embassy_executor::{Executor, Spawner};
use embassy_time::Duration;
#[allow(unused_imports)]
use {defmt_rtt as _, panic_probe as _};

use crate::hmi::inputs::hmi_input_handler;
use crate::hmi::messaging::{
    HmiChannel, HmiChannelPublisher, HmiChannelSubscriber, UiActionChannel,
    UiActionChannelPublisher, UiActionChannelSubscriber,
};
use crate::hmi::rotary_encoder::DebouncedRotaryEncoder;
use crate::weight::interface::hx711async::{Hx711Async, Hx711Gain};
use assign_resources::assign_resources;
use defmt::info;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{self, Config};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::{I2C0, PIO0};
use embassy_rp::pio::Pio;
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::{bind_interrupts, peripherals, pio};
use embassy_sync::pubsub::PubSubChannel;
use hmi::debouncer::Debouncer;
use sh1106::{prelude::*, Builder};

use crate::application::application_manager::ApplicationManager;
use crate::application::led_manager::LedManager;
use crate::application::messaging::{
    ApplicationChannel, ApplicationChannelPublisher, ApplicationChannelSubscriber,
};
use crate::application::weighing_manager::WeighingManager;
use crate::hmi::display::DisplayManager;
use crate::led::led_control::LedController;
use crate::weight::messaging::{WeighingSystemOverChannel, WeightChannel, WeightChannelPublisher};
use crate::weight::weight::WeightScale;
use static_cell::StaticCell;

use core::ptr::addr_of_mut;
use embedded_alloc::LlffHeap as Heap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

static HMI_CHANNEL: HmiChannel = PubSubChannel::new();
static UI_ACTION_CHANNEL: UiActionChannel = PubSubChannel::new();
static WEIGHT_CHANNEL: WeightChannel = PubSubChannel::new();
static APP_CHANNEL: ApplicationChannel = PubSubChannel::new();

const CORE1_STACK_SIZE: usize = 16 * 1024;
const HEAP_SIZE: usize = 16 * 1024;

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

static mut CORE1_STACK: Stack<CORE1_STACK_SIZE> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    let resources = split_resources! {p};

    // Initialize the allocator BEFORE you use it
    {
        use core::mem::MaybeUninit;

        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) }
    }

    let core0_resources = Core0Resources {
        hmi_inputs: resources.hmi_inputs,
        led_control: resources.led_control,
        strain_gauge_io: resources.strain_gauge_io,
    };
    let core1_resources = Core1Resources {
        display_i2c: resources.display_i2c,
    };

    info!("Launching application across cores");

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

fn core0_main(spawner: Spawner, resources: Core0Resources) {
    spawner
        .spawn(hmi_input_task(
            resources.hmi_inputs,
            HMI_CHANNEL.publisher().unwrap(),
        ))
        .unwrap();
    spawner
        .spawn(led_task(
            resources.led_control,
            APP_CHANNEL.subscriber().unwrap(),
        ))
        .unwrap();
    spawner
        .spawn(weighing_task(
            resources.strain_gauge_io,
            APP_CHANNEL.subscriber().unwrap(),
            WEIGHT_CHANNEL.publisher().unwrap(),
        ))
        .unwrap()
}

fn core1_main(spawner: Spawner, resources: Core1Resources) {
    spawner
        .spawn(display_task(
            resources.display_i2c,
            APP_CHANNEL.subscriber().unwrap(),
            UI_ACTION_CHANNEL.publisher().unwrap(),
        ))
        .unwrap();

    let ws = WeighingSystemOverChannel::new(
        WEIGHT_CHANNEL.subscriber().unwrap(),
        APP_CHANNEL.publisher().unwrap(),
    );
    spawner
        .spawn(application_task(
            APP_CHANNEL.publisher().unwrap(),
            HMI_CHANNEL.subscriber().unwrap(),
            UI_ACTION_CHANNEL.subscriber().unwrap(),
            ws,
        ))
        .unwrap();
}

#[embassy_executor::task]
async fn hmi_input_task(
    hmi_input_pins: HmiInputPins,
    hmi_event_channel: HmiChannelPublisher<'static>,
) {
    let rotary_dt = hmi_input_pins.rotary_dt_pin;
    let rotary_clk = hmi_input_pins.rotary_clk_pin;
    let debounced_btn = Debouncer::new(
        Input::new(hmi_input_pins.push_btn_pin, Pull::Up),
        Duration::from_millis(100),
    );

    let mut rotary_encoder = DebouncedRotaryEncoder::new(
        Input::new(rotary_dt, Pull::Up),
        Input::new(rotary_clk, Pull::Up),
        Duration::from_millis(6),
    );

    hmi_input_handler(hmi_event_channel, debounced_btn, &mut rotary_encoder).await;
}

#[embassy_executor::task]
async fn display_task(
    display_i2c_pins: DisplayI2cPins,
    app_subscriber: ApplicationChannelSubscriber<'static>,
    ui_action_publisher: UiActionChannelPublisher<'static>,
) {
    let i2c = i2c::I2c::new_async(
        display_i2c_pins.i2c_peripheral,
        display_i2c_pins.scl_pin,
        display_i2c_pins.sda_pin,
        I2cIrqs,
        Config::default(),
    );

    let display: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();
    let mut display_manager = DisplayManager::new(display, app_subscriber, ui_action_publisher);
    display_manager.run().await;
}

#[embassy_executor::task]
async fn led_task(
    led_pio_resources: LedControlResources,
    application_subscriber: ApplicationChannelSubscriber<'static>,
) {
    let Pio {
        mut common, sm0, ..
    } = Pio::new(led_pio_resources.pio, PioIrqs);
    let program = PioWs2812Program::new(&mut common);
    let pio_ws2812: PioWs2812<'_, PIO0, 0, LED_COUNT> = PioWs2812::new(
        &mut common,
        sm0,
        led_pio_resources.dma_channel,
        led_pio_resources.data_pin,
        &program,
    );
    let led_control = LedController::new(pio_ws2812);

    let mut led_manager = LedManager::new(led_control, application_subscriber);
    led_manager.run().await;
}

#[embassy_executor::task]
async fn weighing_task(
    strain_gauge_resources: StrainGaugeResources,
    app_subscriber: ApplicationChannelSubscriber<'static>,
    weight_event_sender: WeightChannelPublisher<'static>,
) {
    let clk_pin_out = Output::new(strain_gauge_resources.clk_pin, Level::Low);
    let data_pin = Input::new(strain_gauge_resources.data_pin, Pull::Up);
    let strain_gauge = Hx711Async::new(clk_pin_out, data_pin, Hx711Gain::Gain128);
    let weight_scale = WeightScale::new(strain_gauge).await.unwrap();

    let weighing_manager = WeighingManager::new(app_subscriber, weight_event_sender, weight_scale);
    weighing_manager.run().await;
}

#[embassy_executor::task]
async fn application_task(
    app_channel_sender: ApplicationChannelPublisher<'static>,
    hmi_channel_receiver: HmiChannelSubscriber<'static>,
    ui_action_channel_receiver: UiActionChannelSubscriber<'static>,
    weight_interface: WeighingSystemOverChannel,
) {
    let mut application_manager = ApplicationManager::new(
        hmi_channel_receiver,
        app_channel_sender,
        ui_action_channel_receiver,
        weight_interface,
    );
    application_manager.run().await;
}
