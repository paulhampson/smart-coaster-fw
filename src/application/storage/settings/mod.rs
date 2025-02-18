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

use core::fmt::Debug;
use defmt::Format;
use sequential_storage::map::{SerializationError, Value};

pub mod accessor;
pub mod option_types;
mod settings_store;

#[derive(Debug, Format)]
pub enum SettingError {
    SaveError,
    RetrieveError,
    NotInitialized,
    EraseError,
    SaveQueueFull,
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
    MonitoringTargetValue,
    DisplayTimeoutMinutes,
}

impl SettingsAccessorId {
    pub fn get_numeric_properties(&self) -> Option<NumericSettingProperties<u32>> {
        match self {
            SettingsAccessorId::MonitoringTargetValue => Some(NumericSettingProperties::<u32> {
                minimum_value: 0,
                maximum_value: 10000,
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
    async fn get_setting(&self, id: SettingsAccessorId) -> Option<SettingValue>;

    /// Save setting value to the settings storage. Will pass back a storage error if it is unable
    /// to complete the save action. Thread safe.
    async fn save_setting(
        &mut self,
        setting: SettingsAccessorId,
        value: SettingValue,
    ) -> Result<(), Self::Error>;
}

#[repr(u8)]
#[derive(Clone, PartialEq, Debug)]
pub enum SettingValue {
    Default = 0,
    Float(f32),
    SmallUInt(u8),
    UInt(u32),
}

impl SettingValue {
    fn discriminant(&self) -> u8 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}

impl Default for SettingValue {
    fn default() -> Self {
        Self::Default
    }
}

impl Value<'_> for SettingValue {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        let id_byte = self.discriminant();
        let mut value_buffer: [u8; 10] = [0; 10];
        let data_bytes_count = match self {
            SettingValue::Default => 0,
            SettingValue::Float(v) => v.serialize_into(&mut value_buffer)?,
            SettingValue::SmallUInt(v) => {
                value_buffer[0] = *v;
                1
            }
            SettingValue::UInt(v) => v.serialize_into(&mut value_buffer)?,
        };
        let total_serialization_len = data_bytes_count + 1;

        if total_serialization_len > buffer.len() {
            return Err(SerializationError::BufferTooSmall);
        }

        buffer[0] = id_byte;
        if total_serialization_len > 1 {
            buffer[1..total_serialization_len].copy_from_slice(&value_buffer[..data_bytes_count]);
        }

        Ok(total_serialization_len)
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, SerializationError>
    where
        Self: Sized,
    {
        if buffer.len() <= 1 {
            return Err(SerializationError::BufferTooSmall);
        }

        let id_byte = buffer[0];
        let value_buffer = &buffer[1..];

        match id_byte {
            0 => Ok(SettingValue::Default),
            1 => {
                let value = f32::deserialize_from(value_buffer)?;
                Ok(SettingValue::Float(value))
            }
            2 => {
                let value = value_buffer[0];
                Ok(SettingValue::SmallUInt(value))
            }
            3 => {
                let value = u32::deserialize_from(value_buffer)?;
                Ok(SettingValue::UInt(value))
            }
            _ => Err(SerializationError::InvalidFormat),
        }
    }
}
