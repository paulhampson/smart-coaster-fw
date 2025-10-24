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

use crate::storage::settings::{SettingValue, SettingsAccessor, SettingsAccessorId};
use crate::weight::interface::AsyncStrainGaugeInterface;
use crate::weight::WeighingSystem;
use core::cmp::max;
use defmt::{debug, trace, warn, Debug2Format};
use heapless::Vec;
use micromath::statistics::{Mean, StdDev};
use micromath::F32Ext;

const STABILISATION_MEASUREMENTS: usize = 20;

#[derive(Debug)]
pub enum Error<StrainGaugeE> {
    StrainGaugeReadingError(StrainGaugeE),
}

pub struct WeightScale<StrainGauge, SA>
where
    SA: SettingsAccessor,
{
    strain_gauge: StrainGauge,
    tare_offset: f32,
    bits_to_discard: usize,
    calibration_gradient: f32,
    is_stabilized: bool,
    settings: SA,
}

impl<StrainGauge, StrainGaugeE, SA> WeightScale<StrainGauge, SA>
where
    StrainGauge: AsyncStrainGaugeInterface<Error = StrainGaugeE>,
    SA: SettingsAccessor,
{
    pub async fn new(
        mut strain_gauge: StrainGauge,
        settings: SA,
    ) -> Result<Self, Error<StrainGaugeE>> {
        strain_gauge
            .initialize()
            .await
            .map_err(Error::StrainGaugeReadingError)?;

        let mut s = Self {
            strain_gauge,
            tare_offset: 0.0,
            bits_to_discard: 0,
            calibration_gradient: 0.0,
            is_stabilized: false,
            settings,
        };

        (s.tare_offset, s.calibration_gradient, s.bits_to_discard) =
            s.get_stored_calibration().await;

        debug!(
            "Loaded calibration: tare = {}, gradient = {}, bits to discard = {}",
            s.tare_offset, s.calibration_gradient, s.bits_to_discard
        );

        if s.bits_to_discard > 0 {
            s.is_stabilized = true;
        }

        Ok(s)
    }

    async fn get_stored_calibration(&self) -> (f32, f32, usize) {
        let tare_offset: f32 = if let Some(result) = self
            .settings
            .get_setting(SettingsAccessorId::WeighingSystemTareOffset)
            .await
        {
            match result {
                SettingValue::Float(v) => v,
                _ => {
                    warn!("Unable to get stored gradient");
                    0.0
                }
            }
        } else {
            warn!("Unable to get stored gradient");
            0.0
        };

        let calibration_gradient: f32 = if let Some(result) = self
            .settings
            .get_setting(SettingsAccessorId::WeighingSystemCalibrationGradient)
            .await
        {
            match result {
                SettingValue::Float(v) => v,
                _ => {
                    warn!("Unable to get stored tare");
                    0.0
                }
            }
        } else {
            warn!("Unable to get stored tare");
            0.0
        };

        let bits_to_discard: usize = if let Some(result) = self
            .settings
            .get_setting(SettingsAccessorId::WeighingSystemBitsToDiscard)
            .await
        {
            match result {
                SettingValue::SmallUInt(v) => v as usize,
                _ => {
                    warn!("Unable to get bits to discard settings_menu");
                    0usize
                }
            }
        } else {
            warn!("Unable to get bits to discard settings_menu");
            0usize
        };

        (tare_offset, calibration_gradient, bits_to_discard)
    }

    async fn get_filtered_raw_reading(&mut self) -> Result<f32, Error<StrainGaugeE>> {
        let reading = self
            .strain_gauge
            .get_next_reading()
            .await
            .map_err(Error::StrainGaugeReadingError)?;

        Ok((reading >> self.bits_to_discard) as f32)
    }

    async fn save_new_tare(&mut self, tare: f32) {
        self.tare_offset = tare;
        let _ = self
            .settings
            .save_setting(
                SettingsAccessorId::WeighingSystemTareOffset,
                SettingValue::Float(tare),
            )
            .await
            .map_err(|e| warn!("Unable store tare offset: {}", Debug2Format(&e)));
    }

    async fn save_new_calibration_gradient(&mut self, gradient: f32) {
        self.calibration_gradient = gradient;
        let _ = self
            .settings
            .save_setting(
                SettingsAccessorId::WeighingSystemCalibrationGradient,
                SettingValue::Float(gradient),
            )
            .await
            .map_err(|e| warn!("Unable store calibration gradient: {}", Debug2Format(&e)));
    }

    async fn save_new_bits_to_discard(&mut self, bits_to_discard: usize) {
        self.bits_to_discard = bits_to_discard;
        let _ = self
            .settings
            .save_setting(
                SettingsAccessorId::WeighingSystemBitsToDiscard,
                SettingValue::SmallUInt(bits_to_discard as u8),
            )
            .await
            .map_err(|e| warn!("Unable store bits to discard: {}", Debug2Format(&e)));
    }

    pub fn is_stabilized(&self) -> bool {
        self.is_stabilized
    }
}
impl<StrainGauge, StrainGaugeE, SA> WeighingSystem for WeightScale<StrainGauge, SA>
where
    StrainGauge: AsyncStrainGaugeInterface<Error = StrainGaugeE>,
    SA: SettingsAccessor,
{
    type Error = Error<StrainGaugeE>;

    /// Takes readings and analyses noise, sets internal parameters to eliminate noise.
    async fn stabilize_measurements(&mut self) -> Result<(), Error<StrainGaugeE>> {
        let mut measurement_buffer = Vec::<f32, STABILISATION_MEASUREMENTS>::new();

        for _ in 0..STABILISATION_MEASUREMENTS {
            let reading = self
                .strain_gauge
                .get_next_reading()
                .await
                .map_err(Error::StrainGaugeReadingError)? as f32;
            measurement_buffer
                .push(reading)
                .expect("Too many readings taken by stabilization function");
        }

        let standard_deviation = measurement_buffer.as_slice().stddev();

        // Calculate the number of noise bits
        let full_scale_range: f32 = (1 << self.strain_gauge.get_adc_bit_count()) as f32;
        let new_bits_to_discard = self.strain_gauge.get_adc_bit_count()
            - (full_scale_range / standard_deviation).log2().ceil() as usize;
        self.save_new_bits_to_discard(max(new_bits_to_discard, self.bits_to_discard))
            .await;

        debug!(
            "Stabilize measurements calculated {} bits to discard",
            self.bits_to_discard
        );
        self.is_stabilized = true;
        Ok(())
    }

    async fn tare(&mut self) -> Result<(), Error<StrainGaugeE>> {
        let mut measurement_buffer = Vec::<f32, STABILISATION_MEASUREMENTS>::new();

        for _ in 0..STABILISATION_MEASUREMENTS {
            let reading = self.get_filtered_raw_reading().await?;
            measurement_buffer
                .push(reading)
                .expect("Too many readings taken by stabilization function");
        }
        self.save_new_tare(measurement_buffer.into_iter().mean())
            .await;
        trace!("Tare offset = {} ", self.tare_offset);

        Ok(())
    }

    async fn calibrate(&mut self, calibration_mass: f32) -> Result<(), Error<StrainGaugeE>> {
        let mut measurement_buffer = Vec::<f32, STABILISATION_MEASUREMENTS>::new();

        for _ in 0..STABILISATION_MEASUREMENTS {
            let reading = self.get_filtered_raw_reading().await?;
            measurement_buffer
                .push(reading)
                .expect("Too many readings taken by calibration function");
        }

        let tared_mean_measurement = measurement_buffer.into_iter().mean() - self.tare_offset;
        let grams_per_count = calibration_mass / tared_mean_measurement;
        self.save_new_calibration_gradient(grams_per_count).await;
        trace!("Calibration mass per count = {}", grams_per_count);
        Ok(())
    }

    async fn get_instantaneous_weight_grams(&mut self) -> Result<f32, Error<StrainGaugeE>> {
        if !self.is_stabilized() {
            self.stabilize_measurements().await?;
        }
        let reading = self.get_filtered_raw_reading().await?;
        trace!("Reading = {}", reading);
        let tared_reading = reading - self.tare_offset;
        trace!("Tared reading = {}", tared_reading);
        Ok(tared_reading * self.calibration_gradient)
    }

    async fn get_reading(&mut self) -> Result<f32, Self::Error> {
        self.get_instantaneous_weight_grams().await
    }
}
