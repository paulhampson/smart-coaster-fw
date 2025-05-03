use crate::storage::historical::LogEntry;
use crate::storage::settings::StorageError;
use crate::storage::storage_manager::{StorageManager, StoredLogConfig, NV_STORAGE};
use chrono::{Datelike, Timelike};
use defmt::{debug, error, Debug2Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use heapless::spsc::Queue;
use sequential_storage::map::Value;

const DATA_BUFFER_SIZE: usize = 128;

pub static LOG_STORE: LogManagerMutex = Mutex::new(HistoricalLogManager::new());

pub type LogManagerMutex = Mutex<CriticalSectionRawMutex, HistoricalLogManager>;

#[derive(Debug)]
struct LogQueueEntry {
    config: StoredLogConfig,
    entry: LogEntry,
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
        entry: LogEntry,
    ) -> Result<(), StorageError> {
        let queue_entry = LogQueueEntry {
            config: config.clone(),
            entry,
        };
        self.log_queue.enqueue(queue_entry).map_err(|e| {
            error!("Error writing log queue entry: {:?}", Debug2Format(&e));
            StorageError::SaveError
        })?;
        Ok(())
    }

    pub async fn process_queue(&mut self) -> Result<(), StorageError> {
        while let Some(queue_entry) = self.log_queue.dequeue() {
            self.write_entry(queue_entry.config, queue_entry.entry)
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
        entry: LogEntry,
    ) -> Result<(), StorageError> {
        debug!("Writing log entry: {:?}", Debug2Format(&entry));
        let mut data_buffer = [0u8; DATA_BUFFER_SIZE];

        // Date part
        data_buffer[0] = entry.timestamp.year() as u8; // Lower 8 bits of year
        data_buffer[1] = (entry.timestamp.year() >> 8) as u8; // Upper 8 bits of year
        data_buffer[2] = entry.timestamp.month() as u8;
        data_buffer[3] = entry.timestamp.day() as u8;

        // Time part
        data_buffer[4] = entry.timestamp.hour() as u8;
        data_buffer[5] = entry.timestamp.minute() as u8;
        data_buffer[6] = entry.timestamp.second() as u8;

        // Nanoseconds
        let nanos = entry.timestamp.nanosecond();
        data_buffer[7..10].copy_from_slice(&nanos.to_le_bytes()[..3]); // Using only 3 bytes for nanos

        let mut data_length = 10;
        data_length += entry
            .data
            .serialize_into(&mut data_buffer[data_length..])
            .unwrap();

        let mut storage = NV_STORAGE.lock().await;
        storage
            .write_log_data(&config, &data_buffer[0..data_length])
            .await
            .map_err(|e| {
                error!("Error writing log entry: {:?}", e);
                StorageError::SaveError
            })?;
        Ok(())
    }
}

pub async fn process_log_write_queue() {
    let mut log_store = LOG_STORE.lock().await;
    log_store.process_queue().await.unwrap_or_else(|e| {
        error!("Error processing log queue: {}", e);
    });
}
