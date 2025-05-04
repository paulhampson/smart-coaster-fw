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
        // if buf.len() < 10 {
        //     return Err(LogEncodeDecodeError::BufferTooSmall);
        // }
        //
        // // Date part
        // buf[0] = self.timestamp.year() as u8; // Lower 8 bits of year
        // buf[1] = (self.timestamp.year() >> 8) as u8; // Upper 8 bits of year
        // buf[2] = self.timestamp.month() as u8;
        // buf[3] = self.timestamp.day() as u8;
        //
        // // Time part
        // buf[4] = self.timestamp.hour() as u8;
        // buf[5] = self.timestamp.minute() as u8;
        // buf[6] = self.timestamp.second() as u8;
        //
        // // Nanoseconds
        // let nanos = self.timestamp.nanosecond();
        // buf[7..10].copy_from_slice(&nanos.to_le_bytes()[..3]); // Using only 3 bytes for nanos
        //
        // let mut data_length = 10;
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
        todo!();
        Ok(())
    }
}
