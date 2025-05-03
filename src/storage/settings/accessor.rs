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

use crate::storage::settings::messaging::{SettingData, SettingsMessage};
use crate::storage::settings::settings_store::{StoredSettings, SETTINGS_STORE};
use crate::storage::settings::{SettingError, SettingValue, SettingsAccessor, SettingsAccessorId};
use crate::storage::storage_manager::wait_for_storage_initialisation;
use defmt::{error, trace};
use embassy_time::{Duration, Timer};

pub struct FlashSettingsAccessor {}

impl FlashSettingsAccessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl SettingsAccessor for FlashSettingsAccessor {
    type Error = SettingError;

    async fn get_setting(&self, setting: SettingsAccessorId) -> Option<SettingValue> {
        wait_for_settings_initialisation().await;
        let settings = SETTINGS_STORE.lock().await;
        match setting {
            SettingsAccessorId::SystemLedBrightness => settings.get_setting(
                StoredSettings::SystemLedBrightness(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::SystemDisplayBrightness => settings.get_setting(
                StoredSettings::SystemDisplayBrightness(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::WeighingSystemTareOffset => settings.get_setting(
                StoredSettings::WeighingSystemTareOffset(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::WeighingSystemCalibrationGradient => settings.get_setting(
                StoredSettings::WeighingSystemCalibrationGradient(SettingValue::Default)
                    .discriminant(),
            ),
            SettingsAccessorId::WeighingSystemBitsToDiscard => settings.get_setting(
                StoredSettings::WeighingSystemBitsToDiscard(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::MonitoringTargetType => settings.get_setting(
                StoredSettings::MonitoringTargetType(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::MonitoringTargetDaily => settings.get_setting(
                StoredSettings::MonitoringTargetDaily(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::DisplayTimeoutMinutes => settings.get_setting(
                StoredSettings::DisplayTimeoutMinutes(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::MonitoringDailyTargetTime => settings.get_setting(
                StoredSettings::MonitoringDailyTargetTime(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::MonitoringTargetHourly => settings.get_setting(
                StoredSettings::MonitoringTargetHourly(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::MonitoringDisplayIndex => settings.get_setting(
                StoredSettings::MonitoringDisplayIndex(SettingValue::Default).discriminant(),
            ),
        }
    }

    async fn save_setting(
        &self,
        setting: SettingsAccessorId,
        value: SettingValue,
    ) -> Result<(), Self::Error> {
        let setting_obj = match setting {
            SettingsAccessorId::SystemLedBrightness => StoredSettings::SystemLedBrightness(value),
            SettingsAccessorId::SystemDisplayBrightness => {
                StoredSettings::SystemDisplayBrightness(value)
            }
            SettingsAccessorId::WeighingSystemTareOffset => {
                StoredSettings::WeighingSystemTareOffset(value)
            }
            SettingsAccessorId::WeighingSystemCalibrationGradient => {
                StoredSettings::WeighingSystemCalibrationGradient(value)
            }
            SettingsAccessorId::WeighingSystemBitsToDiscard => {
                StoredSettings::WeighingSystemBitsToDiscard(value)
            }
            SettingsAccessorId::MonitoringTargetType => StoredSettings::MonitoringTargetType(value),
            SettingsAccessorId::MonitoringTargetDaily => {
                StoredSettings::MonitoringTargetDaily(value)
            }
            SettingsAccessorId::DisplayTimeoutMinutes => {
                StoredSettings::DisplayTimeoutMinutes(value)
            }
            SettingsAccessorId::MonitoringDailyTargetTime => {
                StoredSettings::MonitoringDailyTargetTime(value)
            }
            SettingsAccessorId::MonitoringTargetHourly => {
                StoredSettings::MonitoringTargetHourly(value)
            }
            SettingsAccessorId::MonitoringDisplayIndex => {
                StoredSettings::MonitoringDisplayIndex(value)
            }
        };

        let mut settings = SETTINGS_STORE.lock().await;
        settings.queue_settings_save(setting_obj)?;

        let setting_data = SettingData {
            setting_id: setting,
            value,
        };
        settings.alert_system(SettingsMessage::Change(setting_data));

        Ok(())
    }
}

pub async fn initialise_settings() {
    wait_for_storage_initialisation().await;
    let mut settings = SETTINGS_STORE.lock().await;
    settings.initialise().await;
}

pub async fn wait_for_settings_initialisation() {
    wait_for_storage_initialisation().await;
    loop {
        {
            let settings = SETTINGS_STORE.lock().await;
            if settings.is_initialized() {
                trace!("Settings available");
                return;
            }
        }
        Timer::after(Duration::from_millis(200)).await;
    }
}

pub async fn process_save_queue() {
    let mut settings = SETTINGS_STORE.lock().await;
    let _ = settings
        .process_queued_saves()
        .await
        .map_err(|e| error!("Unable to process queued settings_menu saves - {:?}", e));
}
