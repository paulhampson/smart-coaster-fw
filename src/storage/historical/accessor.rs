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

use crate::rtc::accessor::RtcAccessor;
use crate::storage::historical::log_config::Logs;
use crate::storage::historical::manager::{DATA_BUFFER_SIZE, LOG_STORE, MAX_READ_CHUNK_SIZE};
use crate::storage::historical::{LogEncodeDecode, SimpleLogEntry};
use crate::storage::settings::StorageError;
use crate::storage::storage_manager::StoredLogConfig;
use crate::storage::StoredDataValue;
use chrono::NaiveDateTime;
use defmt::{warn, Debug2Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

pub type LogReadSignal = Signal<CriticalSectionRawMutex, RetrievedLogChunk>;

#[derive(Debug, Copy, Clone)]
pub struct RetrievedLogEntry {
    pub timestamp: NaiveDateTime,
    pub data: [u8; DATA_BUFFER_SIZE],
}

impl RetrievedLogEntry {
    pub fn from_buffer(buffer: &[u8]) -> Result<Self, StorageError> {
        let timestamp;
        // Extract date components
        let year_lower = buffer[0] as u16;
        let year_upper = buffer[1] as u16;
        let year = year_lower | (year_upper << 8);
        let month = buffer[2] as u32;
        let day = buffer[3] as u32;

        // Extract time components
        let hour = buffer[4] as u32;
        let minute = buffer[5] as u32;
        let second = buffer[6] as u32;

        // Extract nanoseconds (stored in 3 bytes)
        let mut nano_bytes = [0u8; 4];
        nano_bytes[0..3].copy_from_slice(&buffer[7..10]);
        let nano = u32::from_le_bytes(nano_bytes);

        // Create NaiveDate and NaiveTime
        match chrono::NaiveDate::from_ymd_opt(year as i32, month, day) {
            Some(date) => {
                match chrono::NaiveTime::from_hms_nano_opt(hour, minute, second, nano) {
                    Some(time) => {
                        // Combine into a NaiveDateTime
                        timestamp = NaiveDateTime::new(date, time);
                    }
                    None => return Err(StorageError::DecodeError),
                }
            }
            None => return Err(StorageError::DecodeError),
        }

        // create result, copying in the data
        let mut s = Self {
            timestamp,
            data: [0; DATA_BUFFER_SIZE],
        };

        let data_len = s.data.len() - 10;
        s.data[0..data_len].copy_from_slice(&buffer[10..]);
        Ok(s)
    }
}

impl Default for RetrievedLogEntry {
    fn default() -> Self {
        Self {
            timestamp: NaiveDateTime::default(),
            data: [0u8; DATA_BUFFER_SIZE],
        }
    }
}

pub struct RetrievedLogChunk {
    pub entries: [RetrievedLogEntry; MAX_READ_CHUNK_SIZE],
    pub count: usize,
}

pub struct HistoricalLogAccessor {
    log_config: StoredLogConfig,
    rtc_accessor: RtcAccessor,
}

impl HistoricalLogAccessor {
    pub fn new(log: Logs) -> HistoricalLogAccessor {
        HistoricalLogAccessor {
            log_config: log.get_config(),
            rtc_accessor: RtcAccessor::new().unwrap(),
        }
    }

    pub async fn log_simple_data(&mut self, data: StoredDataValue) {
        let mut log_store = LOG_STORE.lock().await;
        let log_entry = SimpleLogEntry { data };

        let _ = log_store
            .queue_write(
                &self.log_config,
                self.rtc_accessor.get_date_time(),
                log_entry,
            )
            .map_err(|e| warn!("Unable to write to log queue: {}", Debug2Format(&e)));
    }

    pub async fn log_data(&mut self, data: impl LogEncodeDecode) {
        let mut log_store = LOG_STORE.lock().await;

        let _ = log_store
            .queue_write(&self.log_config, self.rtc_accessor.get_date_time(), data)
            .map_err(|e| warn!("Unable to write to log queue: {}", Debug2Format(&e)));
    }

    pub async fn get_log_data_after_timestamp(
        &self,
        start_timestamp: NaiveDateTime,
        entry_buffer: [RetrievedLogEntry; MAX_READ_CHUNK_SIZE],
        signal: &'static LogReadSignal,
    ) {
        let mut log_store = LOG_STORE.lock().await;
        let _ = log_store.queue_read(&self.log_config, start_timestamp, entry_buffer, signal);
    }
}
