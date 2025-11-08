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

use defmt::trace;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, Instance};
use embassy_usb::class::cdc_acm::BufferedReceiver;
use embedded_io_async::{Read, Write};
use minicbor::Decode;

#[derive(Debug)]
pub enum ReceiveError {
    ReadError,
    DecodeError,
    MessageTooLarge,
}

#[derive(Debug)]
pub enum SendError {
    EncodeError,
    UsbError,
    MessageTooLarge,
}

/// Read a CBOR message from the buffered receiver with length-prefixed framing
///
/// Message format: [length_high_byte][length_low_byte][cbor_data...]
/// where length is the size of the CBOR data in bytes (big-endian u16)
pub async fn read_cbor_message<'d, 'b, M>(
    rx: &mut BufferedReceiver<'d, Driver<'d, USB>>,
    buffer: &'b mut [u8],
) -> Result<M, ReceiveError>
where
    M: Decode<'b, ()>,
{
    trace!("Reading framing length prefix");
    // Read the 2-byte length prefix
    let mut length_bytes = [0u8; 2];
    rx.read_exact(&mut length_bytes)
        .await
        .map_err(|_| ReceiveError::ReadError)?;

    let message_len = u16::from_be_bytes(length_bytes) as usize;

    // Limit message size to prevent buffer overflow (adjust as needed)
    const MAX_MESSAGE_SIZE: usize = 4096;
    if message_len > MAX_MESSAGE_SIZE {
        return Err(ReceiveError::MessageTooLarge);
    }

    // Read the CBOR message data
    if message_len > buffer.len() {
        return Err(ReceiveError::MessageTooLarge);
    }

    trace!("Reading {} bytes for the message", message_len);
    rx.read_exact(&mut buffer[..message_len])
        .await
        .map_err(|_| ReceiveError::ReadError)?;

    trace!("Decoding message");
    // Decode the CBOR message
    minicbor::decode::<M>(&buffer[..message_len]).map_err(|_| ReceiveError::DecodeError)
}

/// Send a CBOR message through the CDC-ACM sender with length-prefixed framing
///
/// Message format: [length_high_byte][length_low_byte][cbor_data...]
/// where length is the size of the CBOR data in bytes (big-endian u16)
pub async fn send_cbor_message<'d, T: Instance, M>(
    tx: &mut embassy_usb::class::cdc_acm::Sender<'d, Driver<'d, T>>,
    msg: &M,
) -> Result<(), SendError>
where
    M: minicbor::Encode<()> + minicbor::CborLen<()>,
{
    // Encode to a buffer first
    let mut buffer = [0u8; 4096];
    let encoded_len = minicbor::len(msg);
    minicbor::encode(msg, &mut buffer[..]).map_err(|_| SendError::EncodeError)?;

    if encoded_len > u16::MAX as usize {
        return Err(SendError::MessageTooLarge);
    }

    // Write length prefix (big-endian u16)
    let length_bytes = (encoded_len as u16).to_be_bytes();
    tx.write_all(&length_bytes)
        .await
        .map_err(|_| SendError::UsbError)?;

    // Write the encoded message
    tx.write_all(&buffer[..encoded_len])
        .await
        .map_err(|_| SendError::UsbError)
}
