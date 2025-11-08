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
use minicbor::{CborLen, Decode, Encode};

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

#[derive(Debug, PartialEq, Decode, Encode, CborLen)]
pub enum BootloaderMessages {
    #[n(0)] ReadyToDownload(#[n(0)] ReadyToDownload),
    #[n(1)] ReadyToDownloadResponse(#[n(0)] ReadyToDownloadResponse),
    #[n(2)] ChunkReq(#[n(0)] ChunkReq),
    #[n(3)] ChunkResp(#[n(0)] ChunkResp),
    #[n(4)] Goodbye(#[n(0)] Goodbye),
}

#[derive(Debug, PartialEq, Decode, Encode, CborLen)]
pub enum ApplicationMessages {
    #[n(0)] Goodbye(#[n(0)] Goodbye),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The buffer is too small to hold the encoded message and length prefix, value is the expected length
    BufferTooSmall(usize),
    /// The CBOR encoding failed due to invalid data
    EncodingError,
    DecodingError,
}


/// Frame a CBOR message with a length prefix for sending across a data link.
///
/// Returns the number of bytes written on success, or a FrameError on failure.
///
/// # Errors
///
/// - `BufferTooSmall`: if the buffer is not large enough to hold the length prefix (2 bytes)
///   plus the encoded message
/// - `EncodingError`: if the CBOR encoding of the message failed
pub fn frame_message<M>(msg: &M, buffer: &mut [u8]) -> Result<usize, FrameError>
where
    M: minicbor::Encode<()> + minicbor::CborLen<()>,
{
    const PREFIX_BYTE_COUNT: usize = 2;

    let encoded_len = minicbor::len(msg);
    let total_bytes_needed = encoded_len + PREFIX_BYTE_COUNT;

    // Check if the buffer is large enough
    if buffer.len() < total_bytes_needed {
        return Err(FrameError::BufferTooSmall(total_bytes_needed));
    }

    // Write the length prefix
    buffer[0..PREFIX_BYTE_COUNT].copy_from_slice(&(encoded_len as u16).to_be_bytes());

    // Encode the message, capturing any encoding errors
    minicbor::encode(msg, &mut buffer[PREFIX_BYTE_COUNT..])
        .map_err(|_| FrameError::EncodingError)?;

    Ok(total_bytes_needed)
}

pub fn decode_framed_message<'b, M>(
    buffer: &'b [u8],
) -> Result<(usize, M), FrameError>
where
    M: minicbor::Decode<'b, ()>,
{
    const PREFIX_BYTE_COUNT: usize = 2;

    if buffer.len() < PREFIX_BYTE_COUNT {
        return Err(FrameError::BufferTooSmall(PREFIX_BYTE_COUNT));
    }

    // Read the length prefix
    let mut length_bytes = [0u8; PREFIX_BYTE_COUNT];
    length_bytes.copy_from_slice(&buffer[..PREFIX_BYTE_COUNT]);
    let message_len = u16::from_be_bytes(length_bytes) as usize;

    if buffer.len() < message_len + PREFIX_BYTE_COUNT {
        return Err(FrameError::BufferTooSmall(message_len));
    }

    let message_end = PREFIX_BYTE_COUNT + message_len;
    let message = minicbor::decode::<M>(&buffer[PREFIX_BYTE_COUNT..message_end])
        .map_err(|_| FrameError::DecodingError)?;

    let consume_bytes = message_len + PREFIX_BYTE_COUNT;
    Ok((consume_bytes, message))
}