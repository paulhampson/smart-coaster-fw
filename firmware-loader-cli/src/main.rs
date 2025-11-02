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

use std::fs::File;
use smartcoaster_messages::general::builder::GeneralMessagesBuilder;
use smartcoaster_messages::{BootloaderMessages, GeneralMessages};
use std::io::{Error as IoError, ErrorKind, Read, Result as IoResult, Write};
use std::time::Duration;
use log::LevelFilter;
use smartcoaster_messages::bootloader::builder::BootloaderMessagesBuilder;
use smartcoaster_messages::custom_data_types::{AsconHash256Bytes, VersionNumber};

/// Sends a CBOR message with length-prefixed framing over serial
fn send_message<M>(serial: &mut Box<dyn serialport::SerialPort>, msg: &M) -> IoResult<()>
where
    M: minicbor::Encode<()> + minicbor::CborLen<()>,
{
    let mut buffer = [0u8; 4096];
    let encoded_len = minicbor::len(msg);
    minicbor::encode(msg, &mut buffer[..])
        .map_err(|_| IoError::new(ErrorKind::InvalidData, "Encode error"))?;

    // Write length prefix (big-endian u16)
    let length_bytes = (encoded_len as u16).to_be_bytes();
    serial.write_all(&length_bytes)?;
    serial.write_all(&buffer[..encoded_len])?;

    log::info!("Sent message of {} bytes", encoded_len);
    Ok(())
}

fn receive_message<'b, M>(
    serial: &mut Box<dyn serialport::SerialPort>,
    buffer: &'b mut [u8],
) -> IoResult<M>
where
    M: minicbor::Decode<'b, ()>,
{
    // Read the 2-byte length prefix
    let mut length_bytes = [0u8; 2];
    serial.read_exact(&mut length_bytes)?;
    let message_len = u16::from_be_bytes(length_bytes) as usize;

    if message_len > buffer.len() {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            "Message too large",
        ));
    }

    serial.read_exact(&mut buffer[..message_len])?;

    minicbor::decode::<M>(&buffer[..message_len])
        .map_err(|_| IoError::new(ErrorKind::InvalidData, "Decode error"))
}

/// Reads a binary file and returns its contents as a Vec<u8>
fn read_binary_file(path: &str) -> IoResult<Vec<u8>> {
    let mut file = File::open(path)
        .map_err(|e| IoError::new(ErrorKind::NotFound, format!("Failed to open file: {}", e)))?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    log::info!("Read {} bytes from {}", contents.len(), path);
    Ok(contents)
}

/// Calculates the Ascon-Hash256 of the given data
fn calculate_ascon_hash256(data: &[u8]) -> [u8; 32] {
    use ascon_hash::digest::Digest;
    use ascon_hash::AsconHash256;

    let mut hasher = AsconHash256::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash_bytes = [0u8; 32];
    hash_bytes.copy_from_slice(&result[..32]);
    hash_bytes
}

fn main() -> IoResult<()> {
    let log_level = parse_log_level();

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .format_timestamp_millis()
        .init();

    log::info!("Starting SmartCoaster Firmware Loader");

    let args: Vec<String> = std::env::args().collect();

    // Extract firmware file path (first positional arg after --log-level if present)
    let firmware_file_path = extract_firmware_file_path(&args)?;

    log::info!("Available serial ports:");
    let ports = serialport::available_ports()
        .map_err(|e| IoError::new(ErrorKind::Other, e.to_string()))?;

    if ports.is_empty() {
        log::error!("No serial ports found!");
        return Ok(());
    }

    for port in &ports {
        log::info!("  - {}", port.port_name);
    }

    // Use the first available port or get from the environment
    let port_name = args
        .iter()
        .position(|arg| arg == "--port")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| ports[0].port_name.clone());

    log::info!("Connecting to: {}", port_name);

    let mut serial = serialport::new(&port_name, 115200)
        .timeout(Duration::from_secs(5))
        .open()
        .map_err(|e| IoError::new(ErrorKind::ConnectionRefused, e.to_string()))?;

    log::info!("Connected successfully!");
    log::info!("Baud rate: 115200");
    log::info!("Timeout: 5s\n");

    // Give the device time to initialize
    std::thread::sleep(Duration::from_millis(100));

    log::info!("Creating Hello message...");
    let hello = GeneralMessagesBuilder::new().hello();
    log::debug!("Hello message: {:?}", hello);

    log::info!("Sending Hello message...");
    send_message(&mut serial, &hello)?;

    log::info!("Waiting for HelloResp...");
    let mut buffer = [0u8; 1024];

    match receive_message::<GeneralMessages>(&mut serial, &mut buffer) {
        Ok(response) => {
            log::info!("Received message: {:?}", response);

            if let GeneralMessages::HelloResp(hello_resp) = response {
                log::info!("HelloResp received successfully!");
                log::info!("  Mode: {:?}", hello_resp.mode);
                log::info!("  Version: {:?}", hello_resp.version);
            } else {
                log::error!("Expected HelloResp but got different message type");
            }
        }
        Err(e) => {
            log::error!("Failed to receive HelloResp: {}", e);
        }
    }

    // Read and process firmware file
    log::info!("Reading firmware file: {}", firmware_file_path);
    let firmware_data = read_binary_file(&firmware_file_path)?;

    let image_size_bytes = firmware_data.len() as u32;
    log::info!("Firmware image size: {} bytes", image_size_bytes);

    log::info!("Calculating Ascon-Hash256...");
    let hash_bytes = calculate_ascon_hash256(&firmware_data);
    let hash = AsconHash256Bytes::from_bytes(hash_bytes);
    log::info!("Hash calculated successfully");

    // Create and send ReadyToDownload message
    log::info!("Creating ReadyToDownload message...");
    let ready_to_download = BootloaderMessagesBuilder::new()
        .ready_to_download()
        .image_size_bytes(image_size_bytes)
        .version(VersionNumber::new(0, 0, 1)) // TODO get version number
        .hash(hash)
        .build();

    log::debug!("ReadyToDownload message: {:?}", ready_to_download);

    log::info!("Sending ReadyToDownload message...");
    send_message(&mut serial, &ready_to_download)?;

    log::info!("Waiting for ReadyToDownloadResponse...");

    match receive_message::<BootloaderMessages>(&mut serial, &mut buffer) {
        Ok(response) => {
            log::info!("Received message: {:?}", response);

            if let BootloaderMessages::ReadyToDownloadResponse(resp) = response {
                log::info!("ReadyToDownloadResponse received successfully!");
                log::info!("  Desired chunk size: {} bytes", resp.desired_chunk_size);

                // Now wait for ChunkReq messages and send ChunkResp in reply
                log::info!("Waiting for ChunkReq messages...");
                let mut request_buffer = [0u8; 1024];
                let chunk_size = smartcoaster_messages::bootloader::CHUNK_SIZE;

                loop {
                    match receive_message::<BootloaderMessages>(&mut serial, &mut request_buffer) {
                        Ok(chunk_message) => {
                            match chunk_message {
                                BootloaderMessages::ChunkReq(req) => {
                                    log::info!("Received ChunkReq for chunk number: {}", req.chunk_number);

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

                                    log::info!("Sending ChunkResp for chunk {}", req.chunk_number);
                                    send_message(&mut serial, &chunk_resp)?;
                                }
                                BootloaderMessages::Goodbye(_) => {
                                    log::info!("Received Goodbye message, exiting chunk loop");
                                    break;
                                }
                                _ => {
                                    log::info!("Received unexpected message: {:?}", chunk_message);
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
    
    log::info!("Test application completed");
    Ok(())
}

/// Extract the firmware file path from command-line arguments
/// Skips --log-level and its value, and --port and its value
fn extract_firmware_file_path(args: &[String]) -> IoResult<String> {
    let mut i = 1; // Skip program name

    while i < args.len() {
        match args[i].as_str() {
            "--log-level" => {
                i += 2; // Skip flag and value
            }
            "--port" => {
                i += 2; // Skip flag and value
            }
            _ => {
                // First non-flag argument is the firmware file path
                return Ok(args[i].clone());
            }
        }
    }

    Err(IoError::new(
        ErrorKind::InvalidInput,
        "No firmware file path provided. Usage: firmware-loader-cli [--log-level LEVEL] [--port PORT] <firmware.bin>",
    ))
}

/// Parse log level from command-line arguments
/// Supports: --log-level <LEVEL> or RUST_LOG environment variable
/// Defaults to INFO if neither is provided
fn parse_log_level() -> LevelFilter {
    let args: Vec<String> = std::env::args().collect();

    // Check for --log-level argument
    for i in 0..args.len() {
        if args[i] == "--log-level" && i + 1 < args.len() {
            return match args[i + 1].to_uppercase().as_str() {
                "OFF" => LevelFilter::Off,
                "ERROR" => LevelFilter::Error,
                "WARN" => LevelFilter::Warn,
                "INFO" => LevelFilter::Info,
                "DEBUG" => LevelFilter::Debug,
                "TRACE" => LevelFilter::Trace,
                _ => {
                    eprintln!("Unknown log level: {}. Using INFO", args[i + 1]);
                    LevelFilter::Info
                }
            };
        }
    }

    // Default to INFO
    LevelFilter::Info
}
