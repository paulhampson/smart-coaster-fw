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

use chrono::{Datelike, Timelike};
use core::fmt::Debug;
use core::future::Future;
use defmt::Format;
use sequential_storage::map::{SerializationError, Value};

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

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SettingValue {
    Default = 0,
    Float(f32),
    SmallUInt(u8),
    UInt(u32),
    Time(chrono::NaiveTime),
    DateTime(chrono::NaiveDateTime),
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
                1 // bytes used
            }
            SettingValue::UInt(v) => v.serialize_into(&mut value_buffer)?,
            SettingValue::Time(v) => {
                value_buffer[0] = v.hour() as u8;
                value_buffer[1] = v.minute() as u8;
                value_buffer[2] = v.second() as u8;
                let nanos = v.nanosecond();
                value_buffer[3..7].copy_from_slice(&nanos.to_le_bytes());
                // Return the number of bytes used
                7
            }
            SettingValue::DateTime(v) => {
                // Date part
                value_buffer[0] = v.year() as u8; // Lower 8 bits of year
                value_buffer[1] = (v.year() >> 8) as u8; // Upper 8 bits of year
                value_buffer[2] = v.month() as u8;
                value_buffer[3] = v.day() as u8;

                // Time part
                value_buffer[4] = v.hour() as u8;
                value_buffer[5] = v.minute() as u8;
                value_buffer[6] = v.second() as u8;

                // Nanoseconds
                let nanos = v.nanosecond();
                value_buffer[7..].copy_from_slice(&nanos.to_le_bytes()[..3]); // Using only 3 bytes for nanos

                // Return the number of bytes used
                10
            }
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
                // float32
                let value = f32::deserialize_from(value_buffer)?;
                Ok(SettingValue::Float(value))
            }
            2 => {
                // u8
                if value_buffer.len() < 1 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = value_buffer[0];
                Ok(SettingValue::SmallUInt(value))
            }
            3 => {
                // u32
                let value = u32::deserialize_from(value_buffer)?;
                Ok(SettingValue::UInt(value))
            }
            4 => {
                // NaiveTime
                if value_buffer.len() < 7 {
                    return Err(SerializationError::BufferTooSmall);
                }

                // Extract hour, minute, second
                let hour = value_buffer[0] as u32;
                let minute = value_buffer[1] as u32;
                let second = value_buffer[2] as u32;

                // Extract nanoseconds
                let mut nano_bytes = [0u8; 4];
                nano_bytes.copy_from_slice(&value_buffer[3..7]);
                let nano = u32::from_le_bytes(nano_bytes);

                // Create NaiveTime
                match chrono::NaiveTime::from_hms_nano_opt(hour, minute, second, nano) {
                    Some(time) => Ok(SettingValue::Time(time)),
                    None => Err(SerializationError::InvalidFormat),
                }
            }
            5 => {
                //NaiveDateTime
                if value_buffer.len() < 10 {
                    return Err(SerializationError::BufferTooSmall);
                }

                // Extract date components
                let year_lower = value_buffer[0] as u16;
                let year_upper = value_buffer[1] as u16;
                let year = year_lower | (year_upper << 8);
                let month = value_buffer[2] as u32;
                let day = value_buffer[3] as u32;

                // Extract time components
                let hour = value_buffer[4] as u32;
                let minute = value_buffer[5] as u32;
                let second = value_buffer[6] as u32;

                // Extract nanoseconds (stored in 3 bytes)
                let mut nano_bytes = [0u8; 4];
                nano_bytes[0..3].copy_from_slice(&value_buffer[7..10]);
                let nano = u32::from_le_bytes(nano_bytes);

                // Create NaiveDate and NaiveTime
                match chrono::NaiveDate::from_ymd_opt(year as i32, month, day) {
                    Some(date) => {
                        match chrono::NaiveTime::from_hms_nano_opt(hour, minute, second, nano) {
                            Some(time) => {
                                // Combine into a NaiveDateTime
                                let datetime = chrono::NaiveDateTime::new(date, time);
                                Ok(SettingValue::DateTime(datetime))
                            }
                            None => Err(SerializationError::InvalidFormat),
                        }
                    }
                    None => Err(SerializationError::InvalidFormat),
                }
            }
            _ => Err(SerializationError::InvalidFormat),
        }
    }
}
