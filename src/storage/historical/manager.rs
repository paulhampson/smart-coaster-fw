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

use crate::storage::historical::accessor::RetrievedLogEntry;
use crate::storage::historical::messaging::{
    HistoricalLogChannel, HistoricalLogChannelPublisher, HistoricalLogMessage,
};
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
    log_channel: &'static HistoricalLogChannel,
}

impl ReadLogQueueEntry {
    fn new(
        config: &StoredLogConfig,
        start_timestamp: NaiveDateTime,
        log_channel: &'static HistoricalLogChannel,
    ) -> Self {
        Self {
            config: config.clone(),
            start_timestamp,
            log_channel,
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
        log_channel: &'static HistoricalLogChannel,
    ) -> Result<(), StorageError> {
        let queue_entry = ReadLogQueueEntry::new(config, start_timestamp, log_channel);
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
        while let Some(queue_entry) = self.log_read_queue.dequeue() {
            trace!("Processing log read queue request");
            let _ = Self::get_entries_after_timestamp_to_channel(
                &queue_entry.config,
                queue_entry.start_timestamp,
                queue_entry.log_channel.publisher().unwrap(),
            )
            .await?;
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

    async fn get_entries_after_timestamp_to_channel(
        config: &StoredLogConfig,
        target_timestamp: NaiveDateTime,
        publisher: HistoricalLogChannelPublisher<'_>,
    ) -> Result<usize, StorageError> {
        trace!(
            "Getting log entries after {:?}",
            Debug2Format(&target_timestamp)
        );
        let mut temp_buffer = [[0u8; DATA_BUFFER_SIZE]; MAX_READ_CHUNK_SIZE];
        let mut storage = NV_STORAGE.lock().await;
        trace!("Got storage lock");

        // find starting point - this is a naive straight iteration. Could be improved by doing a
        // bisecting search
        let mut chunk_start = 0;
        'find_start: loop {
            let retrieved_count = storage
                .get_log_items(config, chunk_start, MAX_READ_CHUNK_SIZE, &mut temp_buffer)
                .await
                .unwrap_or(0);
            trace!(
                "Start search - retrieved {} items, starting at {}",
                retrieved_count,
                chunk_start
            );
            if retrieved_count == 0 {
                break 'find_start;
            }

            chunk_start += retrieved_count;
            // search backwards in the chunk gives us early exit opportunity
            let mut found_entries_after_start = false;
            for chunk_idx in (0..retrieved_count).rev() {
                let entry = RetrievedLogEntry::from_buffer(&temp_buffer[chunk_idx])?;
                if entry.timestamp < target_timestamp {
                    if found_entries_after_start {
                        break 'find_start; // we found the start point in this chunk
                    } else {
                        trace!("Last entry in chunk is prior to target - early exit");
                        continue 'find_start; // early exit
                    }
                } else {
                    found_entries_after_start = true
                }
                chunk_start -= 1;
                if chunk_idx == 0 {
                    trace!("Found start in first entry of chunk");
                    break 'find_start; // we found the start point in this chunk, it is the first entry
                }
            }
        }
        trace!("Found start point - skipped {} entries", chunk_start);

        let mut total_count = 0;
        'transfer_entries: loop {
            // get the log data and push it over the channel
            let retrieved_count = storage
                .get_log_items(config, chunk_start, MAX_READ_CHUNK_SIZE, &mut temp_buffer)
                .await
                .unwrap_or(0);
            total_count += retrieved_count;
            if retrieved_count == 0 {
                break 'transfer_entries;
            }

            for chunk_idx in 0..retrieved_count {
                let entry = RetrievedLogEntry::from_buffer(&temp_buffer[chunk_idx]);
                if entry.is_err() {
                    error!(
                        "Error parsing record: {}",
                        Debug2Format(&entry.unwrap_err())
                    );
                    let message = HistoricalLogMessage::Error();
                    publisher.publish(message).await;
                    break 'transfer_entries;
                }

                trace!("Sending record");
                let message = HistoricalLogMessage::Record(entry?);
                publisher.publish(message).await;
            }

            chunk_start += retrieved_count;
        }

        trace!("Signalling end of data read");
        let message = HistoricalLogMessage::EndOfRead();
        publisher.publish(message).await;

        Ok(total_count)
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
