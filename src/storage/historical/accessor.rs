use crate::rtc::accessor::RtcAccessor;
use crate::storage::historical::log_config::Logs;
use crate::storage::historical::manager::LOG_STORE;
use crate::storage::historical::LogEntry;
use crate::storage::storage_manager::StoredLogConfig;
use crate::storage::StoredDataValue;
use defmt::{warn, Debug2Format};

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

    pub async fn log(&mut self, data: StoredDataValue) {
        let mut log_store = LOG_STORE.lock().await;
        let log_entry = LogEntry {
            timestamp: self.rtc_accessor.get_date_time(),
            data,
        };

        let _ = log_store
            .write_to_queue(&self.log_config, log_entry)
            .map_err(|e| warn!("Unable to write to log queue: {}", Debug2Format(&e)));
    }
}
