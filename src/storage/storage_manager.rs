use crate::storage::settings::StorageError;
use core::ops::Range;
use defmt::{debug, error, trace, warn};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_rp::flash;
use embassy_rp::flash::{Error, Flash};
use embassy_rp::peripherals::FLASH;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use embedded_storage_async::nor_flash::{MultiwriteNorFlash, NorFlash};
use sequential_storage::cache::NoCache;
use sequential_storage::map;
use sequential_storage::map::Value;

pub trait StorageManager {
    fn is_initialized(&self) -> bool;
    async fn clear_data(&mut self) -> Result<(), StorageError>;
    async fn save_key_value_pair<'d, V: Value<'d>>(
        &mut self,
        key: u16,
        value: V,
    ) -> Result<(), StorageError>;

    async fn read_key_value_pair<V>(&mut self, key: u16) -> Result<Option<V>, StorageError>
    where
        for<'a> V: Value<'a>;
}

pub type BlockingAsyncFlash =
    BlockingAsync<Flash<'static, FLASH, flash::Async, { crate::FLASH_SIZE }>>;

pub type StorageManagerMutex =
    Mutex<CriticalSectionRawMutex, StorageManagerSequentialStorage<Error, BlockingAsyncFlash>>;

pub static NV_STORAGE: StorageManagerMutex = Mutex::<
    CriticalSectionRawMutex,
    StorageManagerSequentialStorage<Error, BlockingAsyncFlash>,
>::new(StorageManagerSequentialStorage::new());

pub struct StorageManagerSequentialStorage<E, F>
where
    E: defmt::Format,
    F: NorFlash + MultiwriteNorFlash<Error = E>,
{
    flash: Option<F>,
    storage_range: Option<Range<u32>>,
    flash_cache: NoCache,
    storage_initialised: bool,
}

const DATA_BUFFER_SIZE: usize = 128;

impl<E, F> StorageManagerSequentialStorage<E, F>
where
    E: defmt::Format,
    F: MultiwriteNorFlash<Error = E>,
{
    pub const fn new() -> Self {
        Self {
            flash: None,
            storage_range: None,
            flash_cache: NoCache::new(),
            storage_initialised: false,
        }
    }

    async fn initialise(&mut self, flash: F, storage_range: Range<u32>, _page_size: usize) {
        self.flash = Some(flash);
        self.storage_range = Some(storage_range);
        debug!(
            "Storage initialising. Flash address range: 0x{:x} to 0x{:x}, size 0x{:x}",
            self.storage_range.clone().unwrap().start,
            self.storage_range.clone().unwrap().end,
            self.flash.as_ref().unwrap().capacity(),
        );

        self.storage_initialised = true;
        debug!("Storage initialised");
    }
}

impl<E, F> StorageManager for StorageManagerSequentialStorage<E, F>
where
    E: defmt::Format,
    F: MultiwriteNorFlash<Error = E>,
{
    fn is_initialized(&self) -> bool {
        trace!(
            "Flash set: {}, storage range set: {}, init flag: {}",
            self.flash.is_some(),
            self.storage_range.is_some(),
            self.storage_initialised
        );
        self.flash.is_some() && self.storage_range.is_some() && self.storage_initialised
    }

    async fn clear_data(&mut self) -> Result<(), StorageError> {
        if !(self.flash.is_some() && self.storage_range.is_some()) {
            warn!("Trying to clear data before storage is configured");
            return Err(StorageError::NotInitialized);
        }
        let flash = self.flash.as_mut().unwrap();
        let storage_range = self.storage_range.clone().unwrap().clone();
        sequential_storage::erase_all(flash, storage_range)
            .await
            .map_err(|e| {
                warn!("Unable to erase storage. Error: {:?}", e);
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
        let storage_range = self.storage_range.clone().unwrap().clone();

        map::store_item(
            flash,
            storage_range,
            &mut self.flash_cache,
            &mut data_buffer,
            &key,
            &value,
        )
        .await
        .map_err(|e| {
            warn!("Unable to save key/value. Error {:?}", e);
            StorageError::SaveError
        })?;

        Ok(())
    }

    async fn read_key_value_pair<V>(&mut self, key: u16) -> Result<Option<V>, StorageError>
    where
        for<'a> V: Value<'a>,
    {
        let mut data_buffer = [0; DATA_BUFFER_SIZE];

        if !(self.flash.is_some() && self.storage_range.is_some()) {
            error!("Trying to load from storage before initialisation");
            return Err(StorageError::NotInitialized);
        }

        let flash = self.flash.as_mut().unwrap();
        let storage_range = self.storage_range.clone().unwrap().clone();

        let value = map::fetch_item(
            flash,
            storage_range,
            &mut self.flash_cache,
            &mut data_buffer,
            &key,
        )
        .await
        .map_err(|e| {
            warn!("Unable to read key value pair data. Error: {:?}", e);
            StorageError::RetrieveError
        })?;
        Ok(value)
    }
}

pub async fn initialise_storage(flash: BlockingAsyncFlash, range: Range<u32>, page_size: usize) {
    let mut settings = NV_STORAGE.lock().await;
    settings.initialise(flash, range, page_size).await;
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
