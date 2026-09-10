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

/// Illumina FASTQ header tokenization.
///
/// Illumina headers have the format:
/// `instrument:run:flowcell:lane:tile:x:y space pair:filtered:control:index`
///
/// We split them into a text prefix (instrument:run:flowcell:lane:),
/// numeric coordinates (tile, x, y), and a text suffix (pair:filtered:control:index).

pub struct IlluminaHeader<'a> {
    pub prefix: &'a [u8], // "instrument:run:flowcell:lane:" (with trailing colon)
    pub tile: u32,
    pub x: u32,
    pub y: u32,
    pub suffix: &'a [u8], // "pair:filtered:control:index" (after the space)
}

/// Parse an Illumina-format FASTQ header (without the leading '@').
/// Returns `None` if the header doesn't match the expected format.
pub fn parse_illumina_header(header: &[u8]) -> Option<IlluminaHeader<'_>> {
    // Find the space separating coordinates from read info
    let space_pos = header.iter().position(|&b| b == b' ')?;
    let coord_part = &header[..space_pos];
    let suffix = &header[space_pos + 1..];

    if suffix.is_empty() {
        return None;
    }

    // Split coord_part by ':', expect exactly 7 fields:
    // instrument:run:flowcell:lane:tile:x:y
    let mut colon_positions = Vec::new();
    for (i, &b) in coord_part.iter().enumerate() {
        if b == b':' {
            colon_positions.push(i);
        }
    }

    if colon_positions.len() != 6 {
        return None; // Need exactly 6 colons for 7 fields
    }

    // prefix = everything up to and including the 4th colon (after lane)
    let prefix_end = colon_positions[3] + 1;
    let prefix = &coord_part[..prefix_end];

    // Parse tile (field 4), x (field 5), y (field 6)
    let tile_str = &coord_part[colon_positions[3] + 1..colon_positions[4]];
    let x_str = &coord_part[colon_positions[4] + 1..colon_positions[5]];
    let y_str = &coord_part[colon_positions[5] + 1..];

    let tile = parse_u32_bytes(tile_str)?;
    let x = parse_u32_bytes(x_str)?;
    let y = parse_u32_bytes(y_str)?;

    Some(IlluminaHeader {
        prefix,
        tile,
        x,
        y,
        suffix,
    })
}

/// Fast ASCII-to-u32 parser for small byte slices.
fn parse_u32_bytes(bytes: &[u8]) -> Option<u32> {
    if bytes.is_empty() {
        return None;
    }
    let mut val: u32 = 0;
    for &b in bytes {
        if !(b'0'..=b'9').contains(&b) {
            return None;
        }
        val = val.checked_mul(10)?.checked_add((b - b'0') as u32)?;
    }
    Some(val)
}

/// Reconstruct an Illumina header from its tokenized components.
/// Writes: `prefix + tile + ":" + x + ":" + y + " " + suffix`
pub fn reconstruct_illumina_header(
    prefix: &[u8],
    tile: u32,
    x: u32,
    y: u32,
    suffix: &[u8],
    out: &mut Vec<u8>,
) {
    out.extend_from_slice(prefix);
    write_u32_ascii(out, tile);
    out.push(b':');
    write_u32_ascii(out, x);
    out.push(b':');
    write_u32_ascii(out, y);
    out.push(b' ');
    out.extend_from_slice(suffix);
}

fn write_u32_ascii(buf: &mut Vec<u8>, val: u32) {
    if val == 0 {
        buf.push(b'0');
        return;
    }
    let start = buf.len();
    let mut v = val;
    while v > 0 {
        buf.push(b'0' + (v % 10) as u8);
        v /= 10;
    }
    buf[start..].reverse();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_illumina_header() {
        let header = b"A00488:61:HMLGNDSXX:4:1101:2692:1000 1:N:0:GATCAG";
        let parsed = parse_illumina_header(header).unwrap();
        assert_eq!(parsed.prefix, b"A00488:61:HMLGNDSXX:4:");
        assert_eq!(parsed.tile, 1101);
        assert_eq!(parsed.x, 2692);
        assert_eq!(parsed.y, 1000);
        assert_eq!(parsed.suffix, b"1:N:0:GATCAG");
    }

    #[test]
    fn test_parse_large_coords() {
        let header = b"INST:1:FC:2:2301:45678:12345 2:Y:0:ACGT+TGCA";
        let parsed = parse_illumina_header(header).unwrap();
        assert_eq!(parsed.prefix, b"INST:1:FC:2:");
        assert_eq!(parsed.tile, 2301);
        assert_eq!(parsed.x, 45678);
        assert_eq!(parsed.y, 12345);
        assert_eq!(parsed.suffix, b"2:Y:0:ACGT+TGCA");
    }

    #[test]
    fn test_parse_invalid_no_space() {
        let header = b"A00488:61:HMLGNDSXX:4:1101:2692:1000";
        assert!(parse_illumina_header(header).is_none());
    }

    #[test]
    fn test_parse_invalid_wrong_field_count() {
        let header = b"A00488:61:HMLGNDSXX:4:1101:2692 1:N:0:GATCAG";
        assert!(parse_illumina_header(header).is_none());
    }

    #[test]
    fn test_parse_invalid_non_numeric_coords() {
        let header = b"A00488:61:HMLGNDSXX:4:abc:2692:1000 1:N:0:GATCAG";
        assert!(parse_illumina_header(header).is_none());
    }

    #[test]
    fn test_parse_invalid_empty_suffix() {
        let header = b"A00488:61:HMLGNDSXX:4:1101:2692:1000 ";
        assert!(parse_illumina_header(header).is_none());
    }

    #[test]
    fn test_parse_invalid_too_many_fields() {
        let header = b"A:B:C:D:E:F:G:H 1:N:0:X";
        assert!(parse_illumina_header(header).is_none());
    }

    #[test]
    fn test_roundtrip_reconstruct() {
        let header = b"A00488:61:HMLGNDSXX:4:1101:2692:1000 1:N:0:GATCAG";
        let parsed = parse_illumina_header(header).unwrap();
        let mut out = Vec::new();
        reconstruct_illumina_header(
            parsed.prefix,
            parsed.tile,
            parsed.x,
            parsed.y,
            parsed.suffix,
            &mut out,
        );
        assert_eq!(out, header);
    }

    #[test]
    fn test_roundtrip_large_coords() {
        let header = b"INST:1:FC:2:2301:45678:12345 2:Y:0:ACGT+TGCA";
        let parsed = parse_illumina_header(header).unwrap();
        let mut out = Vec::new();
        reconstruct_illumina_header(
            parsed.prefix,
            parsed.tile,
            parsed.x,
            parsed.y,
            parsed.suffix,
            &mut out,
        );
        assert_eq!(out, header);
    }

    #[test]
    fn test_write_u32_ascii() {
        let mut buf = Vec::new();
        write_u32_ascii(&mut buf, 0);
        assert_eq!(buf, b"0");

        buf.clear();
        write_u32_ascii(&mut buf, 12345);
        assert_eq!(buf, b"12345");

        buf.clear();
        write_u32_ascii(&mut buf, u32::MAX);
        assert_eq!(buf, b"4294967295");
    }
}
