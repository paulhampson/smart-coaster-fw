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
use crate::custom_data_types::{AsconHash256Bytes, VersionNumber};

#[derive(Debug, PartialEq, Decode, Encode, CborLen)]
pub struct ReadyToDownload {
    #[n(0)] pub image_size_bytes: u32,
    #[n(1)] pub version: VersionNumber,
    #[n(2)] pub hash: AsconHash256Bytes,
}

#[derive(Debug, PartialEq, Decode, Encode, CborLen)]
pub struct ReadyToDownloadResponse {
    #[n(0)] pub desired_chunk_size: u32,
}
