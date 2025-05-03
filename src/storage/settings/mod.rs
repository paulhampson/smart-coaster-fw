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

use crate::storage::StoredDataValue;
use core::fmt::Debug;
use core::future::Future;
use defmt::Format;

pub mod accessor;
pub mod messaging;
pub mod monitor;
mod settings_store;

#[derive(Debug, Format)]
pub enum SettingError {
    SaveError,
    RetrieveError,
    NotInitialized,
    EraseError,
    SaveQueueFull,
}

#[derive(Debug, Format)]
pub enum StorageError {
    SaveError,
    RetrieveError,
    NotInitialized,
    EraseError,
}

pub struct NumericSettingProperties<T> {
    pub minimum_value: T,
    pub maximum_value: T,
}

#[derive(Debug, Format, Copy, Clone, PartialEq)]
pub enum SettingsAccessorId {
    SystemLedBrightness,
    SystemDisplayBrightness,
    WeighingSystemTareOffset,
    WeighingSystemCalibrationGradient,
    WeighingSystemBitsToDiscard,
    MonitoringTargetType,
    MonitoringTargetDaily,
    DisplayTimeoutMinutes,
    MonitoringDailyTargetTime,
    MonitoringTargetHourly,
    MonitoringDisplayIndex,
}

impl SettingsAccessorId {
    pub fn get_numeric_properties(&self) -> Option<NumericSettingProperties<u32>> {
        match self {
            SettingsAccessorId::MonitoringTargetDaily => Some(NumericSettingProperties::<u32> {
                minimum_value: 0,
                maximum_value: 10000,
            }),
            SettingsAccessorId::MonitoringTargetHourly => Some(NumericSettingProperties::<u32> {
                minimum_value: 0,
                maximum_value: 1000,
            }),
            _ => None,
        }
    }
}

pub trait SettingsAccessor {
    type Error: Debug;

    /// Getting required setting from the settings storage. Will return None if it is not available
    /// in the storage. Expects to wait until the storage has been initialised before getting the
    /// value. Thread safe.
    fn get_setting(
        &self,
        id: SettingsAccessorId,
    ) -> impl Future<Output = Option<SettingValue>> + Send;

    /// Save setting value to the settings storage. Will pass back a storage error if it is unable
    /// to complete the save action. Also notifies interested parties the setting has been changed.
    /// Thread safe.
    fn save_setting(
        &self,
        setting: SettingsAccessorId,
        value: SettingValue,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
pub type SettingValue = StoredDataValue;
