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

use core::cell::RefCell;

use crate::usb::firmware_downloader::FirmwareDownloader;
use cortex_m_rt::exception;
use defmt::info;
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use embassy_boot_rp::*;
use embassy_executor::Spawner;
use embassy_rp::flash::Flash;
use embassy_sync::blocking_mutex::Mutex;

const FLASH_SIZE: usize = 16 * 1024 * 1024;

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Default::default());

    // Uncomment this if you are debugging the bootloader with debugger/RTT attached,
    // as it prevents a hard fault when accessing flash 'too early' after boot.
    // for i in 0..10000000 {
    //     cortex_m::asm::nop();
    // }

    info!("Bootloader starting");


    let usb = p.USB;
    let flash = p.FLASH;

    let flash = Flash::<_, _, FLASH_SIZE>::new_blocking(flash);
    let flash = Mutex::new(RefCell::new(flash));

    let config = FirmwareUpdaterConfig::from_linkerfile_blocking(&flash, &flash);
    let mut aligned = AlignedBuffer([0; 1]);
    let mut updater = BlockingFirmwareUpdater::new(config, &mut aligned.0);
    let mut current_state = updater.get_state().unwrap();

    // TODO remove this forcing into DFU mode - change to be triggered by the button being held down
    if current_state == State::Boot {
        current_state = State::DfuDetach;
    }

    if current_state == State::DfuDetach {
        info!("Entering DFU mode");
        let fw_downloader = FirmwareDownloader::new();
        // this will trigger a reset when finished, but in future it could return if it cleans up the usb task
        fw_downloader.start(usb, &flash, spawner).await;
    }

    info!("Running embassy bootloader");

    let config = BootLoaderConfig::from_linkerfile_blocking(&flash, &flash, &flash);
    let active_offset = config.active.offset();
    let bl: BootLoader = BootLoader::prepare(config);

    info!("Booting application");

    unsafe { bl.load(embassy_rp::flash::FLASH_BASE as u32 + active_offset) }
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
