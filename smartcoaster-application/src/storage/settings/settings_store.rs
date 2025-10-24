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

use crate::storage::settings::messaging::SettingsMessage;
use crate::storage::settings::messaging::{SettingsChannelPublisher, SETTINGS_CHANNEL};
use crate::storage::settings::{SettingError, SettingValue};
use crate::storage::storage_manager::{StorageManager, NV_STORAGE};
use defmt::{debug, warn, Debug2Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use heapless::spsc::Queue;
use heapless::FnvIndexMap;
use strum::{EnumCount, EnumIter, IntoEnumIterator};

pub static SETTINGS_STORE: SettingsManagerMutex = Mutex::new(SettingsManager::new());

pub type SettingsManagerMutex = Mutex<CriticalSectionRawMutex, SettingsManager>;

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
    MonitoringTargetDaily(SettingValue) = 6,
    DisplayTimeoutMinutes(SettingValue) = 7,
    MonitoringDailyTargetTime(SettingValue) = 8,
    MonitoringTargetHourly(SettingValue) = 9,
    MonitoringDisplayIndex(SettingValue) = 10,
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
            StoredSettings::MonitoringTargetDaily(v) => v.clone(),
            StoredSettings::DisplayTimeoutMinutes(v) => v.clone(),
            StoredSettings::MonitoringDailyTargetTime(v) => v.clone(),
            StoredSettings::MonitoringTargetHourly(v) => v.clone(),
            StoredSettings::MonitoringDisplayIndex(v) => v.clone(),
        }
    }
}

pub struct SettingsManager {
    settings_cache: FnvIndexMap<
        SettingKey,
        Option<SettingValue>,
        { StoredSettings::COUNT.next_power_of_two() },
    >,
    settings_initialised: bool,
    save_queue: Queue<StoredSettings, 5>,
    settings_publisher: Option<SettingsChannelPublisher<'static>>,
}

impl SettingsManager {
    pub const fn new() -> Self {
        Self {
            settings_cache: FnvIndexMap::new(),
            settings_initialised: false,
            save_queue: Queue::new(),
            settings_publisher: None,
        }
    }

    pub async fn initialise(&mut self) {
        self.settings_publisher = Some(SETTINGS_CHANNEL.publisher().unwrap());

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

        self.settings_initialised = true;
        debug!("Settings initialised");
    }

    pub fn is_initialized(&self) -> bool {
        self.settings_publisher.is_some() && self.settings_initialised
    }

    async fn save_setting(&mut self, setting: StoredSettings) -> Result<(), SettingError> {
        if !self.is_initialized() {
            warn!("Trying to save settings_menu before initialisation");
            return Err(SettingError::NotInitialized);
        }

        {
            let mut storage = NV_STORAGE.lock().await;
            storage
                .save_key_value_pair(setting.discriminant(), setting.value())
                .await
                .map_err(|e| {
                    warn!("Unable to save setting. Error: {:?}", e);
                    SettingError::SaveError
                })?;
        }

        let _ = self
            .settings_cache
            .insert(setting.discriminant(), Some(setting.value()));
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
        // this can be called before the settings themselves are initialised, the only requirement
        // is that storage is initialised.

        let mut storage = NV_STORAGE.lock().await;
        if !storage.is_initialized() {
            warn!("Called load setting prior to storage being initialised.");
            return Err(SettingError::NotInitialized);
        }

        let value = storage
            .read_key_value_pair::<SettingValue>(setting.discriminant())
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

    pub fn alert_system(&mut self, message: SettingsMessage) {
        if let Some(publisher) = &self.settings_publisher {
            publisher.publish_immediate(message);
        }
    }
}
