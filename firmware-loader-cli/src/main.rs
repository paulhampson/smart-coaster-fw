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

use smartcoaster_host_core::{SmartcoasterHostFirmwareLoader};
use std::io::{Error as IoError, ErrorKind, Read, Result as IoResult, Write};
use std::time::Duration;
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

    println!("Loading firmware data from file");

    // Read and process firmware file
    log::debug!("Reading firmware file: {}", firmware_file_path);
    let firmware_data = util::read_binary_file(&firmware_file_path)?;

    println!("Initiating contact with device");

    // Create the firmware loader session
    const BUFFER_SIZE: usize = 4096;
    let mut session: SmartcoasterHostFirmwareLoader<BUFFER_SIZE> = SmartcoasterHostFirmwareLoader::new(&firmware_data);

    let mut rx_buffer = [0u8; BUFFER_SIZE];
    let mut tx_pending = false;
    let mut progress_bar: Option<ProgressBar> = None;
    let mut last_chunk = 0u32;

    // Initialise the session
    let zero_buffer = [0u8; 0];
    let updated_session = match SmartcoasterHostFirmwareLoader::session_handler(session, &zero_buffer) {
        Ok(new_session) => {
            new_session
        }
        Err(e) => {
            log::error!("Session handler error: {:?}", e);
            return Err(IoError::new(ErrorKind::Other, format!("Session error: {:?}", e)));
        }
    };

    session = updated_session;

    // Main communication loop
    loop {
        if SmartcoasterHostFirmwareLoader::is_session_ended(&session) {
            break;
        }

        // Send any pending messages
        if !tx_pending {
            if let Some(bytes_to_send) = SmartcoasterHostFirmwareLoader::get_bytes_to_send(&mut session) {
                log::trace!("Sending {} bytes", bytes_to_send.len());
                serial.write_all(bytes_to_send)
                    .map_err(|e| IoError::new(ErrorKind::Other, format!("Failed to send: {}", e)))?;
                tx_pending = true;
            }
        }

        // Read incoming data from serial port
        match serial.read(&mut rx_buffer) {
            Ok(n) if n > 0 => {
                log::trace!("Received {} bytes", n);
                tx_pending = false;

                // Process the incoming bytes through the session handler
                match SmartcoasterHostFirmwareLoader::session_handler(session, &rx_buffer[..n]) {
                    Ok(updated_session) => {
                        session = updated_session;

                        // Get progress info
                        let progress = SmartcoasterHostFirmwareLoader::get_chunk_progress(&session);

                        // Initialize progress bar on first chunk request
                        if progress.max_chunks > 0 && progress_bar.is_none() {
                            let pb = ProgressBar::new(progress.max_chunks as u64);
                            pb.set_style(ProgressStyle::default_bar()
                                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} chunks ({eta})")
                                .unwrap()
                                .progress_chars("#>-"));
                            progress_bar = Some(pb);
                        }

                        // Update the progress bar if we've received a new chunk
                        if let Some(ref pb) = progress_bar {
                            if progress.current_chunk > last_chunk {
                                pb.set_position(progress.current_chunk as u64);
                                last_chunk = progress.current_chunk;
                            }
                        }
                    }
                    Err(smartcoaster_host_core::SessionHandlerError::SessionEnded) => {
                        log::debug!("Session ended successfully");
                        if let Some(pb) = progress_bar {
                            pb.finish_with_message("✓ Firmware transfer completed");
                        }
                        println!("Firmware transfer completed - please wait for device to load firmware and boot");
                        break;
                    }
                    Err(e) => {
                        log::error!("Session handler error: {:?}", e);
                        return Err(IoError::new(ErrorKind::Other, format!("Session error: {:?}", e)));
                    }
                }
            }
            Ok(_) => {
                // No data received but no error
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Timeout is expected for blocking reads, just try again
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                log::error!("Serial read error: {}", e);
                return Err(e);
            }
        }
    }

    Ok(())
}