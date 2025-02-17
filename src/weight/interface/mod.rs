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

pub mod hx711async;

#[allow(dead_code)]
pub trait AsyncStrainGaugeInterface {
    type Error;

    /// Initialise the gauge and make it ready for taking readings. Will put it into an initalized,
    /// powered up state.
    async fn initialize(&mut self) -> Result<(), Self::Error>;

    /// Gets next reading from the strain gauge. If the gauge is powered down then
    /// this function is expected to power up the device and get the reading.
    async fn get_next_reading(&mut self) -> Result<i32, Self::Error>;

    /// Power down the strain gauge
    async fn power_down(&mut self) -> Result<(), Self::Error>;

    /// Power up the strain gauge
    async fn power_up(&mut self) -> Result<(), Self::Error>;

    /// Return the number of bits supported by the ADC
    fn get_adc_bit_count(&self) -> usize;
}
