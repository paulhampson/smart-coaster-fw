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

use smartcoaster_messages::general::builder::GeneralMessagesBuilder;
use smartcoaster_messages::GeneralMessages;
use std::io::{Error as IoError, ErrorKind, Read, Result as IoResult, Write};
use std::time::Duration;
use log::LevelFilter;

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

/// Receives a CBOR message with length-prefixed framing from serial
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

    // Read the CBOR message data
    serial.read_exact(&mut buffer[..message_len])?;

    // Decode the CBOR message
    minicbor::decode::<M>(&buffer[..message_len])
        .map_err(|_| IoError::new(ErrorKind::InvalidData, "Decode error"))
}

fn main() -> IoResult<()> {
    // Parse log level from command-line arguments
    let log_level = parse_log_level();

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .format_timestamp_millis()
        .init();

    log::info!("SmartCoaster Firmware Loader");
    log::info!("==================================\n");

    // List available serial ports
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
    let port_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ports[0].port_name.clone());

    log::info!("Connecting to: {}", port_name);

    // Open the serial port
    let mut serial = serialport::new(&port_name, 115200)
        .timeout(Duration::from_secs(5))
        .open()
        .map_err(|e| IoError::new(ErrorKind::ConnectionRefused, e.to_string()))?;

    log::info!("Connected successfully!");
    log::info!("Baud rate: 115200");
    log::info!("Timeout: 5s\n");

    // Give device time to initialize
    std::thread::sleep(Duration::from_millis(100));


    // Build and send Hello message
    log::info!("Creating Hello message...");
    let hello = GeneralMessagesBuilder::new().hello();
    log::debug!("Hello message: {:?}", hello);

    log::info!("Sending Hello message...");
    send_message(&mut serial, &hello)?;

    // Wait for HelloResp
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

    log::info!("Test application completed");
    Ok(())
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
