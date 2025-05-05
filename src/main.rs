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
#![no_std]
#![no_main]

#[cfg(all(feature = "flat_board", feature = "pcb_rev1"))]
compile_error!("cannot configure for flat_board and pcb_rev1 at the same time");

#[cfg(not(any(feature = "flat_board", feature = "pcb_rev1")))]
compile_error!("no board configured - use feature \"flat_board\" or \"pcb_rev1\"");

mod application;
mod drink_monitor;
mod hmi;
mod led;
mod rtc;
pub mod storage;
mod weight;

use core::ops::Range;
use embassy_executor::{Executor, Spawner};
use embassy_time::{Duration, Timer};
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
use embassy_rp::peripherals::{FLASH, I2C0, I2C1, PIO0};
use embassy_rp::pio::Pio;
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::{bind_interrupts, flash, peripherals, pio};
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

use crate::drink_monitor::drink_monitoring::DrinkMonitoring;
use crate::drink_monitor::messaging::{
    DrinkMonitorChannel, DrinkMonitorChannelPublisher, DrinkMonitorChannelSubscriber,
};
use crate::rtc::{RtcControl, SystemRtc};
use core::ptr::addr_of_mut;
use ds323x::Ds323x;
use embedded_alloc::LlffHeap as Heap;
use storage::settings::accessor::FlashSettingsAccessor;

#[global_allocator]
static HEAP: Heap = Heap::empty();
static HMI_CHANNEL: HmiChannel = PubSubChannel::new();
static UI_ACTION_CHANNEL: UiActionChannel = PubSubChannel::new();
static WEIGHT_CHANNEL: WeightChannel = PubSubChannel::new();
static APP_CHANNEL: ApplicationChannel = PubSubChannel::new();
static DRINK_MONITOR_CHANNEL: DrinkMonitorChannel = PubSubChannel::new();

const CORE1_STACK_SIZE: usize = 16 * 1024;
const HEAP_SIZE: usize = 16 * 1024;

// Ensure this matches memory.x
const FLASH_SIZE: usize = 2 * 1024 * 1024;
const NVM_PAGE_SIZE: usize = 256;

const SETTINGS_SIZE: usize = 0x2000;
const SETTINGS_NVM_FLASH_OFFSET_RANGE: Range<u32> =
    (FLASH_SIZE - SETTINGS_SIZE) as u32..FLASH_SIZE as u32;

const ACTIVITY_LOG_SIZE: usize = 0x8000;
const ACTIVITY_LOG_NVM_FLASH_OFFSET_RANGE: Range<u32> = (SETTINGS_NVM_FLASH_OFFSET_RANGE.start
    - ACTIVITY_LOG_SIZE as u32)
    ..SETTINGS_NVM_FLASH_OFFSET_RANGE.start;

#[cfg(feature = "flat_board")]
const LED_COUNT: usize = 8;

#[cfg(feature = "pcb_rev1")]
const LED_COUNT: usize = 12;

#[cfg(feature = "flat_board")]
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
    storage: StorageResources {
        flash: FLASH,
        dma_channel: DMA_CH1,
    }
}

#[cfg(feature = "pcb_rev1")]
assign_resources! {
    display_i2c: DisplayI2cPins{
        sda_pin: PIN_20,
        scl_pin: PIN_21,
        i2c_peripheral: I2C0
    },
    hmi_inputs: HmiInputPins {
        rotary_dt_pin: PIN_24,
        rotary_clk_pin: PIN_23,
        push_btn_pin: PIN_25,
    },
    led_control: LedControlResources {
        pio: PIO0,
        dma_channel: DMA_CH0,
        data_pin: PIN_18,
        led_power_en: PIN_15,
    }
    strain_gauge_io: StrainGaugeResources {
        clk_pin: PIN_16,
        data_pin: PIN_17,
    }
    storage: StorageResources {
        flash: FLASH,
        dma_channel: DMA_CH1,
    }
    rtc: RtcResources {
        i2c_peripheral: I2C1,
        sda_pin: PIN_2,
        scl_pin: PIN_3,
    }
}

struct Core0Resources {
    hmi_inputs: HmiInputPins,
    led_control: LedControlResources,
    strain_gauge_io: StrainGaugeResources,
    storage: StorageResources,
    rtc: RtcResources,
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

bind_interrupts!(struct I2c1Irqs {
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
});

static mut CORE1_STACK: Stack<CORE1_STACK_SIZE> = Stack::new();
static EXECUTOR0: StaticCell<Executor> = StaticCell::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    let resources = split_resources! {p};

    // Initialize the allocator BEFORE it's used
    {
        use core::mem::MaybeUninit;

        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) }
    }

    let core0_resources = Core0Resources {
        hmi_inputs: resources.hmi_inputs,
        led_control: resources.led_control,
        strain_gauge_io: resources.strain_gauge_io,
        storage: resources.storage,
        rtc: resources.rtc,
    };
    let core1_resources = Core1Resources {
        display_i2c: resources.display_i2c,
    };

    info!("Launching application across cores");

    spawn_core1(
        p.CORE1,
        unsafe { &mut *addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| core1_main(spawner, core1_resources, &HEAP));
        },
    );

    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| core0_main(spawner, core0_resources));
}

fn core0_main(spawner: Spawner, resources: Core0Resources) {
    spawner.spawn(storage_task(resources.storage)).unwrap();
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
            DRINK_MONITOR_CHANNEL.subscriber().unwrap(),
        ))
        .unwrap();
    spawner
        .spawn(weighing_task(
            resources.strain_gauge_io,
            APP_CHANNEL.subscriber().unwrap(),
            WEIGHT_CHANNEL.publisher().unwrap(),
        ))
        .unwrap();
    spawner.spawn(rtc_task(resources.rtc)).unwrap();
}

fn core1_main(spawner: Spawner, resources: Core1Resources, heap: &'static Heap) {
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
            DRINK_MONITOR_CHANNEL.subscriber().unwrap(),
            ws,
            heap,
        ))
        .unwrap();

    let drink_monitor_ws = WeighingSystemOverChannel::new(
        WEIGHT_CHANNEL.subscriber().unwrap(),
        APP_CHANNEL.publisher().unwrap(),
    );
    spawner
        .spawn(drink_monitor_task(
            DRINK_MONITOR_CHANNEL.publisher().unwrap(),
            APP_CHANNEL.subscriber().unwrap(),
            drink_monitor_ws,
        ))
        .unwrap();
}

#[embassy_executor::task]
async fn rtc_task(rtc_resources: RtcResources) {
    let i2c = i2c::I2c::new_async(
        rtc_resources.i2c_peripheral,
        rtc_resources.scl_pin,
        rtc_resources.sda_pin,
        I2c1Irqs,
        Config::default(),
    );

    let rtc: SystemRtc = Ds323x::new_ds3231(i2c);
    let mut rtc_control = RtcControl::new(rtc);
    rtc_control.run().await;
}

#[embassy_executor::task]
async fn storage_task(storage_resources: StorageResources) {
    let flash = embassy_rp::flash::Flash::<FLASH, flash::Async, FLASH_SIZE>::new(
        storage_resources.flash,
        storage_resources.dma_channel,
    );
    let flash = embassy_embedded_hal::adapter::BlockingAsync::new(flash);

    storage::storage_manager::initialise_storage(
        flash,
        SETTINGS_NVM_FLASH_OFFSET_RANGE,
        NVM_PAGE_SIZE,
    )
    .await;

    storage::settings::accessor::initialise_settings().await;

    loop {
        Timer::after(Duration::from_millis(200)).await;
        storage::settings::accessor::process_save_queue().await;
        storage::historical::manager::process_log_queues().await;
    }
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
        Duration::from_millis(4),
    );

    let mut rotary_encoder = DebouncedRotaryEncoder::new(
        Input::new(rotary_dt, Pull::Up),
        Input::new(rotary_clk, Pull::Up),
        Duration::from_millis(4),
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

    let settings = FlashSettingsAccessor::new();
    let display: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();
    let mut display_manager =
        DisplayManager::new(display, app_subscriber, ui_action_publisher, &settings).await;
    display_manager.run().await;
}

#[embassy_executor::task]
async fn led_task(
    led_pio_resources: LedControlResources,
    application_subscriber: ApplicationChannelSubscriber<'static>,
    drink_monitor_subscriber: DrinkMonitorChannelSubscriber<'static>,
) {
    let _led_power_en_output = Output::new(led_pio_resources.led_power_en, Level::High);

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
    let settings = FlashSettingsAccessor::new();
    let led_control = LedController::new(pio_ws2812, settings).await;

    let mut led_manager = LedManager::new(
        led_control,
        application_subscriber,
        drink_monitor_subscriber,
    );
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
    let settings = FlashSettingsAccessor::new();
    let weight_scale = WeightScale::new(strain_gauge, settings).await.unwrap();

    let mut weighing_manager =
        WeighingManager::new(app_subscriber, weight_event_sender, weight_scale);
    weighing_manager.run().await;
}

#[embassy_executor::task]
async fn application_task(
    app_channel_sender: ApplicationChannelPublisher<'static>,
    hmi_channel_receiver: HmiChannelSubscriber<'static>,
    ui_action_channel_receiver: UiActionChannelSubscriber<'static>,
    drink_monitor_receiver: DrinkMonitorChannelSubscriber<'static>,
    weight_interface: WeighingSystemOverChannel,
    heap: &'static Heap,
) {
    let mut application_manager =
        ApplicationManager::new(app_channel_sender, weight_interface, heap);
    application_manager
        .run(
            ui_action_channel_receiver,
            hmi_channel_receiver,
            drink_monitor_receiver,
        )
        .await;
}

#[embassy_executor::task]
async fn drink_monitor_task(
    drink_monitor_publisher: DrinkMonitorChannelPublisher<'static>,
    application_channel_subscriber: ApplicationChannelSubscriber<'static>,
    weight_interface: WeighingSystemOverChannel,
) {
    let settings = FlashSettingsAccessor::new();
    let mut drink_monitor = DrinkMonitoring::new(drink_monitor_publisher, weight_interface);
    drink_monitor
        .run(application_channel_subscriber, settings)
        .await;
}
