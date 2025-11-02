#![no_std]
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


use crate::bootloader::chunk::{ChunkReq, ChunkResp};
use crate::bootloader::ready_to_download::{ReadyToDownload, ReadyToDownloadResponse};
use crate::general::goodbye::Goodbye;
use crate::general::hello::{Hello, HelloResp};
use minicbor::{Encode, Decode, CborLen};

pub mod bootloader;
pub mod custom_data_types;
pub mod general;

#[derive(Debug, PartialEq, Decode, Encode, CborLen)]
pub enum GeneralMessages {
    #[n(0)] Hello(
        #[n(0)] Hello
    ),
    #[n(1)] HelloResp(
        #[n(0)] HelloResp
    ),
}

#[derive(Debug, PartialEq)]
pub enum BootloaderMessages {
    ReadyToDownload(ReadyToDownload),
    ReadyToDownloadResponse(ReadyToDownloadResponse),
    ChunkReq(ChunkReq),
    ChunkResp(ChunkResp),
    Goodbye(Goodbye),
}

#[derive(Debug, PartialEq)]
pub enum ApplicationMessages {
    Goodbye(Goodbye),
}
