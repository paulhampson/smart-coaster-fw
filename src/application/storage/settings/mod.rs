use core::fmt::Debug;
use defmt::Format;
use sequential_storage::map::{SerializationError, Value};

pub mod accessor;
mod settings_store;

#[derive(Debug, Format)]
pub enum SettingError {
    SaveError,
    RetrieveError,
    NotInitialized,
    EraseError,
    SaveQueueFull,
}

#[derive(Debug, Format, Copy, Clone)]
pub enum SettingsAccessorId {
    SystemLedBrightness,
    SystemDisplayBrightness,
    WeighingSystemTareOffset,
    WeighingSystemCalibrationGradient,
    WeighingSystemBitsToDiscard,
}

pub trait SettingsAccessor {
    type Error: Debug;

    async fn get_setting(&self, id: SettingsAccessorId) -> Option<SettingValue>;
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
            _ => Err(SerializationError::InvalidFormat),
        }
    }
}
