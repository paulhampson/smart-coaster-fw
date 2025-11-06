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

mod util;
mod cbor_messaging;


use smartcoaster_messages::general::builder::GeneralMessagesBuilder;
use smartcoaster_messages::{BootloaderMessages, GeneralMessages};
use std::io::{Error as IoError, ErrorKind, Result as IoResult};
use std::time::Duration;
use smartcoaster_messages::bootloader::builder::BootloaderMessagesBuilder;
use smartcoaster_messages::custom_data_types::{AsconHash256Bytes, VersionNumber};
use indicatif::{ProgressBar, ProgressStyle};


fn main() -> IoResult<()> {
    let log_level = util::parse_log_level();

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .format_timestamp_millis()
        .init();

    println!("Starting SmartCoaster Firmware Loader");

    let args: Vec<String> = std::env::args().collect();

    // Extract firmware file path (first positional arg after --log-level if present)
    let firmware_file_path = util::extract_firmware_file_path(&args)?;

    println!("Available serial ports:");
    let ports = serialport::available_ports()
        .map_err(|e| IoError::new(ErrorKind::Other, e.to_string()))?;

    if ports.is_empty() {
        log::error!("No serial ports found!");
        return Ok(());
    }

    for port in &ports {
        println!("  - {}", port.port_name);
    }

    // Use the first available port or get from the environment
    let port_name = args
        .iter()
        .position(|arg| arg == "--port")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| ports[0].port_name.clone());

    log::debug!("Connecting to: {}", port_name);

    let mut serial = serialport::new(&port_name, 115200)
        .timeout(Duration::from_secs(5))
        .open()
        .map_err(|e| IoError::new(ErrorKind::ConnectionRefused, e.to_string()))?;

    println!("Connected to serial port");
    log::debug!("Connected successfully!");
    log::debug!("Baud rate: 115200");
    log::debug!("Timeout: 5s\n");

    // Give the device time to initialize
    std::thread::sleep(Duration::from_millis(100));

    println!("Initiating contact with device");

    log::debug!("Creating Hello message...");
    let hello = GeneralMessagesBuilder::new().hello();
    log::debug!("Hello message: {:?}", hello);

    log::debug!("Sending Hello message...");
    cbor_messaging::send_message(&mut serial, &hello)?;

    log::debug!("Waiting for HelloResp...");
    let mut buffer = [0u8; 1024];

    match cbor_messaging::receive_message::<GeneralMessages>(&mut serial, &mut buffer) {
        Ok(response) => {
            log::debug!("Received message: {:?}", response);

            if let GeneralMessages::HelloResp(hello_resp) = response {
                log::debug!("HelloResp received successfully!");
                log::debug!("  Mode: {:?}", hello_resp.mode);
                log::debug!("  Version: {:?}", hello_resp.version);
            } else {
                log::error!("Expected HelloResp but got different message type");
            }
        }
        Err(e) => {
            log::error!("Failed to receive HelloResp: {}", e);
        }
    }

    // Read and process firmware file
    log::debug!("Reading firmware file: {}", firmware_file_path);
    let firmware_data = util::read_binary_file(&firmware_file_path)?;

    let image_size_bytes = firmware_data.len() as u32;
    log::debug!("Firmware image size: {} bytes", image_size_bytes);

    log::debug!("Calculating Ascon-Hash256...");
    let hash_bytes = util::calculate_ascon_hash256(&firmware_data);
    let hash = AsconHash256Bytes::from_bytes(hash_bytes);
    log::debug!("Hash calculated successfully");

    // Create and send ReadyToDownload message
    log::debug!("Creating ReadyToDownload message...");
    let ready_to_download = BootloaderMessagesBuilder::new()
        .ready_to_download()
        .image_size_bytes(image_size_bytes)
        .version(VersionNumber::new(0, 0, 1)) // TODO get version number
        .hash(hash)
        .build();

    log::debug!("ReadyToDownload message: {:?}", ready_to_download);

    log::debug!("Sending ReadyToDownload message...");
    cbor_messaging::send_message(&mut serial, &ready_to_download)?;

    log::debug!("Waiting for ReadyToDownloadResponse...");

    match cbor_messaging::receive_message::<BootloaderMessages>(&mut serial, &mut buffer) {
        Ok(response) => {
            log::debug!("Received message: {:?}", response);

            if let BootloaderMessages::ReadyToDownloadResponse(resp) = response {
                log::debug!("ReadyToDownloadResponse received successfully!");
                log::debug!("  Desired chunk size: {} bytes", resp.desired_chunk_size);

                // Calculate total number of chunks
                let chunk_size = smartcoaster_messages::bootloader::CHUNK_SIZE;
                let total_chunks = (firmware_data.len() + chunk_size - 1) / chunk_size;

                // Create progress bar
                let progress_bar = ProgressBar::new(total_chunks as u64);
                progress_bar.set_style(ProgressStyle::default_bar()
                    .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} chunks ({eta})")
                    .unwrap()
                    .progress_chars("#>-"));

                // Track which chunks have been successfully processed
                let mut processed_chunks = std::collections::HashSet::new();

                // Now wait for ChunkReq messages and send ChunkResp in reply
                log::debug!("Waiting for ChunkReq messages...");
                let mut request_buffer = [0u8; 1024];
                let chunk_size = smartcoaster_messages::bootloader::CHUNK_SIZE;

                loop {
                    match cbor_messaging::receive_message::<BootloaderMessages>(&mut serial, &mut request_buffer) {
                        Ok(chunk_message) => {
                            match chunk_message {
                                BootloaderMessages::ChunkReq(req) => {
                                    log::debug!("Received ChunkReq for chunk number: {}", req.chunk_number);

                                    // Calculate byte offset
                                    let byte_offset = (req.chunk_number as usize) * chunk_size;

                                    // Verify we have data for this chunk
                                    if byte_offset >= firmware_data.len() {
                                        log::error!("Chunk request out of bounds: offset {} >= file size {}",
                                                  byte_offset, firmware_data.len());
                                        break;
                                    }

                                    // Prepare chunk data (pad with zeros if needed)
                                    let mut chunk_data = [0u8; smartcoaster_messages::bootloader::CHUNK_SIZE];
                                    let available_bytes = std::cmp::min(chunk_size, firmware_data.len() - byte_offset);
                                    chunk_data[..available_bytes]
                                        .copy_from_slice(&firmware_data[byte_offset..byte_offset + available_bytes]);

                                    log::debug!("Sending {} bytes from offset {}", available_bytes, byte_offset);

                                    // Build and send ChunkResp
                                    let chunk_resp = BootloaderMessagesBuilder::new()
                                        .chunk_resp()
                                        .chunk_number(req.chunk_number)
                                        .chunk_data(chunk_data)
                                        .build();

                                    log::debug!("Sending ChunkResp for chunk {}", req.chunk_number);
                                    cbor_messaging::send_message(&mut serial, &chunk_resp)?;

                                    // Only update the progress bar if this is a new chunk (not a retry)
                                    if processed_chunks.insert(req.chunk_number) {
                                        progress_bar.inc(1);
                                    } else {
                                        log::debug!("Chunk {} was already processed (retry)", req.chunk_number);
                                    }
                                }
                                BootloaderMessages::Goodbye(_) => {
                                    log::debug!("Received Goodbye message, exiting chunk loop");
                                    break;
                                }
                                _ => {
                                    log::error!("Received unexpected message: {:?}", chunk_message);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to receive message: {}", e);
                            break;
                        }
                    }
                }
            } else {
                log::error!("Expected ReadyToDownloadResponse but got different message type");
            }
        }
        Err(e) => {
            log::error!("Failed to receive ReadyToDownloadResponse: {}", e);
        }
    }

    println!("Firmware transfer completed - please wait for device to load firmware and boot");
    Ok(())
}


