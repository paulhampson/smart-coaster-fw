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

use minicbor::{CborLen, Decode, Encode};
use crate::custom_data_types::VersionNumber;

#[derive(Debug, PartialEq, Default, Encode, Decode, CborLen)]
pub struct Hello {}

#[derive(Debug, PartialEq, Copy, Clone, Default, Encode, Decode, CborLen)]
pub enum SystemMode {
    #[default]
    #[n(0)] Bootloader,
    #[n(1)] Application,
}

#[derive(Debug, PartialEq, Default, Encode, Decode, CborLen)]
pub struct HelloResp {
    #[n(0)] pub mode: SystemMode,
    #[n(1)] pub version: VersionNumber,
}
