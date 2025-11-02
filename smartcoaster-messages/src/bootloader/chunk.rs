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
use crate::bootloader::CHUNK_SIZE;

#[derive(Debug, PartialEq, Encode, Decode, CborLen)]
pub struct ChunkReq {
    #[n(0)] pub chunk_number: u32,
}

#[derive(Debug, PartialEq, Encode, Decode, CborLen)]
pub struct ChunkResp {
    #[n(0)] pub chunk_number: u32,
    #[n(1)] pub chunk_data: [u8; CHUNK_SIZE],
    #[n(2)] pub crc32: [u8; 4],
}

impl ChunkResp {
    pub fn is_crc_ok(&self) -> bool {
        crate::bootloader::builder::calculate_crc32(&self.chunk_data) == self.crc32
    }
}