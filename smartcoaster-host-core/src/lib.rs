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

#[cfg(target_arch = "wasm32")]
pub mod wasm_bindings;

use std::io::BufRead;
use circular_buffer::CircularBuffer;
use smartcoaster_messages::bootloader::builder::BootloaderMessagesBuilder;
use smartcoaster_messages::BootloaderMessages;
use smartcoaster_messages::custom_data_types::{AsconHash256Bytes, VersionNumber};
use smartcoaster_messages::general::builder::GeneralMessagesBuilder;
use smartcoaster_messages::general::hello::SystemMode::Bootloader;

pub use smartcoaster_messages::FrameError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionHandlerError {
    FramingError(FrameError),
    RxBufferNotEnoughSpace,
    UnexpectedMessage,
    IncorrectDeviceMode,
    SessionEnded,
    ChunkRequestOutOfBounds,
}

impl From<FrameError> for SessionHandlerError {
    fn from(err: FrameError) -> Self {
        SessionHandlerError::FramingError(err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub max_chunks: u32,
    pub current_chunk: u32,
}

#[derive(Debug)]
enum HostSessionState {
    Start,
    WaitingHelloResp,
    WaitingReadyToDownloadResp,
    ChunkTransfer,
    Done,
}

pub struct SmartcoasterHostFirmwareLoader<const BUFFER_SIZE: usize> {
    firmware_bytes: Vec<u8>,
    session_state: HostSessionState,
    tx_message_buffer: [u8; BUFFER_SIZE],
    tx_valid_bytes_size: usize,
    rx_message_buffer: CircularBuffer::<BUFFER_SIZE, u8>,
    download_progress: Progress,
    chunk_size: usize,
}

impl<const BUFFER_SIZE: usize> SmartcoasterHostFirmwareLoader<BUFFER_SIZE> {
    pub fn new(firmware_bytes: Vec<u8>) -> Self {
        Self {
            firmware_bytes,
            session_state: HostSessionState::Start,
            tx_message_buffer: [0u8; BUFFER_SIZE],
            tx_valid_bytes_size: 0,
            rx_message_buffer: CircularBuffer::<BUFFER_SIZE, u8>::new(),
            download_progress: Progress {
                max_chunks: 0,
                current_chunk: 0,
            },
            chunk_size: 0,
        }
    }

    pub fn session_handler(mut session: SmartcoasterHostFirmwareLoader<BUFFER_SIZE>, incoming_bytes: &[u8]) -> Result<SmartcoasterHostFirmwareLoader<BUFFER_SIZE>, SessionHandlerError> {
        if incoming_bytes.len() + session.rx_message_buffer.len() > session.rx_message_buffer.capacity() {
            return Err(SessionHandlerError::RxBufferNotEnoughSpace);
        }
        log::trace!("Called with {} new bytes, {} bytes in buffer", incoming_bytes.len(), session.rx_message_buffer.len());
        session.rx_message_buffer.extend_from_slice(incoming_bytes);
        session.rx_message_buffer.make_contiguous();

        log::trace!("Session state: {:?}", session.session_state);

        match session.session_state {
            HostSessionState::Start => {
                let hello = GeneralMessagesBuilder::new().hello();
                session.tx_valid_bytes_size =
                    smartcoaster_messages::frame_message(&hello, &mut session.tx_message_buffer)?;
                session.session_state = HostSessionState::WaitingHelloResp;
                log::trace!("Generated hello message, waiting for response");
            }
            HostSessionState::WaitingHelloResp => {
                let (message_buffer, _) = session.rx_message_buffer.as_slices();
                let (consumed_bytes_count, message) = match smartcoaster_messages::decode_framed_message(message_buffer) {
                    Ok(result) => result,
                    Err(FrameError::BufferTooSmall(expected_len)) => {
                        log::trace!("Need {expected_len} bytes to decode");
                        return Ok(session);
                    }
                    Err(e) => return Err(SessionHandlerError::FramingError(e)),
                };
                log::trace!("Consumed {} bytes from rx buffer",consumed_bytes_count);
                session.rx_message_buffer.consume(consumed_bytes_count);

                match message {
                    smartcoaster_messages::GeneralMessages::HelloResp(hello_resp) => {
                        log::trace!("Received hello response: {:?}", hello_resp);
                        if hello_resp.mode == Bootloader {} else {
                            return Err(SessionHandlerError::IncorrectDeviceMode);
                        }

                        let image_size_bytes = session.firmware_bytes.len() as u32;
                        log::trace!("Firmware image size: {} bytes", image_size_bytes);

                        log::trace!("Calculating Ascon-Hash256...");
                        let hash_bytes = util::calculate_ascon_hash256(session.firmware_bytes.as_slice());
                        let hash = AsconHash256Bytes::from_bytes(hash_bytes);
                        log::trace!("Hash calculated successfully");

                        // Create and send a ReadyToDownload message
                        log::trace!("Creating ReadyToDownload message...");
                        let ready_to_download = BootloaderMessagesBuilder::new()
                            .ready_to_download()
                            .image_size_bytes(image_size_bytes)
                            .version(VersionNumber::new(0, 0, 0)) // TODO get version number
                            .hash(hash)
                            .build();

                        log::trace!("ReadyToDownload message: {:?}", ready_to_download);

                        session.tx_valid_bytes_size =
                            smartcoaster_messages::frame_message(&ready_to_download, &mut session.tx_message_buffer)?;

                        session.session_state = HostSessionState::WaitingReadyToDownloadResp;
                    }
                    _ => {
                        log::trace!("Unexpected message: {:?}", message);
                        return Err(SessionHandlerError::UnexpectedMessage);
                    }
                }
            }
            HostSessionState::WaitingReadyToDownloadResp => {
                let (message_buffer, _) = session.rx_message_buffer.as_slices();
                let (consumed_bytes_count, message) = match smartcoaster_messages::decode_framed_message(message_buffer) {
                    Ok(result) => result,
                    Err(FrameError::BufferTooSmall(expected_len)) => {
                        log::trace!("Need {expected_len} bytes to decode");
                        return Ok(session);
                    }
                    Err(e) => return Err(SessionHandlerError::FramingError(e)),
                };
                log::trace!("Consumed {} bytes from rx buffer", consumed_bytes_count);
                session.rx_message_buffer.consume(consumed_bytes_count);

                match message {
                    smartcoaster_messages::BootloaderMessages::ReadyToDownloadResponse(ready_to_download_resp) => {
                        log::trace!("Received ready to download response: {:?}", ready_to_download_resp);
                        session.chunk_size = ready_to_download_resp.desired_chunk_size as usize;
                        session.download_progress.max_chunks = (session.firmware_bytes.len() - 1) as u32 / ready_to_download_resp.desired_chunk_size;
                        session.session_state = HostSessionState::ChunkTransfer;
                        session.tx_valid_bytes_size = 0;
                    }
                    _ => {
                        log::trace!("Unexpected message: {:?}", message);
                        return Err(SessionHandlerError::UnexpectedMessage);
                    }
                }
            }
            HostSessionState::ChunkTransfer => {
                let (message_buffer, _) = session.rx_message_buffer.as_slices();
                let (consumed_bytes_count, message) = match smartcoaster_messages::decode_framed_message(message_buffer) {
                    Ok(result) => result,
                    Err(FrameError::BufferTooSmall(expected_len)) => {
                        log::trace!("Need {expected_len} bytes to decode");
                        return Ok(session);
                    }
                    Err(e) => return Err(SessionHandlerError::FramingError(e)),
                };
                log::trace!("Consumed {} bytes from rx buffer", consumed_bytes_count);
                session.rx_message_buffer.consume(consumed_bytes_count);

                match message {
                    smartcoaster_messages::BootloaderMessages::ChunkReq(chunk_req) => {
                        log::trace!("Received ChunkReq for chunk number: {}", chunk_req.chunk_number);

                        // Calculate byte offset
                        let byte_offset = (chunk_req.chunk_number as usize) * session.chunk_size;

                        // Verify we have data for this chunk
                        if byte_offset >= session.firmware_bytes.len() {
                            log::error!("Chunk request out of bounds: offset {} >= file size {}",
                                                  byte_offset, session.firmware_bytes.len());
                            return Err(SessionHandlerError::ChunkRequestOutOfBounds);
                        }

                        // Prepare chunk data (pad with zeros if needed)
                        let mut chunk_data = [0u8; smartcoaster_messages::bootloader::CHUNK_SIZE];
                        let available_bytes = std::cmp::min(session.chunk_size, session.firmware_bytes.len() - byte_offset);
                        chunk_data[..available_bytes]
                            .copy_from_slice(&session.firmware_bytes[byte_offset..byte_offset + available_bytes]);

                        log::trace!("Sending {} bytes from offset {}", available_bytes, byte_offset);

                        // Build and send ChunkResp
                        let chunk_resp = BootloaderMessagesBuilder::new()
                            .chunk_resp()
                            .chunk_number(chunk_req.chunk_number)
                            .chunk_data(chunk_data)
                            .build();

                        log::trace!("Generating ChunkResp for chunk {}", chunk_req.chunk_number);
                        session.tx_valid_bytes_size =
                            smartcoaster_messages::frame_message(&chunk_resp, &mut session.tx_message_buffer)?;
                        log::trace!("ChunkResp for chunk is {} bytes", session.tx_valid_bytes_size);
                        session.download_progress.current_chunk = chunk_req.chunk_number;
                    }
                    BootloaderMessages::Goodbye(_) => {
                        log::trace!("Received Goodbye message, exiting chunk loop");
                        session.tx_valid_bytes_size = 0;
                        session.session_state = HostSessionState::Done;
                    }
                    _ => {
                        log::trace!("Unexpected message: {:?}", message);
                        return Err(SessionHandlerError::UnexpectedMessage);
                    }
                }
            }
            HostSessionState::Done => {
                return Err(SessionHandlerError::SessionEnded);
            }
        }
        log::trace!("Session actions completed");

        if session.rx_message_buffer.len() > 0 {
            log::trace!("{} more bytes in rx buffer, calling session_handler again", session.rx_message_buffer.len());
            let empty_buffer = [0u8; 0];
            let updated_session = SmartcoasterHostFirmwareLoader::session_handler(session, &empty_buffer)?;
            return Ok(updated_session);
        }

        Ok(session)
    }

    pub fn get_bytes_to_send(session: &mut SmartcoasterHostFirmwareLoader<BUFFER_SIZE>) -> Option<&[u8]> {
        if session.tx_valid_bytes_size > 0 {
            log::trace!("Returning {} bytes to send", session.tx_valid_bytes_size);
            let message_size = session.tx_valid_bytes_size;
            session.tx_valid_bytes_size = 0;
            return Some(&session.tx_message_buffer[..message_size]);
        }
        log::trace!("Nothing to send");
        None
    }

    pub fn get_chunk_progress(session: &SmartcoasterHostFirmwareLoader<BUFFER_SIZE>) -> Progress {
        session.download_progress
    }

    pub fn is_session_ended(session: &SmartcoasterHostFirmwareLoader<BUFFER_SIZE>) -> bool {
        match session.session_state {
            HostSessionState::Done => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(1, 1);
    }
}
