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

use defmt::{Debug2Format, info, warn};
use embassy_executor::Spawner;
use embassy_rp::peripherals::USB;
use embassy_rp::{Peri, bind_interrupts};

use crate::usb::cbor_send_receive::{read_cbor_message, send_cbor_message};
use embassy_rp::usb::{Driver, Instance, InterruptHandler};
use embassy_usb::class::cdc_acm::{BufferedReceiver, CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use smartcoaster_messages::GeneralMessages;
use smartcoaster_messages::custom_data_types::VersionNumber;
use smartcoaster_messages::general::builder::GeneralMessagesBuilder;
use smartcoaster_messages::general::hello::SystemMode::Bootloader;
use static_cell::StaticCell;

bind_interrupts!(struct UsbIrqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const MAX_PACKET_SIZE: u8 = 64;

pub struct FirmwareDownloader {}

impl FirmwareDownloader {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn start(&self, usb_peripheral: Peri<'static, USB>, spawner: Spawner) {
        // Create the driver, from the HAL.
        let driver = Driver::new(usb_peripheral, UsbIrqs);

        let config = {
            let mut config = embassy_usb::Config::new(0x1209, 0x4004);
            config.manufacturer = Some("SmartCoaster");
            config.product = Some("SmartCoaster Bootloader");
            config.serial_number = Some("12345678");
            config.max_power = 500;
            config.max_packet_size_0 = MAX_PACKET_SIZE;
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
        let mut class = { CdcAcmClass::new(&mut builder, state, 64) };

        let usb = builder.build();

        spawner.spawn(usb_task(usb)).unwrap();

        class.wait_connection().await;
        let (mut sender, receiver) = class.split();

        static RX_BUF: StaticCell<[u8; 1024]> = StaticCell::new();
        let rx_buf = RX_BUF.init([0u8; 1024]); // TODO need to get max message size
        let mut buffered_rx = receiver.into_buffered(rx_buf);

        // Do stuff with the class!
        let serial_usb_fut = async {
            loop {
                info!("Connected");
                let _ = firmware_download(&mut sender, &mut buffered_rx).await;
                info!("Disconnected");
            }
        };

        serial_usb_fut.await;
    }
}

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>) {
    usb.run().await;
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

enum FirmwareDownloaderState {
    WaitingForHello,
    WaitingForReadyToDownload,
    WaitingForChunk,
    DownloadFinished,
}

async fn firmware_download<'d, T: Instance + 'd>(
    sender: &mut embassy_usb::class::cdc_acm::Sender<'d, Driver<'d, T>>,
    receiver: &mut BufferedReceiver<'d, Driver<'d, USB>>,
) -> Result<(), Disconnected> {
    let mut state = FirmwareDownloaderState::WaitingForHello;

    let mut buffer = [0u8; 1024];

    loop {
        match state {
            FirmwareDownloaderState::WaitingForHello => {
                info!("Waiting for Hello message");
                match read_cbor_message(receiver, &mut buffer).await {
                    Ok(message) => {
                        info!("Received message: {:?}", Debug2Format(&message));
                        if let GeneralMessages::Hello(_hello) = message {
                            info!("Hello message received");
                            let hello_resp = GeneralMessagesBuilder::new()
                                .hello_resp()
                                .mode(Bootloader)
                                .version(VersionNumber::new(0, 0, 0))
                                .build();

                            send_cbor_message(sender, &hello_resp)
                                .await
                                .expect("Failed to send HelloResp");

                            // TODO implement further states
                            // state = FirmwareDownloaderState::WaitingForReadyToDownload;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read message: {:?}", Debug2Format(&e));
                    }
                }
            }
            FirmwareDownloaderState::WaitingForReadyToDownload => {}
            FirmwareDownloaderState::WaitingForChunk => {}
            FirmwareDownloaderState::DownloadFinished => {}
        }
    }
}
