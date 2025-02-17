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

use crate::application::storage::settings::settings_store::{
    wait_for_settings_store_initialisation, BlockingAsyncFlash, StoredSettings, SETTINGS_STORE,
};
use crate::application::storage::settings::{
    SettingError, SettingValue, SettingsAccessor, SettingsAccessorId,
};
use core::ops::Range;
use defmt::error;
pub struct FlashSettingsAccessor {}

impl FlashSettingsAccessor {
    pub fn new() -> Self {
        Self {}
    }
}

impl SettingsAccessor for FlashSettingsAccessor {
    type Error = SettingError;

    async fn get_setting(&self, setting: SettingsAccessorId) -> Option<SettingValue> {
        wait_for_settings_store_initialisation().await;
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
            SettingsAccessorId::MonitoringTargetValue => settings.get_setting(
                StoredSettings::MonitoringTargetValue(SettingValue::Default).discriminant(),
            ),
            SettingsAccessorId::DisplayTimeoutMinutes => settings.get_setting(
                StoredSettings::DisplayTimeoutMinutes(SettingValue::Default).discriminant(),
            ),
        }
    }

    async fn save_setting(
        &mut self,
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
            SettingsAccessorId::MonitoringTargetValue => {
                StoredSettings::MonitoringTargetValue(value)
            }
            SettingsAccessorId::DisplayTimeoutMinutes => {
                StoredSettings::DisplayTimeoutMinutes(value)
            }
        };

        wait_for_settings_store_initialisation().await;
        let mut settings = SETTINGS_STORE.lock().await;
        settings.queue_settings_save(setting_obj)
    }
}

pub async fn initialise_settings_store(
    flash: BlockingAsyncFlash,
    range: Range<u32>,
    page_size: usize,
) {
    let mut settings = SETTINGS_STORE.lock().await;
    settings.initialise(flash, range, page_size).await;
}

pub async fn process_save_queue() {
    let mut settings = SETTINGS_STORE.lock().await;
    let _ = settings
        .process_queued_saves()
        .await
        .map_err(|e| error!("Unable to process queued settings_menu saves - {:?}", e));
}
