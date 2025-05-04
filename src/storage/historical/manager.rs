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

use crate::storage::historical::LogEncodeDecode;
use crate::storage::settings::StorageError;
use crate::storage::storage_manager::{StorageManager, StoredLogConfig, NV_STORAGE};
use chrono::{Datelike, NaiveDateTime, Timelike};
use core::fmt::Debug;
use defmt::{debug, error, Debug2Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use heapless::spsc::Queue;

pub static LOG_STORE: LogManagerMutex = Mutex::new(HistoricalLogManager::new());

pub type LogManagerMutex = Mutex<CriticalSectionRawMutex, HistoricalLogManager>;

const DATA_BUFFER_SIZE: usize = 128;

#[derive(Debug)]
struct LogQueueEntry {
    config: StoredLogConfig,
    data: [u8; DATA_BUFFER_SIZE],
    entry_size: usize,
}

impl LogQueueEntry {
    fn new(
        config: StoredLogConfig,
        timestamp: NaiveDateTime,
        entry: impl LogEncodeDecode,
    ) -> Result<Self, StorageError> {
        let mut data = [0; DATA_BUFFER_SIZE];

        // Date part
        data[0] = timestamp.year() as u8; // Lower 8 bits of year
        data[1] = (timestamp.year() >> 8) as u8; // Upper 8 bits of year
        data[2] = timestamp.month() as u8;
        data[3] = timestamp.day() as u8;

        // Time part
        data[4] = timestamp.hour() as u8;
        data[5] = timestamp.minute() as u8;
        data[6] = timestamp.second() as u8;

        // Nanoseconds
        let nanos = timestamp.nanosecond();
        data[7..10].copy_from_slice(&nanos.to_le_bytes()[..3]); // Using only 3 bytes for nanos

        let ts_length = 10;

        let entry_size = ts_length
            + entry.encode(&mut data[ts_length..]).map_err(|_| {
                error!("Failed to encode.");
                StorageError::SaveError
            })?;

        Ok(Self {
            config,
            data,
            entry_size,
        })
    }
}

pub struct HistoricalLogManager {
    log_queue: Queue<LogQueueEntry, 8>,
}

impl HistoricalLogManager {
    pub const fn new() -> Self {
        Self {
            log_queue: Queue::new(),
        }
    }

    pub fn write_to_queue(
        &mut self,
        config: &StoredLogConfig,
        timestamp: NaiveDateTime,
        entry: impl LogEncodeDecode,
    ) -> Result<(), StorageError> {
        let queue_entry = LogQueueEntry::new(config.clone(), timestamp, entry)?;

        self.log_queue.enqueue(queue_entry).map_err(|e| {
            error!("Error writing log queue entry: {:?}", Debug2Format(&e));
            StorageError::SaveError
        })?;
        Ok(())
    }

    pub async fn process_queue(&mut self) -> Result<(), StorageError> {
        while let Some(queue_entry) = self.log_queue.dequeue() {
            self.write_entry(queue_entry.config, queue_entry.data, queue_entry.entry_size)
                .await
                .map_err(|e| {
                    error!(
                        "Error while processing log queue entry {:?}",
                        Debug2Format(&e)
                    );
                    StorageError::SaveError
                })?;
        }
        Ok(())
    }

    async fn write_entry(
        &self,
        config: StoredLogConfig,
        data: [u8; DATA_BUFFER_SIZE],
        data_size: usize,
    ) -> Result<(), StorageError> {
        debug!("Writing log entry - {} bytes", data_size);
        let mut storage = NV_STORAGE.lock().await;
        storage
            .write_log_data(&config, &data[0..data_size])
            .await
            .map_err(|e| {
                error!("Error writing log entry: {:?}", e);
                StorageError::SaveError
            })?;
        debug!(
            "Remaining capacity - {} of {} bytes",
            storage.get_space_remaining(&config).await?,
            &config.storage_range.len()
        );
        Ok(())
    }
}

pub async fn process_log_write_queue() {
    let mut log_store = LOG_STORE.lock().await;
    log_store.process_queue().await.unwrap_or_else(|e| {
        error!("Error processing log queue: {}", e);
    });
}
