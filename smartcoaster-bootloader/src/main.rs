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

mod usb;
mod display;
mod status_messaging;

use core::cell::RefCell;
use crate::usb::firmware_downloader::FirmwareDownloader;
use cortex_m_rt::exception;
use defmt::{info};
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use embassy_boot_rp::*;
use embassy_executor::Spawner;
use embassy_rp::flash::Flash;
use embassy_rp::gpio::{Input, Pull};
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{Duration, Timer};

use embassy_rp::Peri;
use embassy_rp::{bind_interrupts, peripherals};
use embassy_rp::i2c::{self, Config};
use embassy_rp::peripherals::I2C0;
use assign_resources::assign_resources;
use embassy_sync::pubsub::PubSubChannel;
use sh1106::{Builder, prelude::*};
use crate::display::DisplayManager;
use crate::status_messaging::{BootloaderStatusChannel, BootloaderStatusMessage};

static STATUS_CHANNEL: BootloaderStatusChannel = PubSubChannel::new();

const FLASH_SIZE: usize = 16 * 1024 * 1024;

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
}

struct Resources {
    display_i2c: DisplayI2cPins,
    hmi_inputs: HmiInputPins,
}

bind_interrupts!(struct I2cIrqs {
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Default::default());
    let resources = split_resources! {p};

    // Uncomment this if you are debugging the bootloader with debugger/RTT attached,
    // as it prevents a hard fault when accessing flash 'too early' after boot.
    // for i in 0..10000000 {
    //     cortex_m::asm::nop();
    // }

    info!("Bootloader starting");

    let resources = Resources {
        display_i2c: resources.display_i2c,
        hmi_inputs: resources.hmi_inputs,
    };

    spawner.must_spawn(display_task(resources.display_i2c));

    let status_publisher = STATUS_CHANNEL.publisher().unwrap();

    status_publisher.publish(BootloaderStatusMessage::Starting).await;

    let usb = p.USB;
    let flash = p.FLASH;

    let flash = Flash::<_, _, FLASH_SIZE>::new_blocking(flash);
    let flash = Mutex::new(RefCell::new(flash));

    let config = FirmwareUpdaterConfig::from_linkerfile_blocking(&flash, &flash);
    let mut aligned = AlignedBuffer([0; 1]);
    let mut updater = BlockingFirmwareUpdater::new(config, &mut aligned.0);
    let mut current_state = updater.get_state().unwrap();

    if current_state == State::Boot {
        let saved_state = updater.get_state().unwrap();

        // check if button is pushed - if so and it's still held after 2 sec, enter DFU mode
        let push_btn_pin = resources.hmi_inputs.push_btn_pin;
        let push_btn_input = Input::new(push_btn_pin, Pull::Up);
        if push_btn_input.is_low() {
            info!("Button pressed - checking for hold");
            const CHECK_PERIOD: Duration = Duration::from_millis(50);
            const HOLD_TIME: Duration = Duration::from_millis(2000);
            for _ in 0..(HOLD_TIME.as_millis() / CHECK_PERIOD.as_millis()) {
                Timer::after(CHECK_PERIOD).await;
                status_publisher.publish_immediate(BootloaderStatusMessage::HoldButton);
                if push_btn_input.is_high() { // released, carry on
                    current_state = saved_state;
                    info!("Button released early - continuing");
                    break;
                } else {
                    // held, enter DFU mode
                    info!("Button held - entering DFU mode");
                    current_state = State::DfuDetach;
                }
            }
        }
    }

    info!("Finished checking button - state = {}", current_state);

    if current_state == State::DfuDetach {
        info!("Entering DFU mode");
        status_publisher.publish_immediate(BootloaderStatusMessage::WaitingForFirmware);
        let downloader_status_publisher = STATUS_CHANNEL.publisher().unwrap();
        let fw_downloader = FirmwareDownloader::new(downloader_status_publisher);
        // this will trigger a reset when finished, but in future it could return if it cleans up the usb task
        fw_downloader.start(usb, &flash, spawner).await;
    } else {
        status_publisher.publish_immediate(BootloaderStatusMessage::StartingApplication);
    }

    // allow screen to update because the bootloader operation is not async, so will not yield
    Timer::after_millis(100).await;

    info!("Running embassy bootloader");

    let config = BootLoaderConfig::from_linkerfile_blocking(&flash, &flash, &flash);
    let active_offset = config.active.offset();
    let bl: BootLoader = BootLoader::prepare(config);

    info!("Booting application");

    unsafe { bl.load(embassy_rp::flash::FLASH_BASE as u32 + active_offset) }
}

#[embassy_executor::task]
async fn display_task(
    display_i2c_pins: DisplayI2cPins,
    // app_subscriber: BootloaderStatusChannelSubscriber<'static>,
) {
    let i2c = i2c::I2c::new_async(
        display_i2c_pins.i2c_peripheral,
        display_i2c_pins.scl_pin,
        display_i2c_pins.sda_pin,
        I2cIrqs,
        Config::default(),
    );

    info!("Display task started");

    let status_subscriber = STATUS_CHANNEL.subscriber().unwrap();
    let display: GraphicsMode<_> = Builder::new().connect_i2c(i2c).into();
    let mut display_manager =
        DisplayManager::new(display,
                            status_subscriber,
        );
    display_manager.run().await;
}

#[unsafe(no_mangle)]
#[cfg_attr(target_os = "none", unsafe(link_section = ".HardFault.user"))]
unsafe extern "C" fn HardFault() {
    cortex_m::peripheral::SCB::sys_reset();
}

#[exception]
unsafe fn DefaultHandler(_: i16) -> ! {
    const SCB_ICSR: *const u32 = 0xE000_ED04 as *const u32;
    let irqn = unsafe { core::ptr::read_volatile(SCB_ICSR) } as u8 as i16 - 16;

    panic!("DefaultHandler #{:?}", irqn);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    cortex_m::asm::udf();
}
