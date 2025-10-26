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

use crate::storage::historical::manager::DATA_BUFFER_SIZE;
use crate::storage::settings::StorageError;
use core::cell::RefCell;
use core::future::Future;
use core::ops::Range;
use defmt::{Debug2Format, debug, error, trace, warn};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_embedded_hal::flash::partition::BlockingPartition;
use embassy_rp::flash;
use embassy_rp::peripherals::FLASH;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use embedded_storage_async::nor_flash::NorFlash;
use sequential_storage::cache::NoCache;
use sequential_storage::map;
use sequential_storage::map::Value;

#[derive(Debug, Clone)]
pub struct StoredLogConfig {
    pub storage_range: Range<u32>,
    pub allow_overwrite_old: bool,
}

pub trait StorageManager {
    fn is_initialized(&self) -> bool;
    fn clear_data(&mut self) -> impl Future<Output = Result<(), StorageError>>;
    fn save_key_value_pair<'d, V: Value<'d>>(
        &mut self,
        key: u16,
        value: V,
    ) -> impl Future<Output = Result<(), StorageError>>;
    fn read_key_value_pair<V>(
        &mut self,
        key: u16,
    ) -> impl Future<Output = Result<Option<V>, StorageError>>
    where
        for<'a> V: Value<'a>;

    fn write_log_data(
        &mut self,
        config: &StoredLogConfig,
        data: &[u8],
    ) -> impl Future<Output = Result<(), StorageError>>;

    fn clear_log_data(
        &mut self,
        config: &StoredLogConfig,
    ) -> impl Future<Output = Result<(), StorageError>>;

    fn get_space_remaining(
        &mut self,
        config: &StoredLogConfig,
    ) -> impl Future<Output = Result<u32, StorageError>>;

    fn get_log_items(
        &mut self,
        config: &StoredLogConfig,
        start: usize,
        count: usize,
        buf: &mut [[u8; DATA_BUFFER_SIZE]],
    ) -> impl Future<Output = Result<usize, StorageError>>;
}

pub type BlockingFlash =
    embassy_rp::flash::Flash<'static, FLASH, flash::Blocking, { crate::FLASH_SIZE }>;

pub type BlockingAsyncPartition =
    BlockingAsync<BlockingPartition<'static, CriticalSectionRawMutex, BlockingFlash>>;

pub type StorageManagerMutex =
    Mutex<CriticalSectionRawMutex, StorageManagerSequentialStorage<BlockingAsyncPartition>>;

pub static NV_STORAGE: StorageManagerMutex =
    StorageManagerMutex::new(StorageManagerSequentialStorage::new());

pub struct StorageManagerSequentialStorage<F>
where
    F: NorFlash,
{
    flash: Option<F>,
    key_value_range: Option<Range<u32>>,
    flash_cache: NoCache,
    storage_initialised: bool,
}

impl<F> StorageManagerSequentialStorage<F>
where
    F: NorFlash,
{
    pub const fn new() -> Self {
        Self {
            flash: None,
            key_value_range: None,
            flash_cache: NoCache::new(),
            storage_initialised: false,
        }
    }

    async fn initialise(&mut self, flash: F, settings_range_in_partition: Range<u32>) {
        self.flash = Some(flash);
        self.key_value_range = Some(settings_range_in_partition);
        debug!(
            "Storage initialising. KeyValue flash address range: 0x{:x} to 0x{:x}, flash size: {}",
            self.key_value_range.clone().unwrap().start,
            self.key_value_range.clone().unwrap().end,
            self.flash.as_ref().unwrap().capacity(),
        );

        self.storage_initialised = true;
        debug!("Storage initialised");
    }
}

impl<F> StorageManager for StorageManagerSequentialStorage<F>
where
    F: NorFlash,
{
    fn is_initialized(&self) -> bool {
        trace!(
            "Flash set: {}, storage range set: {}, init flag: {}",
            self.flash.is_some(),
            self.key_value_range.is_some(),
            self.storage_initialised
        );
        self.flash.is_some() && self.key_value_range.is_some() && self.storage_initialised
    }

    async fn clear_data(&mut self) -> Result<(), StorageError> {
        if !(self.flash.is_some() && self.key_value_range.is_some()) {
            warn!("Trying to clear data before storage is configured");
            return Err(StorageError::NotInitialized);
        }
        let flash = self.flash.as_mut().unwrap();
        let storage_range = self.key_value_range.clone().unwrap().clone();
        sequential_storage::erase_all(flash, storage_range)
            .await
            .map_err(|_| {
                warn!("Unable to erase storage.");
                StorageError::EraseError
            })?;

        Ok(())
    }

    async fn save_key_value_pair<'d, V: Value<'d>>(
        &mut self,
        key: u16,
        value: V,
    ) -> Result<(), StorageError> {
        // Storage management layer requires a buffer to work with. It must be big enough to
        // serialize the biggest value of your storage type in,
        // rounded up  to word alignment of the flash. Some kinds of internal flash may require
        // this buffer to be aligned in RAM as well.
        let mut data_buffer = [0; DATA_BUFFER_SIZE];

        if !self.is_initialized() {
            error!("Trying to save to storage before initialisation");
            return Err(StorageError::NotInitialized);
        }

        let flash = self.flash.as_mut().unwrap();
        let storage_range = self.key_value_range.clone().unwrap().clone();

        map::store_item(
            flash,
            storage_range,
            &mut self.flash_cache,
            &mut data_buffer,
            &key,
            &value,
        )
        .await
        .map_err(|_| {
            warn!("Unable to save key/value.");
            StorageError::SaveError
        })?;

        Ok(())
    }

    async fn read_key_value_pair<V>(&mut self, key: u16) -> Result<Option<V>, StorageError>
    where
        for<'a> V: Value<'a>,
    {
        let mut data_buffer = [0; DATA_BUFFER_SIZE];

        if !(self.flash.is_some() && self.key_value_range.is_some()) {
            error!("Trying to load from storage before initialisation");
            return Err(StorageError::NotInitialized);
        }

        let flash = self.flash.as_mut().unwrap();
        let storage_range = self.key_value_range.clone().unwrap().clone();

        let value = map::fetch_item(
            flash,
            storage_range,
            &mut self.flash_cache,
            &mut data_buffer,
            &key,
        )
        .await
        .map_err(|_| {
            warn!("Unable to read key value pair data.");
            StorageError::RetrieveError
        })?;
        Ok(value)
    }

    async fn write_log_data(
        &mut self,
        config: &StoredLogConfig,
        data: &[u8],
    ) -> Result<(), StorageError> {
        if !self.flash.is_some() {
            error!("Trying to read from storage before initialisation");
            return Err(StorageError::NotInitialized);
        }

        let flash = self.flash.as_mut().unwrap();
        sequential_storage::queue::push(
            flash,
            config.storage_range.clone(),
            &mut NoCache::new(),
            data,
            config.allow_overwrite_old,
        )
        .await
        .map_err(|e| {
            warn!("Unable to write log data: {:?}", Debug2Format(&e));
            StorageError::SaveError
        })?;
        Ok(())
    }

    async fn clear_log_data(&mut self, config: &StoredLogConfig) -> Result<(), StorageError> {
        if !self.flash.is_some() {
            error!("Trying to read from storage before initialisation");
            return Err(StorageError::NotInitialized);
        }

        let flash = self.flash.as_mut().unwrap();
        sequential_storage::erase_all(flash, config.storage_range.clone())
            .await
            .map_err(|_| {
                error!("Unable to erase data.");
                StorageError::EraseError
            })?;
        Ok(())
    }

    async fn get_space_remaining(&mut self, config: &StoredLogConfig) -> Result<u32, StorageError> {
        let flash = self.flash.as_mut().unwrap();
        sequential_storage::queue::space_left(
            flash,
            config.storage_range.clone(),
            &mut NoCache::new(),
        )
        .await
        .map_err(|_| {
            warn!("Unable to get remaining space.");
            StorageError::CapacityCheckError
        })
    }

    async fn get_log_items(
        &mut self,
        config: &StoredLogConfig,
        start: usize,
        count: usize,
        buf: &mut [[u8; DATA_BUFFER_SIZE]],
    ) -> Result<usize, StorageError> {
        trace!("Getting log items start = {}, max count = {}", start, count);
        let flash = self.flash.as_mut().unwrap();
        let mut cache = NoCache::new();
        let mut storage_iter =
            sequential_storage::queue::iter(flash, config.storage_range.clone(), &mut cache)
                .await
                .map_err(|_| {
                    error!("Unable to get iterator for NVM queue");
                    StorageError::RetrieveError
                })?;

        let mut retrieved_count = 0;
        let mut index = 0;

        // skip entries
        while index < start {
            let mut temp_buf = [0; DATA_BUFFER_SIZE];
            let entry = storage_iter.next(&mut temp_buf).await.map_err(|_| {
                error!("Failed to read while skipping entries");
                StorageError::RetrieveError
            })?;
            if entry.is_none() {
                break;
            }
            index += 1;
        }

        // retrieve entries
        while index < start + count {
            let entry = storage_iter
                .next(&mut buf[retrieved_count])
                .await
                .map_err(|_| {
                    error!("Failed to read while retrieving entries");
                    StorageError::RetrieveError
                })?;
            if entry.is_none() {
                break;
            }
            retrieved_count += 1;
            index += 1;
        }

        trace!("Retrieved {} entries", retrieved_count);
        Ok(retrieved_count)
    }
}

pub async fn initialise_storage(
    flash_mutex: &'static mut embassy_sync::blocking_mutex::Mutex<
        CriticalSectionRawMutex,
        RefCell<BlockingFlash>,
    >,
    partition_range_in_flash: Range<u32>,
    settings_range_in_partition: Range<u32>,
) {
    let nvm_partition = BlockingPartition::new(
        flash_mutex,
        partition_range_in_flash.start,
        partition_range_in_flash.len() as u32,
    );
    let blocking_async_nvm_partition = BlockingAsync::new(nvm_partition);

    let mut nvm_storage = NV_STORAGE.lock().await;

    nvm_storage
        .initialise(blocking_async_nvm_partition, settings_range_in_partition)
        .await;
}

pub async fn wait_for_storage_initialisation() {
    trace!("Checking storage initialisation");
    loop {
        {
            let storage = NV_STORAGE.lock().await;
            if storage.is_initialized() {
                trace!("Storage now available");
                return;
            }
        }
        Timer::after(Duration::from_millis(200)).await;
    }
}
