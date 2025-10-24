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

pub(crate) mod weight;
pub(crate) mod interface;
pub(crate) mod messaging;


pub trait WeighingSystem {

    type Error;

    async fn stabilize_measurements(&mut self) -> Result<(), Self::Error>;

    async fn tare(&mut self) -> Result<(), Self::Error>;

    async fn calibrate(&mut self, calibration_mass:f32) -> Result<(), Self::Error>;

    async fn get_instantaneous_weight_grams(&mut self) -> Result<f32, Self::Error>;

    async fn get_reading(&mut self) -> Result<f32, Self::Error>;
}