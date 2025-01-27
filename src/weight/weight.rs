use crate::application::storage::settings::{
    wait_for_settings_store_initialisation, SETTINGS_STORE,
};
use crate::weight::interface::AsyncStrainGaugeInterface;
use crate::weight::WeighingSystem;
use core::cmp::max;
use defmt::{trace, warn};
use heapless::Vec;
use micromath::statistics::{Mean, StdDev};
use micromath::F32Ext;

const BITS_TO_DISCARD_BEFORE_STABILISATION: usize = 0;
const STABILISATION_MEASUREMENTS: usize = 20;

#[derive(Debug)]
pub enum Error<StrainGaugeE> {
    StrainGaugeReadingError(StrainGaugeE),
}

pub struct WeightScale<StrainGauge> {
    strain_gauge: StrainGauge,
    tare_offset: f32,
    bits_to_discard: usize,
    calibration_gradient: f32,
    is_stabilized: bool,
}

impl<StrainGauge, StrainGaugeE> WeightScale<StrainGauge>
where
    StrainGauge: AsyncStrainGaugeInterface<Error = StrainGaugeE>,
{
    pub async fn new(mut strain_gauge: StrainGauge) -> Result<Self, Error<StrainGaugeE>> {
        strain_gauge
            .initialize()
            .await
            .map_err(Error::StrainGaugeReadingError)?;

        let (tare_offset, calibration_gradient) = Self::get_stored_calibration().await;

        Ok(Self {
            strain_gauge,
            tare_offset,
            bits_to_discard: BITS_TO_DISCARD_BEFORE_STABILISATION,
            calibration_gradient,
            is_stabilized: false,
        })
    }

    async fn get_stored_calibration() -> (f32, f32) {
        let tare_offset;
        let calibration_gradient;
        wait_for_settings_store_initialisation().await;
        let settings = SETTINGS_STORE.lock().await;
        tare_offset = settings.get_weighing_system_tare_offset().unwrap_or(0.0);
        calibration_gradient = settings
            .get_weighing_system_calibration_gradient()
            .unwrap_or(0.0);
        trace!(
            "Loaded calibration values - tare: {}, gradient: {}",
            tare_offset,
            calibration_gradient
        );
        (tare_offset, calibration_gradient)
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
        let mut settings = SETTINGS_STORE.lock().await;
        let _ = settings
            .set_weighing_system_tare_offset(tare)
            .map_err(|_| warn!("Unable store tare offset"));
    }

    async fn save_new_calibration_gradient(&mut self, gradient: f32) {
        self.calibration_gradient = gradient;
        let mut settings = SETTINGS_STORE.lock().await;
        let _ = settings
            .set_weighing_system_calibration_gradient(gradient)
            .map_err(|_| warn!("Unable store calibration gradient"));
    }

    pub fn is_stabilized(&self) -> bool {
        self.is_stabilized
    }
}
impl<StrainGauge, StrainGaugeE> WeighingSystem for WeightScale<StrainGauge>
where
    StrainGauge: AsyncStrainGaugeInterface<Error = StrainGaugeE>,
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
        self.bits_to_discard = max(new_bits_to_discard, self.bits_to_discard);

        trace!(
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
