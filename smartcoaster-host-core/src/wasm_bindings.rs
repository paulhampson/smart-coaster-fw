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

// Copyright (C) 2025 Paul Hampson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License version 3 as published by the
// Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.

use wasm_bindgen::prelude::*;
use std::sync::{Arc, Mutex};
use crate::{SmartcoasterHostFirmwareLoader, SessionHandlerError};

const WASM_BUFFER_SIZE: usize = 4096;

#[wasm_bindgen]
pub fn init_logging() {
    wasm_logger::init(
        wasm_logger::Config::new(log::Level::Trace)
            .message_on_new_line()
    );
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct WasmFirmwareLoader {
    session: Arc<Mutex<Option<SmartcoasterHostFirmwareLoader<WASM_BUFFER_SIZE>>>>,
    firmware_data: Arc<Vec<u8>>,
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct WasmProgress {
    pub max_chunks: u32,
    pub current_chunk: u32,
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub enum WasmSessionError {
    FramingError,
    RxBufferNotEnoughSpace,
    UnexpectedMessage,
    IncorrectDeviceMode,
    SessionEnded,
    ChunkRequestOutOfBounds,
}

#[wasm_bindgen]
impl WasmFirmwareLoader {
    /// Create a new firmware loader with the provided firmware bytes
    #[wasm_bindgen(constructor)]
    pub fn new(firmware_bytes: &[u8]) -> WasmFirmwareLoader {
        let firmware_data = Arc::new(firmware_bytes.to_vec());
        WasmFirmwareLoader {
            session: Arc::new(Mutex::new(None)),
            firmware_data,
        }
    }

    /// Initialize the firmware loader session
    pub fn init_session(&mut self) -> Result<(), JsValue> {
        let loader = SmartcoasterHostFirmwareLoader::new((*self.firmware_data).clone());
        *self.session.lock().unwrap() = Some(loader);
        Ok(())
    }

    /// Process incoming bytes from the device and advance session state
    pub fn handle_incoming_bytes(&self, incoming_bytes: &[u8]) -> Result<(), JsValue> {
        let mut session_lock = self.session.lock().unwrap();
        let session_opt = session_lock.take();

        if let Some(session) = session_opt {
            match SmartcoasterHostFirmwareLoader::session_handler(session, incoming_bytes) {
                Ok(updated_session) => {
                    *session_lock = Some(updated_session);
                    log::trace!("Session handler succeeded");
                    Ok(())
                }
                Err(e) => {
                    log::error!("Session handler error: {:?}", e);

                    // Convert error to string message for JavaScript
                    let error_msg = match e {
                        SessionHandlerError::FramingError(fe) => {
                            format!("Framing error: {:?}", fe)
                        }
                        SessionHandlerError::RxBufferNotEnoughSpace => {
                            "RX buffer not enough space".to_string()
                        }
                        SessionHandlerError::UnexpectedMessage => {
                            "Unexpected message from device".to_string()
                        }
                        SessionHandlerError::IncorrectDeviceMode => {
                            "Incorrect device mode".to_string()
                        }
                        SessionHandlerError::SessionEnded => {
                            "Session ended".to_string()
                        }
                        SessionHandlerError::ChunkRequestOutOfBounds => {
                            "Chunk request out of bounds".to_string()
                        }
                    };

                    Err(JsValue::from_str(&error_msg))
                }
            }
        } else {
            Err(JsValue::from_str("Session not initialized"))
        }
    }

    /// Get bytes that need to be sent to the device
    pub fn get_bytes_to_send(&self) -> Option<Vec<u8>> {
        let mut session_lock = self.session.lock().unwrap();

        if let Some(ref mut session) = *session_lock {
            if let Some(bytes) = SmartcoasterHostFirmwareLoader::get_bytes_to_send(session) {
                return Some(bytes.to_vec());
            }
        }
        None
    }

    /// Get current download progress
    pub fn get_progress(&self) -> Option<WasmProgress> {
        let session_lock = self.session.lock().unwrap();

        if let Some(ref session) = *session_lock {
            let progress = SmartcoasterHostFirmwareLoader::get_chunk_progress(session);
            return Some(WasmProgress {
                max_chunks: progress.max_chunks,
                current_chunk: progress.current_chunk,
            });
        }
        None
    }

    /// Check if session has ended
    pub fn is_session_ended(&self) -> bool {
        let session_lock = self.session.lock().unwrap();

        if let Some(ref session) = *session_lock {
            return SmartcoasterHostFirmwareLoader::is_session_ended(session);
        }
        false
    }

    /// Get firmware size in bytes
    pub fn get_firmware_size(&self) -> u32 {
        self.firmware_data.len() as u32
    }
}