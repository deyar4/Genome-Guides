/*
   Copyright (C) 2026 Kamila Szewczyk <k@iczelia.net> on the behalf of the Saarland University.

   This program is free software; you can redistribute it and/or modify
   it under the terms of the GNU General Public License as published by
   the Free Software Foundation; either version 3 of the License, or
   (at your option) any later version.

   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
   GNU General Public License for more details.

   You should have received a copy of the GNU General Public License
   along with this program. If not, see <http://www.gnu.org/licenses/>.
*/

/// LEB128 unsigned varint encoding/decoding.

/// Encode a u32 value as unsigned LEB128 into `buf`. Returns bytes written.
pub fn encode_varint_u32(buf: &mut Vec<u8>, mut value: u32) -> usize {
    let start = buf.len();
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
    buf.len() - start
}

/// Decode unsigned LEB128 from `data`. Returns (value, bytes_consumed).
pub fn decode_varint_u32(data: &[u8]) -> std::io::Result<(u32, usize)> {
    let mut value: u32 = 0;
    let mut shift: u32 = 0;
    for (i, &byte) in data.iter().enumerate() {
        if shift >= 35 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "varint overflow (more than 5 bytes for u32)",
            ));
        }
        value |= ((byte & 0x7F) as u32) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Ok((value, i + 1));
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "truncated varint",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(val: u32) {
        let mut buf = Vec::new();
        let written = encode_varint_u32(&mut buf, val);
        assert_eq!(buf.len(), written);
        let (decoded, consumed) = decode_varint_u32(&buf).unwrap();
        assert_eq!(decoded, val);
        assert_eq!(consumed, written);
    }

    #[test]
    fn test_varint_zero() {
        round_trip(0);
        let mut buf = Vec::new();
        encode_varint_u32(&mut buf, 0);
        assert_eq!(buf, vec![0x00]);
    }

    #[test]
    fn test_varint_single_byte() {
        round_trip(1);
        round_trip(127);
        let mut buf = Vec::new();
        encode_varint_u32(&mut buf, 127);
        assert_eq!(buf, vec![0x7F]);
    }

    #[test]
    fn test_varint_two_bytes() {
        round_trip(128);
        let mut buf = Vec::new();
        encode_varint_u32(&mut buf, 128);
        assert_eq!(buf, vec![0x80, 0x01]);

        round_trip(16383);
        let mut buf = Vec::new();
        encode_varint_u32(&mut buf, 16383);
        assert_eq!(buf, vec![0xFF, 0x7F]);
    }

    #[test]
    fn test_varint_max() {
        round_trip(u32::MAX);
        let mut buf = Vec::new();
        encode_varint_u32(&mut buf, u32::MAX);
        assert_eq!(buf.len(), 5);
    }

    #[test]
    fn test_varint_typical_illumina_values() {
        // tile: 1101, x: 2692, y: 1000
        round_trip(1101);
        round_trip(2692);
        round_trip(1000);
        round_trip(50000);
    }

    #[test]
    fn test_varint_truncated() {
        assert!(decode_varint_u32(&[0x80]).is_err());
        assert!(decode_varint_u32(&[]).is_err());
    }

    #[test]
    fn test_varint_extra_data() {
        // Extra data after varint should be ignored
        let mut buf = Vec::new();
        encode_varint_u32(&mut buf, 42);
        buf.push(0xFF); // extra
        let (val, consumed) = decode_varint_u32(&buf).unwrap();
        assert_eq!(val, 42);
        assert_eq!(consumed, 1);
    }
}
