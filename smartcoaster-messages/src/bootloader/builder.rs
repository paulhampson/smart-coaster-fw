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

use crate::BootloaderMessages;
use crate::bootloader::{chunk::{ChunkReq, ChunkResp}, CHUNK_SIZE};
use crate::bootloader::ready_to_download::{ReadyToDownload, ReadyToDownloadResponse};
use crate::custom_data_types::{AsconHash256Bytes, VersionNumber};
use crate::general::goodbye::{Goodbye, GoodbyeReason};
use crc::{Crc, CRC_32_ISO_HDLC};

/// A builder for creating `BootloaderMessages`.
pub struct BootloaderMessagesBuilder;

impl BootloaderMessagesBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn ready_to_download(self) -> ReadyToDownloadBuilder {
        ReadyToDownloadBuilder::new()
    }

    pub fn ready_to_download_response(self) -> ReadyToDownloadResponseBuilder {
        ReadyToDownloadResponseBuilder::new()
    }

    pub fn chunk_req(self) -> ChunkReqBuilder {
        ChunkReqBuilder::new()
    }

    pub fn chunk_resp(self) -> ChunkRespBuilder {
        ChunkRespBuilder::new()
    }

    pub fn goodbye(self) -> GoodbyeBuilder {
        GoodbyeBuilder::new()
    }
}

impl Default for BootloaderMessagesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ReadyToDownloadBuilder {
    image_size_bytes: Option<u32>,
    version: Option<VersionNumber>,
    hash: Option<AsconHash256Bytes>,
}

impl ReadyToDownloadBuilder {
    fn new() -> Self {
        Self {
            image_size_bytes: None,
            version: None,
            hash: None,
        }
    }

    pub fn image_size_bytes(mut self, size: u32) -> Self {
        self.image_size_bytes = Some(size);
        self
    }

    pub fn version(mut self, version: VersionNumber) -> Self {
        self.version = Some(version);
        self
    }

    pub fn hash(mut self, hash: AsconHash256Bytes) -> Self {
        self.hash = Some(hash);
        self
    }

    /// Builds the `BootloaderMessages::ReadyToDownload` message.
    ///
    /// # Panics
    ///
    /// Panics if `image_size_bytes`, `version`, or `hash` have not been set.
    pub fn build(self) -> BootloaderMessages {
        BootloaderMessages::ReadyToDownload(ReadyToDownload {
            image_size_bytes: self.image_size_bytes.expect("image_size_bytes must be set"),
            version: self.version.expect("version must be set"),
            hash: self.hash.expect("hash must be set"),
        })
    }
}

pub struct ReadyToDownloadResponseBuilder {
    desired_chunk_size: Option<u32>,
}

impl ReadyToDownloadResponseBuilder {
    fn new() -> Self {
        Self {
            desired_chunk_size: Some(CHUNK_SIZE as u32),
        }
    }

    /// Builds the `BootloaderMessages::ReadyToDownloadResponse` message.

    pub fn build(self) -> BootloaderMessages {
        BootloaderMessages::ReadyToDownloadResponse(ReadyToDownloadResponse {
            desired_chunk_size: self.desired_chunk_size.expect("desired_chunk_size must be set"),
        })
    }
}

pub struct ChunkReqBuilder {
    chunk_number: Option<u32>,
}

impl ChunkReqBuilder {
    fn new() -> Self {
        Self {
            chunk_number: None,
        }
    }

    pub fn chunk_number(mut self, number: u32) -> Self {
        self.chunk_number = Some(number);
        self
    }

    /// Builds the `BootloaderMessages::ChunkReq` message.
    ///
    /// # Panics
    ///
    /// Panics if `chunk_number` has not been set.
    pub fn build(self) -> BootloaderMessages {
        BootloaderMessages::ChunkReq(ChunkReq {
            chunk_number: self.chunk_number.expect("chunk_number must be set"),
        })
    }
}

pub struct ChunkRespBuilder {
    chunk_number: Option<u32>,
    chunk_data: Option<[u8; CHUNK_SIZE]>,
}

impl ChunkRespBuilder {
    fn new() -> Self {
        Self {
            chunk_number: None,
            chunk_data: None,
        }
    }

    /// Sets the chunk number.
    pub fn chunk_number(mut self, number: u32) -> Self {
        self.chunk_number = Some(number);
        self
    }

    /// Sets the chunk data.
    pub fn chunk_data(mut self, data: [u8; CHUNK_SIZE]) -> Self {
        self.chunk_data = Some(data);
        self
    }

    /// Builds the `BootloaderMessages::ChunkResp` message.
    ///
    /// The CRC32 field is automatically calculated from the chunk data.
    ///
    /// # Panics
    ///
    /// Panics if `chunk_number` or `chunk_data` have not been set.
    pub fn build(self) -> BootloaderMessages {
        let chunk_data = self.chunk_data.expect("chunk_data must be set");
        let crc32 = calculate_crc32(&chunk_data);

        BootloaderMessages::ChunkResp(ChunkResp {
            chunk_number: self.chunk_number.expect("chunk_number must be set"),
            chunk_data,
            crc32,
        })
    }
}

/// A builder for creating a `BootloaderMessages::Goodbye` message.
pub struct GoodbyeBuilder {
    reason: Option<GoodbyeReason>,
}

impl GoodbyeBuilder {
    fn new() -> Self {
        Self { reason: None }
    }

    pub fn reason(mut self, reason: GoodbyeReason) -> Self {
        self.reason = Some(reason);
        self
    }

    /// Sets the reason to `InstallingNewFirmware`.
    pub fn installing_new_firmware(self) -> Self {
        self.reason(GoodbyeReason::InstallingNewFirmware)
    }

    /// Builds the `BootloaderMessages::Goodbye` message.
    ///
    /// # Panics
    ///
    /// Panics if `reason` has not been set.
    pub fn build(self) -> BootloaderMessages {
        BootloaderMessages::Goodbye(Goodbye {
            reason: self.reason.expect("reason must be set"),
        })
    }
}

/// Calculates the CRC32 checksum of the given data using CRC_32_ISO_HDLC.
///
/// Returns the CRC32 as a 4-byte array in little-endian format.
fn calculate_crc32(data: &[u8]) -> [u8; 4] {
    const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    let checksum = CRC.checksum(data);
    checksum.to_le_bytes()
}