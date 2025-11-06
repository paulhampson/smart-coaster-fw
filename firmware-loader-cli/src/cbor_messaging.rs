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

use std::io::{Error as IoError, ErrorKind, Read, Result as IoResult, Write};

/// Sends a CBOR message with length-prefixed framing over serial
pub(crate) fn send_message<M>(serial: &mut Box<dyn serialport::SerialPort>, msg: &M) -> IoResult<()>
where
    M: minicbor::Encode<()> + minicbor::CborLen<()>,
{
    let mut buffer = [0u8; 4096];
    let encoded_len = minicbor::len(msg);
    minicbor::encode(msg, &mut buffer[..])
        .map_err(|_| IoError::new(ErrorKind::InvalidData, "Encode error"))?;

    // Write length prefix (big-endian u16)
    let length_bytes = (encoded_len as u16).to_be_bytes();
    serial.write_all(&length_bytes)?;
    serial.write_all(&buffer[..encoded_len])?;

    log::trace!("Sent message of {} bytes", encoded_len);
    Ok(())
}

pub(crate) fn receive_message<'b, M>(
    serial: &mut Box<dyn serialport::SerialPort>,
    buffer: &'b mut [u8],
) -> IoResult<M>
where
    M: minicbor::Decode<'b, ()>,
{
    // Read the 2-byte length prefix
    let mut length_bytes = [0u8; 2];
    serial.read_exact(&mut length_bytes)?;
    let message_len = u16::from_be_bytes(length_bytes) as usize;

    if message_len > buffer.len() {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            "Message too large",
        ));
    }

    serial.read_exact(&mut buffer[..message_len])?;

    minicbor::decode::<M>(&buffer[..message_len])
        .map_err(|_| IoError::new(ErrorKind::InvalidData, "Decode error"))
}