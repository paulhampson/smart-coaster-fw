use crate::weight::interface::AsyncStrainGaugeInterface;
use core::cmp::max;
use defmt::trace;
use heapless::Vec;
use micromath::statistics::{Mean, StdDev};
use micromath::F32Ext;

const BITS_TO_DISCARD_BEFORE_STABILISATION: usize = 0;
const STABILISATION_MEASUREMENTS: usize = 10;

#[derive(Debug)]
pub enum Error<StrainGaugeE> {
    StrainGaugeReadingError(StrainGaugeE)
}

pub struct WeightScale<StrainGauge> {
    strain_gauge: StrainGauge,
    tare_offset: f32,
    bits_to_discard: usize,
    calibration_gradient: f32,
}

impl<StrainGauge, StrainGaugeE> WeightScale<StrainGauge>
where
    StrainGauge: AsyncStrainGaugeInterface<Error = StrainGaugeE>,
{

    pub async fn new(mut strain_gauge: StrainGauge) -> Result<Self, Error<StrainGaugeE>>  {
        strain_gauge.initialize().await.map_err(Error::StrainGaugeReadingError)?;
        Ok(Self {
            strain_gauge,
            tare_offset: 0.0,
            bits_to_discard: BITS_TO_DISCARD_BEFORE_STABILISATION,
            calibration_gradient: 0.0,
        })
    }

    async fn get_filtered_raw_reading(&mut self) -> Result<f32, Error<StrainGaugeE>>
    {
        let reading = self
            .strain_gauge
            .get_next_reading()
            .await
            .map_err(Error::StrainGaugeReadingError)?;

        Ok((reading >> self.bits_to_discard) as f32)
    }

    pub async fn tare(&mut self) -> Result<(), Error<StrainGaugeE>> {
        self.stabilize_measurements().await?;

        let mut measurement_buffer = Vec::<f32, STABILISATION_MEASUREMENTS>::new();

        for _ in 0..STABILISATION_MEASUREMENTS {
            let reading = self.get_filtered_raw_reading().await?;
            measurement_buffer.push(reading).expect("Too many readings taken by stabilization function");
        }

        self.tare_offset = measurement_buffer.into_iter().mean();
        trace!("Tare offset = {} ", self.tare_offset);

        Ok(())
    }

    pub async fn calibrate(&mut self, calibration_mass: f32) -> Result<(), Error<StrainGaugeE>> {
        self.stabilize_measurements().await?;

        let mut measurement_buffer = Vec::<f32, STABILISATION_MEASUREMENTS>::new();

        for _ in 0..STABILISATION_MEASUREMENTS {
            let reading = self.get_filtered_raw_reading().await?;
            measurement_buffer.push(reading).expect("Too many readings taken by calibration function");
        }

        let tared_mean_measurement = measurement_buffer.into_iter().mean() - self.tare_offset;
        let grams_per_count = calibration_mass / tared_mean_measurement;
        self.calibration_gradient = grams_per_count;
        trace!("Calibration mass per count = {}", grams_per_count);
        Ok(())
    }

    pub async fn get_instantaneous_weight_grams(&mut self) -> Result<f32, Error<StrainGaugeE>> {
        let reading = self.get_filtered_raw_reading().await?;
        trace!("Reading = {}", reading);
        let tared_reading = reading - self.tare_offset as f32;
        trace!("Tared reading = {}", tared_reading);
        Ok(tared_reading * self.calibration_gradient)
    }

    /// Takes readings and analyses noise, sets internal parameters to eliminate noise.
    pub async fn stabilize_measurements(&mut self) -> Result<(), Error<StrainGaugeE>> {
        let mut measurement_buffer = Vec::<f32, STABILISATION_MEASUREMENTS>::new();

        for _ in 0..STABILISATION_MEASUREMENTS {
            let reading = self
                .strain_gauge
                .get_next_reading()
                .await
                .map_err(Error::StrainGaugeReadingError)? as f32;
            measurement_buffer.push(reading).expect("Too many readings taken by stabilization function");
        }

        let standard_deviation = measurement_buffer.as_slice().stddev();

        // Calculate the number of noise bits
        let full_scale_range: f32 = (1 << self.strain_gauge.get_adc_bit_count()) as f32;
        let new_bits_to_discard = self.strain_gauge.get_adc_bit_count() - (full_scale_range / standard_deviation ).log2().ceil() as usize;
        self.bits_to_discard = max(new_bits_to_discard, self.bits_to_discard);

        trace!("Stabilize measurements calculated {} bits to discard", self.bits_to_discard);

        Ok(())
    }

}