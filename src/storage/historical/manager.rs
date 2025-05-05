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

use crate::drink_monitor::drink_monitoring::LOG_READ_SIGNAL;
use crate::storage::historical::accessor::RetrievedLogChunk;
pub(crate) use crate::storage::historical::accessor::{LogReadSignal, RetrievedLogEntry};
use crate::storage::historical::LogEncodeDecode;
use crate::storage::settings::StorageError;
use crate::storage::storage_manager::{StorageManager, StoredLogConfig, NV_STORAGE};
use chrono::{Datelike, NaiveDateTime, Timelike};
use core::fmt::Debug;
use defmt::{debug, error, trace, Debug2Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use heapless::spsc::Queue;

pub static LOG_STORE: LogManagerMutex = Mutex::new(HistoricalLogManager::new());

pub type LogManagerMutex = Mutex<CriticalSectionRawMutex, HistoricalLogManager>;

pub const DATA_BUFFER_SIZE: usize = 64;
pub const MAX_READ_CHUNK_SIZE: usize = 5;

struct ReadLogQueueEntry {
    config: StoredLogConfig,
    start_timestamp: NaiveDateTime,
    buffer: [RetrievedLogEntry; MAX_READ_CHUNK_SIZE],
    // signal: &'static LogReadSignal,
}

impl ReadLogQueueEntry {
    fn new(
        config: &StoredLogConfig,
        start_timestamp: NaiveDateTime,
        buffer: [RetrievedLogEntry; MAX_READ_CHUNK_SIZE],
        //signal: &'static LogReadSignal,
    ) -> Self {
        Self {
            config: config.clone(),
            start_timestamp,
            buffer,
            // signal,
        }
    }
}

#[derive(Debug)]
struct WriteLogQueueEntry {
    config: StoredLogConfig,
    data: [u8; DATA_BUFFER_SIZE],
    entry_size: usize,
}

impl WriteLogQueueEntry {
    fn new(
        config: &StoredLogConfig,
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
            config: config.clone(),
            data,
            entry_size,
        })
    }
}

pub struct HistoricalLogManager {
    log_write_queue: Queue<WriteLogQueueEntry, 8>,
    log_read_queue: Queue<ReadLogQueueEntry, 4>,
}

impl HistoricalLogManager {
    pub const fn new() -> Self {
        Self {
            log_write_queue: Queue::new(),
            log_read_queue: Queue::new(),
        }
    }

    pub fn queue_write(
        &mut self,
        config: &StoredLogConfig,
        timestamp: NaiveDateTime,
        entry: impl LogEncodeDecode,
    ) -> Result<(), StorageError> {
        let queue_entry = WriteLogQueueEntry::new(config, timestamp, entry)?;

        self.log_write_queue.enqueue(queue_entry).map_err(|e| {
            error!("Error writing log queue entry: {:?}", Debug2Format(&e));
            StorageError::SaveError
        })?;
        Ok(())
    }

    pub fn queue_read(
        &mut self,
        config: &StoredLogConfig,
        start_timestamp: NaiveDateTime,
        buffer: [RetrievedLogEntry; MAX_READ_CHUNK_SIZE],
        //signal: &'static LogReadSignal,
    ) -> Result<(), StorageError> {
        let queue_entry = ReadLogQueueEntry::new(config, start_timestamp, buffer /*signal*/);
        trace!("Queueing storage read");
        self.log_read_queue.enqueue(queue_entry).map_err(|_| {
            error!("Error writing read log queue request entry");
            StorageError::RetrieveError
        })?;
        Ok(())
    }

    pub async fn process_write_queue(&mut self) -> Result<(), StorageError> {
        while let Some(queue_entry) = self.log_write_queue.dequeue() {
            trace!("Processing log write queue request");
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

    pub async fn process_read_queue(&mut self) -> Result<(), StorageError> {
        while let Some(mut queue_entry) = self.log_read_queue.dequeue() {
            trace!("Processing log read queue request");
            let retrieved_count = Self::get_entries_after_timestamp(
                &queue_entry.config,
                queue_entry.start_timestamp,
                &mut queue_entry.buffer,
            )
            .await?;
            let retrieved_chunk = RetrievedLogChunk {
                count: retrieved_count,
                entries: queue_entry.buffer,
            };
            // queue_entry.signal.signal(retrieved_chunk);
            // FIXME
            LOG_READ_SIGNAL.signal(retrieved_chunk);
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

    async fn get_entries_after_timestamp(
        config: &StoredLogConfig,
        timestamp: NaiveDateTime,
        entry_data: &mut [RetrievedLogEntry],
    ) -> Result<usize, StorageError> {
        trace!("Geting log entries after {:?}", Debug2Format(&timestamp));
        let mut temp_buffer = [[0u8; DATA_BUFFER_SIZE]; MAX_READ_CHUNK_SIZE];
        let mut storage = NV_STORAGE.lock().await;

        // find starting point
        let mut chunk_start = 0;
        'find_start: loop {
            let retrieved_count = storage
                .get_log_items(config, chunk_start, MAX_READ_CHUNK_SIZE, &mut temp_buffer)
                .await
                .unwrap_or(0);
            if retrieved_count == 0 {
                break 'find_start;
            }

            for chunk_idx in 0..retrieved_count {
                let entry = RetrievedLogEntry::from_buffer(&temp_buffer[chunk_idx])?;
                if entry.timestamp >= timestamp {
                    break 'find_start;
                }
                chunk_start += 1;
            }
        }
        trace!("Skipped {} entries", chunk_start);

        // get the log data
        let retrieved_count = storage
            .get_log_items(config, chunk_start, entry_data.len(), &mut temp_buffer)
            .await
            .unwrap_or(0);

        for chunk_idx in 0..retrieved_count {
            entry_data[chunk_idx] = RetrievedLogEntry::from_buffer(&temp_buffer[chunk_idx])?;
        }

        Ok(retrieved_count)
    }
}

pub async fn process_log_queues() {
    let mut log_store = LOG_STORE.lock().await;
    log_store.process_write_queue().await.unwrap_or_else(|e| {
        error!("Error processing write log queue: {}", e);
    });

    log_store.process_read_queue().await.unwrap_or_else(|e| {
        error!("Error processing read log queue: {}", e);
    });
}
