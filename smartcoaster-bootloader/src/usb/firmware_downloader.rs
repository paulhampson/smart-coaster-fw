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

use core::cell::RefCell;
use core::cmp::min;
use ascon_hash::{AsconHash256, Digest};
use defmt::{Debug2Format, info, warn, debug, trace, error};
use embassy_boot_rp::{AlignedBuffer, BlockingFirmwareUpdater, FirmwareUpdaterConfig};
use embassy_executor::Spawner;
use embassy_rp::peripherals::{USB};
use embassy_rp::{Peri, bind_interrupts};

use crate::usb::cbor_send_receive::{read_cbor_message, send_cbor_message};
use embassy_rp::usb::{Driver, Instance, InterruptHandler};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_usb::class::cdc_acm::{BufferedReceiver, CdcAcmClass, State};
use embassy_usb::driver::EndpointError;
use embassy_boot_rp::BootLoaderConfig;
use embedded_storage::nor_flash::NorFlash;
use smartcoaster_messages::{BootloaderMessages, GeneralMessages};
use smartcoaster_messages::custom_data_types::{AsconHash256Bytes, VersionNumber};
use smartcoaster_messages::general::builder::GeneralMessagesBuilder;
use smartcoaster_messages::general::hello::SystemMode::Bootloader;
use static_cell::StaticCell;
use smartcoaster_messages::bootloader::builder::BootloaderMessagesBuilder;
use smartcoaster_messages::general::goodbye::GoodbyeReason;

bind_interrupts!(struct UsbIrqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const MAX_PACKET_SIZE: u8 = 64;

pub struct FirmwareDownloader {}

impl FirmwareDownloader {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn start<F: NorFlash>(&self, usb_peripheral: Peri<'static, USB>, flash: &Mutex<NoopRawMutex, RefCell<F>>, spawner: Spawner) {
        // Create the driver, from the HAL.
        let driver = Driver::new(usb_peripheral, UsbIrqs);

        let config =
            FirmwareUpdaterConfig::from_linkerfile_blocking(&flash, &flash);
        let mut aligned = AlignedBuffer([0; 1]);
        let mut updater = BlockingFirmwareUpdater::new(config, &mut aligned.0);

        let config = {
            let mut config = embassy_usb::Config::new(0x1209, 0x4004); // Pending acceptance of USB PID from pid.codes
            config.manufacturer = Some("SmartCoaster");
            config.product = Some("SmartCoaster Bootloader");
            config.serial_number = Some("12345678"); // TODO get this from flash device
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
                firmware_download(&mut sender, &mut buffered_rx, &mut updater).await;
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

async fn firmware_download<'d, T: Instance + 'd, DFU: NorFlash, STATE: NorFlash>(
    sender: &mut embassy_usb::class::cdc_acm::Sender<'d, Driver<'d, T>>,
    receiver: &mut BufferedReceiver<'d, Driver<'d, USB>>,
    updater: &mut BlockingFirmwareUpdater<'_, DFU, STATE>,
) -> ! {
    let mut state = FirmwareDownloaderState::WaitingForHello;

    let mut buffer = [0u8; 1024];

    let mut image_size_bytes = 0;
    let mut expected_image_hash = AsconHash256Bytes::default();
    let mut received_image_hash = AsconHash256::new();
    let mut chunk_index = 0;

    loop {
        match state {
            FirmwareDownloaderState::WaitingForHello => {
                info!("Waiting for Hello message");
                match read_cbor_message(receiver, &mut buffer).await {
                    Ok(message) => {
                        debug!("Received message: {:?}", Debug2Format(&message));
                        if let GeneralMessages::Hello(_hello) = message {
                            let hello_resp = GeneralMessagesBuilder::new()
                                .hello_resp()
                                .mode(Bootloader)
                                .version(VersionNumber::new(0, 0, 0))
                                .build();

                            send_cbor_message(sender, &hello_resp)
                                .await
                                .expect("Failed to send HelloResp");

                            state = FirmwareDownloaderState::WaitingForReadyToDownload;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read message: {:?}", Debug2Format(&e));
                    }
                }
            }
            FirmwareDownloaderState::WaitingForReadyToDownload => {
                info!("Waiting for ReadyToDownload message");
                match read_cbor_message(receiver, &mut buffer).await {
                    Ok(message) => {
                        debug!("Received message: {:?}", Debug2Format(&message));
                        if let BootloaderMessages::ReadyToDownload(ready_to_download) = message {
                            image_size_bytes = ready_to_download.image_size_bytes;
                            expected_image_hash = ready_to_download.hash;
                            info!("Image size: {} bytes", image_size_bytes);
                            info!("Image hash: {:?}", Debug2Format(&expected_image_hash));

                            let resp = BootloaderMessagesBuilder::new().ready_to_download_response().build();
                            send_cbor_message(sender, &resp)
                                .await
                                .expect("Failed to send ReadyToDownloadResponse");
                            let chunk_req = BootloaderMessagesBuilder::new().chunk_req().chunk_number(chunk_index).build();
                            send_cbor_message(sender, &chunk_req)
                                .await
                                .expect("Failed to send ChunkReq");
                            state = FirmwareDownloaderState::WaitingForChunk;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read message: {:?}", Debug2Format(&e));
                    }
                }
            }
            FirmwareDownloaderState::WaitingForChunk => {
                info!("Waiting for ChunkResp message");
                match read_cbor_message(receiver, &mut buffer).await {
                    Ok(message) => {
                        debug!("Received message: {:?}", Debug2Format(&message));
                        if let BootloaderMessages::ChunkResp(chunk_resp) = message {
                            if chunk_resp.chunk_number == chunk_index {
                                if chunk_resp.is_crc_ok() {
                                    trace!("Chunk {} CRC OK", chunk_index);

                                    // process received data
                                    let valid_chunk_data_length = min(smartcoaster_messages::bootloader::CHUNK_SIZE,
                                                                      image_size_bytes as usize - chunk_index as usize * smartcoaster_messages::bootloader::CHUNK_SIZE);
                                    info!("Valid chunk data length: {}", valid_chunk_data_length);
                                    received_image_hash.update(&chunk_resp.chunk_data[..valid_chunk_data_length]);
                                    updater
                                        .write_firmware(chunk_index as usize * smartcoaster_messages::bootloader::CHUNK_SIZE, &chunk_resp.chunk_data[..valid_chunk_data_length])
                                        .expect("Failed to write to DFU partition");

                                    chunk_index += 1;
                                    if chunk_index * smartcoaster_messages::bootloader::CHUNK_SIZE as u32 >= image_size_bytes {
                                        state = FirmwareDownloaderState::DownloadFinished;
                                    } else {
                                        let req = BootloaderMessagesBuilder::new().chunk_req().chunk_number(chunk_index).build();
                                        send_cbor_message(sender, &req)
                                            .await
                                            .expect("Failed to send ChunkReq");
                                    }
                                } else {
                                    warn!("CRC failed on chunk {}", chunk_index);
                                    // CRC failure - this repeats the request for the chunk
                                    let req = BootloaderMessagesBuilder::new().chunk_req().chunk_number(chunk_index).build();
                                    send_cbor_message(sender, &req)
                                        .await
                                        .expect("Failed to send ChunkReq");
                                }
                            } else {
                                warn!("Got chunk number {} but expected {}", chunk_resp.chunk_number, chunk_index);
                                let req = BootloaderMessagesBuilder::new().chunk_req().chunk_number(chunk_index).build();
                                send_cbor_message(sender, &req)
                                    .await
                                    .expect("Failed to send ChunkReq");
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read message: {:?}", Debug2Format(&e));
                    }
                }
            }
            FirmwareDownloaderState::DownloadFinished => {
                info!("Download finished");
                let received_hash_output = received_image_hash.finalize();
                let mut hash_bytes = [0u8; 32];
                hash_bytes.copy_from_slice(&received_hash_output[..32]);
                let received_hash = AsconHash256Bytes::from_bytes(hash_bytes);
                let goodbye_reason = if expected_image_hash.eq(&received_hash) {
                    info!("Image hash matches - will swap to new firmware version");
                    updater.mark_updated().expect("Unable to mark update available");
                    GoodbyeReason::InstallingNewFirmware
                } else {
                    error!("Image hash does not match - system will boot into existing software version");
                    // this will clear the DFU partition area
                    updater.prepare_update().expect("Unable to clear DFU partition");
                    // this puts the bootloader into a state that says the existing firmware is OK to boot
                    updater.mark_booted().expect("Unable to mark existing firmware as OK");
                    GoodbyeReason::DownloadHashMismatch
                };

                info!("Sending goodbye - reason: {:?}", Debug2Format(&goodbye_reason));
                let goodbye = BootloaderMessagesBuilder::new().goodbye().reason(goodbye_reason).build();
                send_cbor_message(sender, &goodbye)
                    .await
                    .expect("Failed to send Goodbye");

                info!("Resetting device");
                cortex_m::peripheral::SCB::sys_reset();
            }
        }
    }
}
