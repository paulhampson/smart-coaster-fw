use core::ops::Range;
use embedded_storage_async::nor_flash::{MultiwriteNorFlash, NorFlash};
use heapless::FnvIndexMap;
use sequential_storage::cache::NoCache;
use sequential_storage::map;
use sequential_storage::map::{SerializationError, Value};
use strum::{EnumCount, EnumIter, IntoEnumIterator};

pub enum SettingError {
    SaveError,
    RetrieveError,
}

#[repr(u8)]
#[derive(Clone, PartialEq, Debug)]
pub enum SettingValue {
    Default = 0,
    Float(f32),
    SmallInt(i8),
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
            SettingValue::SmallInt(v) => {
                value_buffer[0] = *v as u8;
                1
            }
        };
        let total_serialization_len = data_bytes_count + 1;

        if total_serialization_len > buffer.len() {
            return Err(SerializationError::BufferTooSmall);
        }

        buffer[0] = id_byte;
        if total_serialization_len > 1 {
            buffer[1..].copy_from_slice(&value_buffer[..data_bytes_count]);
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
            // Assuming 1 represents SmallInt
            1 => {
                let value = value_buffer[0] as i8;
                Ok(SettingValue::SmallInt(value))
            }
            _ => Err(SerializationError::InvalidFormat),
        }
    }
}

type SettingKey = u16;
#[repr(u16)]
#[derive(Clone, PartialEq, EnumCount, EnumIter, Debug)]
pub enum StoredSettings {
    WeighingSystemTareOffset(SettingValue) = 0,
    WeighingSystemCalibrationGradient(SettingValue) = 1,
    SystemLedBrightness(SettingValue) = 2,
}

impl StoredSettings {
    fn discriminant(&self) -> u16 {
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
        }
    }
}

pub struct SettingsManager<E, F>
where
    E: defmt::Format,
    F: NorFlash + MultiwriteNorFlash<Error = E>,
{
    flash: F,
    storage_range: Range<u32>,
    flash_cache: NoCache,
    settings_cache: FnvIndexMap<SettingKey, Option<SettingValue>, { StoredSettings::COUNT }>,
}

const DATA_BUFFER_SIZE: usize = 128;

impl<E, F> SettingsManager<E, F>
where
    E: defmt::Format,
    F: NorFlash + MultiwriteNorFlash<Error = E>,
{
    pub fn new(flash: F, storage_range: Range<u32>, page_size: usize) -> Self {
        Self {
            flash,
            storage_range,
            flash_cache: NoCache::new(),
            settings_cache: FnvIndexMap::new(),
        }
    }

    pub async fn initialise(&mut self) {
        for setting in StoredSettings::iter() {
            let value = self.load_setting(&setting).await;
            if let Ok(setting_value) = value {
                let _ = self
                    .settings_cache
                    .insert(setting.discriminant(), setting_value);
            } else {
                let _ = self.settings_cache.insert(setting.discriminant(), None);
            }
        }
    }

    async fn save_setting(&mut self, setting: StoredSettings) -> Result<(), SettingError> {
        // Storage managment layer requires a buffer to work with. It must be big enough to
        // serialize the biggest value of your storage type in,
        // rounded up  to word alignment of the flash. Some kinds of internal flash may require
        // this buffer to be aligned in RAM as well.
        let mut data_buffer = [0; DATA_BUFFER_SIZE];

        let k = setting.discriminant();
        let v = setting.value();
        map::store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut self.flash_cache,
            &mut data_buffer,
            &k,
            &v,
        )
        .await
        .map_err(|_| SettingError::SaveError)?;

        let _ = self.settings_cache.insert(setting.discriminant(), Some(v));

        Ok(())
    }

    async fn load_setting(
        &mut self,
        setting: &StoredSettings,
    ) -> Result<Option<SettingValue>, SettingError> {
        let mut data_buffer = [0; DATA_BUFFER_SIZE];
        let value = map::fetch_item::<u16, SettingValue, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut self.flash_cache,
            &mut data_buffer,
            &setting.discriminant(),
        )
        .await
        .map_err(|_| SettingError::RetrieveError)?;
        Ok(value)
    }

    async fn get_setting(&mut self, setting_id: u16) -> Option<SettingValue> {
        self.settings_cache
            .get(&setting_id)
            .cloned()
            .unwrap_or(None)
    }

    pub async fn set_weighing_system_tare_offset(
        &mut self,
        offset: f32,
    ) -> Result<(), SettingError> {
        self.save_setting(StoredSettings::WeighingSystemTareOffset(
            SettingValue::Float(offset),
        ))
        .await
    }

    pub async fn get_weighing_system_tare_offset(&mut self) -> Option<f32> {
        let result = self
            .get_setting(
                StoredSettings::WeighingSystemTareOffset(SettingValue::Float(0.0)).discriminant(),
            )
            .await;

        if let Some(setting_value) = result {
            return match setting_value {
                SettingValue::Float(v) => Some(v),
                _ => None,
            };
        }
        None
    }

    pub async fn set_weighing_system_calibration_gradient(
        &mut self,
        gradient: f32,
    ) -> Result<(), SettingError> {
        self.save_setting(StoredSettings::WeighingSystemCalibrationGradient(
            SettingValue::Float(gradient),
        ))
        .await
    }

    pub async fn get_weighing_system_calibration_gradient(&mut self) -> Option<f32> {
        let result = self
            .get_setting(
                StoredSettings::WeighingSystemCalibrationGradient(SettingValue::Float(0.0))
                    .discriminant(),
            )
            .await;

        if let Some(setting_value) = result {
            return match setting_value {
                SettingValue::Float(v) => Some(v),
                _ => None,
            };
        }
        None
    }

    pub async fn set_system_led_brightness(&mut self, brightness: i8) -> Result<(), SettingError> {
        self.save_setting(StoredSettings::SystemLedBrightness(SettingValue::SmallInt(
            brightness,
        )))
        .await
    }

    pub async fn get_system_led_brightness(&mut self) -> Option<i8> {
        let result = self
            .get_setting(
                StoredSettings::SystemLedBrightness(SettingValue::SmallInt(0i8)).discriminant(),
            )
            .await;

        if let Some(setting_value) = result {
            return match setting_value {
                SettingValue::SmallInt(v) => Some(v),
                _ => None,
            };
        }
        None
    }
}
