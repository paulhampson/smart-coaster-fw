use crate::hmi::screens::settings_menu::monitoring_options::MonitoringTargetPeriodOptions;
use crate::storage::historical::{LogEncodeDecode, LogEncodeDecodeError};
use crate::storage::StoredDataValue;
use chrono::NaiveTime;
use defmt::{error, trace, warn, Debug2Format};
use sequential_storage::map::{SerializationError, Value};

pub struct DrinkMonitorLogData {
    hourly_consumption_target: StoredDataValue,
    daily_consumption_target: StoredDataValue,
    target_mode: StoredDataValue,
    total_consumption: StoredDataValue,
    daily_consumption_target_time: StoredDataValue,
    last_consumption: StoredDataValue,
}

impl DrinkMonitorLogData {
    pub fn new(
        hourly_consumption_target: f32,
        daily_consumption_target: u32,
        target_mode: MonitoringTargetPeriodOptions,
        total_consumption: f32,
        daily_consumption_target_time: NaiveTime,
        last_consumption: f32,
    ) -> Self {
        Self {
            hourly_consumption_target: StoredDataValue::Float(hourly_consumption_target),
            daily_consumption_target: StoredDataValue::UInt(daily_consumption_target),
            target_mode: StoredDataValue::SmallUInt(target_mode.into()),
            total_consumption: StoredDataValue::Float(total_consumption),
            daily_consumption_target_time: StoredDataValue::Time(daily_consumption_target_time),
            last_consumption: StoredDataValue::Float(last_consumption),
        }
    }

    pub fn get_last_consumption(&self) -> f32 {
        if let StoredDataValue::Float(last_consumption) = self.last_consumption {
            last_consumption
        } else {
            warn!(
                "Unexpected stored data for last_consumption: {}",
                Debug2Format(&self.last_consumption)
            );
            0.0
        }
    }
}

impl Default for DrinkMonitorLogData {
    fn default() -> Self {
        Self {
            hourly_consumption_target: StoredDataValue::Float(f32::default()),
            daily_consumption_target: StoredDataValue::UInt(u32::default()),
            target_mode: StoredDataValue::SmallUInt(u8::default()),
            total_consumption: StoredDataValue::Float(f32::default()),
            daily_consumption_target_time: StoredDataValue::Time(NaiveTime::default()),
            last_consumption: StoredDataValue::Float(f32::default()),
        }
    }
}

impl LogEncodeDecode for DrinkMonitorLogData {
    fn encode(&self, buf: &mut [u8]) -> Result<usize, LogEncodeDecodeError> {
        let mut data_size = 0;
        data_size += self
            .hourly_consumption_target
            .serialize_into(&mut buf[data_size..])
            .map_err(|e| {
                if e == SerializationError::BufferTooSmall {
                    LogEncodeDecodeError::BufferTooSmall
                } else {
                    LogEncodeDecodeError::EncodeFailed
                }
            })?;
        data_size += self
            .daily_consumption_target
            .serialize_into(&mut buf[data_size..])
            .map_err(|e| {
                if e == SerializationError::BufferTooSmall {
                    LogEncodeDecodeError::BufferTooSmall
                } else {
                    LogEncodeDecodeError::EncodeFailed
                }
            })?;
        data_size += self
            .target_mode
            .serialize_into(&mut buf[data_size..])
            .map_err(|e| {
                if e == SerializationError::BufferTooSmall {
                    LogEncodeDecodeError::BufferTooSmall
                } else {
                    LogEncodeDecodeError::EncodeFailed
                }
            })?;
        data_size += self
            .total_consumption
            .serialize_into(&mut buf[data_size..])
            .map_err(|e| {
                if e == SerializationError::BufferTooSmall {
                    LogEncodeDecodeError::BufferTooSmall
                } else {
                    LogEncodeDecodeError::EncodeFailed
                }
            })?;
        data_size += self
            .daily_consumption_target_time
            .serialize_into(&mut buf[data_size..])
            .map_err(|e| {
                if e == SerializationError::BufferTooSmall {
                    LogEncodeDecodeError::BufferTooSmall
                } else {
                    LogEncodeDecodeError::EncodeFailed
                }
            })?;
        data_size += self
            .last_consumption
            .serialize_into(&mut buf[data_size..])
            .map_err(|e| {
                if e == SerializationError::BufferTooSmall {
                    LogEncodeDecodeError::BufferTooSmall
                } else {
                    LogEncodeDecodeError::EncodeFailed
                }
            })?;

        Ok(data_size)
    }

    fn from_bytes(buf: &[u8]) -> Result<Self, LogEncodeDecodeError>
    where
        Self: Sized,
    {
        let expected_bytes = 30;
        if buf.len() < expected_bytes {
            error!(
                "Not enough bytes to decode - got {} expected {}",
                buf.len(),
                expected_bytes
            );
            return Err(LogEncodeDecodeError::BufferTooSmall);
        }
        trace!("buffer: {}", &buf);

        let mut s = Self::default();
        let mut data_start: usize = 0;
        let mut element_size = s.hourly_consumption_target.get_serialization_buffer_size();
        s.hourly_consumption_target =
            StoredDataValue::deserialize_from(&buf[data_start..data_start + element_size])
                .map_err(|e| {
                    error!(
                        "Unable to decode data {} - start: {} - element_size: {} - bytes: {}",
                        e,
                        data_start,
                        element_size,
                        &buf[data_start..data_start + element_size]
                    );
                    LogEncodeDecodeError::DecodeFailed
                })?;
        data_start += element_size;

        element_size = s.daily_consumption_target.get_serialization_buffer_size();
        s.daily_consumption_target =
            StoredDataValue::deserialize_from(&buf[data_start..data_start + element_size])
                .map_err(|e| {
                    error!(
                        "Unable to decode data {} - start: {} - element_size: {} - bytes: {}",
                        e,
                        data_start,
                        element_size,
                        &buf[data_start..data_start + element_size]
                    );
                    LogEncodeDecodeError::DecodeFailed
                })?;
        data_start += element_size;

        element_size = s.target_mode.get_serialization_buffer_size();
        s.target_mode =
            StoredDataValue::deserialize_from(&buf[data_start..data_start + element_size])
                .map_err(|e| {
                    error!(
                        "Unable to decode data {} - start: {} - element_size: {} - bytes: {}",
                        e,
                        data_start,
                        element_size,
                        &buf[data_start..data_start + element_size]
                    );
                    LogEncodeDecodeError::DecodeFailed
                })?;
        data_start += element_size;

        element_size = s.total_consumption.get_serialization_buffer_size();
        s.total_consumption =
            StoredDataValue::deserialize_from(&buf[data_start..data_start + element_size])
                .map_err(|e| {
                    error!(
                        "Unable to decode data {} - start: {} - element_size: {} - bytes: {}",
                        e,
                        data_start,
                        element_size,
                        &buf[data_start..data_start + element_size]
                    );
                    LogEncodeDecodeError::DecodeFailed
                })?;
        data_start += element_size;

        element_size = s
            .daily_consumption_target_time
            .get_serialization_buffer_size();
        s.daily_consumption_target_time =
            StoredDataValue::deserialize_from(&buf[data_start..data_start + element_size])
                .map_err(|e| {
                    error!(
                        "Unable to decode data {} - start: {} - element_size: {} - bytes: {}",
                        e,
                        data_start,
                        element_size,
                        &buf[data_start..data_start + element_size]
                    );
                    LogEncodeDecodeError::DecodeFailed
                })?;
        data_start += element_size;

        element_size = s.last_consumption.get_serialization_buffer_size();
        s.last_consumption =
            StoredDataValue::deserialize_from(&buf[data_start..data_start + element_size])
                .map_err(|e| {
                    error!(
                        "Unable to decode data {} - start: {} - element_size: {} - bytes: {}",
                        e,
                        data_start,
                        element_size,
                        &buf[data_start..data_start + element_size]
                    );
                    LogEncodeDecodeError::DecodeFailed
                })?;
        data_start += element_size;

        trace!("Decoded {} bytes", data_start);

        Ok(s)
    }
}
