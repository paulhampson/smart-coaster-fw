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

#[derive(Debug, PartialEq, Default, Encode, Decode, CborLen)]
pub struct VersionNumber {
    #[n(0)] major: u16,
    #[n(1)] minor: u16,
    #[n(2)] patch: u16,
}

impl VersionNumber {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct AsconHash256 {
    hash: [u8; 32],
}
