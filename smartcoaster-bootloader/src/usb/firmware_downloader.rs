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

use defmt::info;
use embassy_rp::peripherals::USB;
use embassy_rp::{Peri, bind_interrupts};

use embassy_futures::join::join;
use embassy_rp::usb::{Driver, Instance, InterruptHandler};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use static_cell::StaticCell;

bind_interrupts!(struct UsbIrqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

pub struct FirmwareDownloader {}

impl FirmwareDownloader {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn start(&self, usb_peripheral: Peri<'static, USB>) {
        // Create the driver, from the HAL.
        let driver = Driver::new(usb_peripheral, UsbIrqs);

        // Create embassy-usb Config
        let config = {
            let mut config = embassy_usb::Config::new(0x1209, 0x4004);
            config.manufacturer = Some("SmartCoaster");
            config.product = Some("SmartCoaster Bootloader");
            config.serial_number = Some("12345678");
            config.max_power = 500;
            config.max_packet_size_0 = 64;
            config
        };

        // Create embassy-usb DeviceBuilder using the driver and config.
        // It needs some buffers for building the descriptors.
        let mut builder = {
            static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
            static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
            static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

            let builder = embassy_usb::Builder::new(
                driver,
                config,
                CONFIG_DESCRIPTOR.init([0; 256]),
                BOS_DESCRIPTOR.init([0; 256]),
                &mut [], // no msos descriptors
                CONTROL_BUF.init([0; 64]),
            );
            builder
        };

        static STATE: StaticCell<State> = StaticCell::new();
        let state = STATE.init(State::new());
        // Create classes on the builder.
        let mut class = { CdcAcmClass::new(&mut builder, state, 64) };

        // Build the builder.
        let mut usb = builder.build();

        // Run the USB device.
        let usb_fut = usb.run();

        // Do stuff with the class!
        let serial_usb_fut = async {
            loop {
                class.wait_connection().await;
                info!("Connected");
                let _ = echo(&mut class).await;
                info!("Disconnected");
            }
        };

        join(usb_fut, serial_usb_fut).await;
    }
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}

async fn echo<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    loop {
        let n = class.read_packet(&mut buf).await?;
        let data = &buf[..n];
        info!("data: {:x}", data);
        class.write_packet(data).await?;
    }
}
