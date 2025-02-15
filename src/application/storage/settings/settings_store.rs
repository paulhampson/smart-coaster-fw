use crate::application::storage::settings::{SettingError, SettingValue};
use core::ops::Range;
use defmt::{debug, trace, warn, Debug2Format};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_rp::flash;
use embassy_rp::flash::{Error, Flash};
use embassy_rp::peripherals::FLASH;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use embedded_storage_async::nor_flash::{MultiwriteNorFlash, NorFlash};
use heapless::spsc::Queue;
use heapless::FnvIndexMap;
use sequential_storage::cache::NoCache;
use sequential_storage::map;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

pub type BlockingAsyncFlash =
    BlockingAsync<Flash<'static, FLASH, flash::Async, { crate::FLASH_SIZE }>>;

pub static SETTINGS_STORE: SettingsManagerMutex<Error, BlockingAsyncFlash> =
    Mutex::new(SettingsManager::<Error, BlockingAsyncFlash>::new());

pub type SettingsManagerMutex<E, F> = Mutex<CriticalSectionRawMutex, SettingsManager<E, F>>;

pub async fn wait_for_settings_store_initialisation() {
    trace!("Checking settings_menu initialisation");
    loop {
        {
            let settings = SETTINGS_STORE.lock().await;
            if settings.is_initialized() {
                trace!("Settings now available");
                return;
            }
        }
        Timer::after(Duration::from_millis(200)).await;
    }
}

type SettingKey = u16;
#[repr(u16)]
#[derive(Clone, PartialEq, EnumCount, EnumIter, Debug)]
pub enum StoredSettings {
    WeighingSystemTareOffset(SettingValue) = 0,
    WeighingSystemCalibrationGradient(SettingValue) = 1,
    SystemLedBrightness(SettingValue) = 2,
    SystemDisplayBrightness(SettingValue) = 3,
    WeighingSystemBitsToDiscard(SettingValue) = 4,
    MonitoringTargetType(SettingValue) = 5,
    MonitoringTargetValue(SettingValue) = 6,
    DisplayTimeoutMinutes(SettingValue) = 7,
}

impl StoredSettings {
    pub(crate) fn discriminant(&self) -> u16 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        unsafe { *<*const _>::from(self).cast::<u16>() }
    }

    fn value(&self) -> SettingValue {
        match self {
            StoredSettings::WeighingSystemTareOffset(v) => v.clone(),
            StoredSettings::WeighingSystemCalibrationGradient(v) => v.clone(),
            StoredSettings::SystemLedBrightness(v) => v.clone(),
            StoredSettings::SystemDisplayBrightness(v) => v.clone(),
            StoredSettings::WeighingSystemBitsToDiscard(v) => v.clone(),
            StoredSettings::MonitoringTargetType(v) => v.clone(),
            StoredSettings::MonitoringTargetValue(v) => v.clone(),
            StoredSettings::DisplayTimeoutMinutes(v) => v.clone(),
        }
    }
}

pub struct SettingsManager<E, F>
where
    E: defmt::Format,
    F: NorFlash + MultiwriteNorFlash<Error = E>,
{
    flash: Option<F>,
    storage_range: Option<Range<u32>>,
    flash_cache: NoCache,
    settings_cache: FnvIndexMap<
        SettingKey,
        Option<SettingValue>,
        { StoredSettings::COUNT.next_power_of_two() },
    >,
    settings_initialised: bool,
    save_queue: Queue<StoredSettings, 5>,
}

const DATA_BUFFER_SIZE: usize = 128;

impl<E, F> SettingsManager<E, F>
where
    E: defmt::Format,
    F: MultiwriteNorFlash<Error = E>,
{
    pub const fn new() -> Self {
        Self {
            flash: None,
            storage_range: None,
            flash_cache: NoCache::new(),
            settings_cache: FnvIndexMap::new(),
            settings_initialised: false,
            save_queue: Queue::new(),
        }
    }

    pub async fn initialise(&mut self, flash: F, storage_range: Range<u32>, _page_size: usize) {
        self.flash = Some(flash);
        self.storage_range = Some(storage_range);
        debug!(
            "Settings initialising. Flash address range: 0x{:x} to 0x{:x}, size 0x{:x}",
            self.storage_range.clone().unwrap().start,
            self.storage_range.clone().unwrap().end,
            self.flash.as_ref().unwrap().capacity(),
        );

        for setting in StoredSettings::iter() {
            let value = self.load_setting_from_flash(&setting).await;
            if let Ok(setting_value) = value {
                let _ = self
                    .settings_cache
                    .insert(setting.discriminant(), setting_value);
            } else {
                let _ = self.settings_cache.insert(setting.discriminant(), None);
            }
        }

        if self.settings_cache.iter().all(|(_, v)| v.is_none()) {
            warn!("No settings_menu loaded on initialisation, erasing storage");
            let _ = self
                .clear_data()
                .await
                .map_err(|_| warn!("Clearing data failed"));
        }
        self.settings_initialised = true;
        debug!("Settings initialised");
    }

    pub fn is_initialized(&self) -> bool {
        self.flash.is_some() && self.storage_range.is_some() && self.settings_initialised
    }

    async fn clear_data(&mut self) -> Result<(), SettingError> {
        if !(self.flash.is_some() && self.storage_range.is_some()) {
            warn!("Trying to clear data before storage is configured");
            return Err(SettingError::NotInitialized);
        }
        let flash = self.flash.as_mut().unwrap();
        let storage_range = self.storage_range.clone().unwrap().clone();
        sequential_storage::erase_all(flash, storage_range)
            .await
            .map_err(|e| {
                warn!("Unable to erase settings_menu storage. Error: {:?}", e);
                SettingError::EraseError
            })?;

        Ok(())
    }

    async fn save_setting(&mut self, setting: StoredSettings) -> Result<(), SettingError> {
        // Storage management layer requires a buffer to work with. It must be big enough to
        // serialize the biggest value of your storage type in,
        // rounded up  to word alignment of the flash. Some kinds of internal flash may require
        // this buffer to be aligned in RAM as well.
        let mut data_buffer = [0; DATA_BUFFER_SIZE];

        if !self.is_initialized() {
            warn!("Trying to save settings_menu before initialisation");
            return Err(SettingError::NotInitialized);
        }

        let flash = self.flash.as_mut().unwrap();
        let storage_range = self.storage_range.clone().unwrap().clone();

        let k = setting.discriminant();
        let v = setting.value();
        map::store_item(
            flash,
            storage_range,
            &mut self.flash_cache,
            &mut data_buffer,
            &k,
            &v,
        )
        .await
        .map_err(|e| {
            warn!("Unable to save value. Error {:?}", e);
            SettingError::SaveError
        })?;

        let _ = self.settings_cache.insert(setting.discriminant(), Some(v));
        debug!("Setting saved - {}", Debug2Format(&setting));

        Ok(())
    }

    pub async fn process_queued_saves(&mut self) -> Result<(), SettingError> {
        while let Some(item) = self.save_queue.dequeue() {
            self.save_setting(item).await?;
        }
        Ok(())
    }

    async fn load_setting_from_flash(
        &mut self,
        setting: &StoredSettings,
    ) -> Result<Option<SettingValue>, SettingError> {
        let mut data_buffer = [0; DATA_BUFFER_SIZE];

        if !(self.flash.is_some() && self.storage_range.is_some()) {
            warn!("Trying to load settings_menu before initialisation");
            return Err(SettingError::NotInitialized);
        }

        let flash = self.flash.as_mut().unwrap();
        let storage_range = self.storage_range.clone().unwrap().clone();

        let value = map::fetch_item::<u16, SettingValue, _>(
            flash,
            storage_range,
            &mut self.flash_cache,
            &mut data_buffer,
            &setting.discriminant(),
        )
        .await
        .map_err(|e| {
            warn!("Unable to load setting. Error: {:?}", e);
            SettingError::RetrieveError
        })?;
        Ok(value)
    }

    pub fn get_setting(&self, setting_id: u16) -> Option<SettingValue> {
        self.settings_cache
            .get(&setting_id)
            .cloned()
            .unwrap_or(None)
    }

    pub fn queue_settings_save(&mut self, setting: StoredSettings) -> Result<(), SettingError> {
        self.save_queue
            .enqueue(setting)
            .map_err(|_| SettingError::SaveQueueFull)
    }
}
