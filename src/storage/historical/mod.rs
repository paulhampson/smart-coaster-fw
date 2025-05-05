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

pub mod accessor;
pub mod log_config;
pub mod manager;

use crate::storage::StoredDataValue;
use defmt::{error, Debug2Format};
use sequential_storage::map::{SerializationError, Value};

#[derive(Debug)]
pub enum LogEncodeDecodeError {
    EncodeFailed,
    DecodeFailed,
    BufferTooSmall,
}

pub trait LogEncodeDecode {
    fn encode(&self, buf: &mut [u8]) -> Result<usize, LogEncodeDecodeError>;
    fn decode(&mut self, buf: &mut [u8]) -> Result<(), LogEncodeDecodeError>
    where
        Self: Sized;
}

#[derive(Debug)]
pub struct SimpleLogEntry {
    pub data: StoredDataValue,
}

impl LogEncodeDecode for SimpleLogEntry {
    fn encode(&self, buf: &mut [u8]) -> Result<usize, LogEncodeDecodeError> {
        let data_length = self.data.serialize_into(&mut buf[0..]).map_err(|e| {
            error!(
                "Unable to encode log entry: {:?}. Because {:?}.",
                Debug2Format(&self),
                e
            );
            if e == SerializationError::BufferTooSmall {
                return LogEncodeDecodeError::BufferTooSmall;
            }
            LogEncodeDecodeError::EncodeFailed
        })?;

        Ok(data_length)
    }

    fn decode(&mut self, buf: &mut [u8]) -> Result<(), LogEncodeDecodeError>
    where
        Self: Sized,
    {
        self.data = StoredDataValue::deserialize_from(&buf).map_err(|e| {
            error!("Unable to decode data {}", e);
            LogEncodeDecodeError::DecodeFailed
        })?;
        Ok(())
    }
}
