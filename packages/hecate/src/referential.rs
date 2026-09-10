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

use crate::container::{AlphabetMode, CompressionHeader, FileHeader};
use crate::mux::reader::StreamingMuxReader;
use crate::stream::traits::StreamRead;
use std::io;

// --- Unsqueeze / Squeeze ---

/// Expand packed nucleotide bytes into individual code values.
/// 2-bit: 1 byte -> 4 codes (0-3), 4-bit: 1 byte -> 2 codes (0-15), 8-bit: identity.
pub fn unsqueeze_nuc(alphabet: AlphabetMode, packed: &[u8]) -> Vec<u8> {
    match alphabet {
        AlphabetMode::TwoBit => {
            let mut out = Vec::with_capacity(packed.len() * 4);
            for &b in packed {
                out.push(b & 0x03);
                out.push((b >> 2) & 0x03);
                out.push((b >> 4) & 0x03);
                out.push((b >> 6) & 0x03);
            }
            out
        }
        AlphabetMode::FourBit => {
            let mut out = Vec::with_capacity(packed.len() * 2);
            for &b in packed {
                out.push(b & 0x0F);
                out.push((b >> 4) & 0x0F);
            }
            out
        }
        AlphabetMode::EightBit => packed.to_vec(),
    }
}

/// Re-pack individual code values back into packed nucleotide bytes.
/// 2-bit: 4 codes -> 1 byte, 4-bit: 2 codes -> 1 byte, 8-bit: identity.
pub fn squeeze_nuc(alphabet: AlphabetMode, expanded: &[u8]) -> Vec<u8> {
    match alphabet {
        AlphabetMode::TwoBit => {
            let mut out = Vec::with_capacity(expanded.len().div_ceil(4));
            for chunk in expanded.chunks(4) {
                let mut byte = chunk[0] & 0x03;
                if chunk.len() > 1 {
                    byte |= (chunk[1] & 0x03) << 2;
                }
                if chunk.len() > 2 {
                    byte |= (chunk[2] & 0x03) << 4;
                }
                if chunk.len() > 3 {
                    byte |= (chunk[3] & 0x03) << 6;
                }
                out.push(byte);
            }
            out
        }
        AlphabetMode::FourBit => {
            let mut out = Vec::with_capacity(expanded.len().div_ceil(2));
            for chunk in expanded.chunks(2) {
                let mut byte = chunk[0] & 0x0F;
                if chunk.len() > 1 {
                    byte |= (chunk[1] & 0x0F) << 4;
                }
                out.push(byte);
            }
            out
        }
        AlphabetMode::EightBit => expanded.to_vec(),
    }
}

// --- ReferentialMeta ---

const REF_META_MAGIC: u8 = b'R';

/// Serialize metadata needed to reconstruct the target file from a referential diff.
///
/// Format: [magic b'R'] [FileHeader 8 bytes] [cmp_len: u32le] [CompressionHeader bytes]
///         [stream_count: u8] [per-stream original uncompressed size: u64le * stream_count]
pub fn serialize_ref_meta(
    header: &FileHeader,
    cmp: &CompressionHeader,
    sizes: &[u64],
) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    buf.push(REF_META_MAGIC);
    // Write FileHeader into temp buffer to get its bytes
    let mut hdr_buf = VecWriter(Vec::new());
    header.write_to(&mut hdr_buf)?;
    buf.extend_from_slice(&hdr_buf.0);
    // Write CompressionHeader into temp buffer
    let mut cmp_buf = VecWriter(Vec::new());
    cmp.write_to(&mut cmp_buf)?;
    let cmp_bytes = &cmp_buf.0;
    // cmp_len as u32le
    buf.extend_from_slice(&(cmp_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(cmp_bytes);
    // stream_count + per-stream sizes
    buf.push(sizes.len() as u8);
    for &sz in sizes {
        buf.extend_from_slice(&sz.to_le_bytes());
    }
    Ok(buf)
}

/// Parse metadata from the beginning of a referential diff's stream 0 data.
/// Returns (FileHeader, CompressionHeader, per-stream original sizes, bytes consumed).
pub fn parse_ref_meta(data: &[u8]) -> io::Result<(FileHeader, CompressionHeader, Vec<u64>, usize)> {
    if data.is_empty() || data[0] != REF_META_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing referential metadata magic",
        ));
    }
    let mut cur = SliceReader { data, pos: 1 };
    let header = FileHeader::read_from(&mut cur)?;
    let cmp_len = {
        let mut b = [0u8; 4];
        cur.read_exact(&mut b)?;
        u32::from_le_bytes(b) as usize
    };
    // Validate cmp_len doesn't exceed remaining data
    if cur.pos + cmp_len > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "referential metadata truncated (compression header)",
        ));
    }
    let cmp = CompressionHeader::read_from(&mut cur)?;
    let stream_count = {
        let mut b = [0u8; 1];
        cur.read_exact(&mut b)?;
        b[0] as usize
    };
    let mut sizes = Vec::with_capacity(stream_count);
    for _ in 0..stream_count {
        let mut b = [0u8; 8];
        cur.read_exact(&mut b)?;
        sizes.push(u64::from_le_bytes(b));
    }
    Ok((header, cmp, sizes, cur.pos))
}

// --- Stream draining helper ---

/// Drain all data from every stream in a StreamingMuxReader into Vec<Vec<u8>>.
/// The mux reader may encounter the file trailer when trying to pump the next
/// block after all real blocks have been consumed; this is handled gracefully
/// by catching UnexpectedEof.
pub fn drain_all_streams(
    reader: &mut StreamingMuxReader<impl StreamRead>,
    stream_count: usize,
) -> io::Result<Vec<Vec<u8>>> {
    let mut results: Vec<Vec<u8>> = (0..stream_count).map(|_| Vec::new()).collect();
    loop {
        let mut any_read = false;
        for s in 0..stream_count {
            // Use ensure_available(1) to pump at most one block, then borrow
            // whatever is available. This avoids the issue where read_stream
            // with a large buffer keeps pumping past the end of block data.
            match reader.ensure_available(s, 1) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    // Hit the trailer -- drain any remaining buffered data
                    // from all streams before returning.  This handles the
                    // case where a previous pump loaded data for earlier
                    // streams that we haven't consumed yet.
                    for s2 in 0..stream_count {
                        let data = reader.borrow_stream(s2);
                        if !data.is_empty() {
                            results[s2].extend_from_slice(data);
                            let n = data.len();
                            reader.advance_stream(s2, n);
                        }
                    }
                    return Ok(results);
                }
                Err(e) => return Err(e),
            }
            let data = reader.borrow_stream(s);
            if !data.is_empty() {
                results[s].extend_from_slice(data);
                let n = data.len();
                reader.advance_stream(s, n);
                any_read = true;
            }
        }
        if !any_read {
            break;
        }
    }
    Ok(results)
}

// --- Internal helpers ---

struct VecWriter(Vec<u8>);

impl crate::stream::traits::StreamWrite for VecWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct SliceReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl StreamRead for SliceReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let avail = self.data.len() - self.pos;
        let n = avail.min(buf.len());
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::*;

    #[test]
    fn test_unsqueeze_squeeze_2bit_roundtrip() {
        // Pack CGAT = 0b11_10_01_00
        let packed = vec![0b11_10_01_00u8, 0b00_01_10_11];
        let expanded = unsqueeze_nuc(AlphabetMode::TwoBit, &packed);
        assert_eq!(expanded, vec![0, 1, 2, 3, 3, 2, 1, 0]);
        let repacked = squeeze_nuc(AlphabetMode::TwoBit, &expanded);
        assert_eq!(repacked, packed);
    }

    #[test]
    fn test_unsqueeze_squeeze_4bit_roundtrip() {
        let packed = vec![0x21u8, 0x84]; // codes: 1,2, 4,8
        let expanded = unsqueeze_nuc(AlphabetMode::FourBit, &packed);
        assert_eq!(expanded, vec![1, 2, 4, 8]);
        let repacked = squeeze_nuc(AlphabetMode::FourBit, &expanded);
        assert_eq!(repacked, packed);
    }

    #[test]
    fn test_unsqueeze_squeeze_8bit_identity() {
        let data = vec![65, 67, 71, 84];
        let expanded = unsqueeze_nuc(AlphabetMode::EightBit, &data);
        assert_eq!(expanded, data);
        let repacked = squeeze_nuc(AlphabetMode::EightBit, &expanded);
        assert_eq!(repacked, data);
    }

    #[test]
    fn test_unsqueeze_squeeze_2bit_partial() {
        // 3 bases in 1 byte (last 2 bits are padding)
        let packed = vec![0b00_10_01_00u8]; // C=0, G=1, A=2, pad=0
        let expanded = unsqueeze_nuc(AlphabetMode::TwoBit, &packed);
        assert_eq!(expanded, vec![0, 1, 2, 0]); // includes padding
                                                // If we know only 3 bases, we truncate after unsqueeze before diffing
        let trimmed = &expanded[..3];
        let repacked = squeeze_nuc(AlphabetMode::TwoBit, trimmed);
        assert_eq!(repacked, vec![0b00_10_01_00]); // padding bits zero
    }

    #[test]
    fn test_unsqueeze_squeeze_4bit_partial() {
        // 1 base in 1 byte (upper nibble is padding)
        let packed = vec![0x05u8]; // code 5, pad 0
        let expanded = unsqueeze_nuc(AlphabetMode::FourBit, &packed);
        assert_eq!(expanded, vec![5, 0]); // includes padding
        let trimmed = &expanded[..1];
        let repacked = squeeze_nuc(AlphabetMode::FourBit, trimmed);
        assert_eq!(repacked, vec![0x05]);
    }

    #[test]
    fn test_ref_meta_roundtrip() {
        let header = FileHeader {
            alphabet: AlphabetMode::TwoBit,
            file_type: FileType::Fasta,
            case_mode: CaseMode::Include,
            indexed: false,
            compressed: true,
            referential: false,
            illumina: false,
        };
        let cmp = CompressionHeader {
            stream_count: 5,
            methods: vec![MethodSpec::Zstd {
                level: 3,
                window_log: 0,
            }],
            pattern: vec![0x00, 0x00, 0x00, 0xFF, 0x00],
        };
        let sizes = vec![100, 2000, 50, 300, 10];

        let serialized = serialize_ref_meta(&header, &cmp, &sizes).unwrap();
        let (hdr2, cmp2, sizes2, consumed) = parse_ref_meta(&serialized).unwrap();
        assert_eq!(hdr2, header);
        assert_eq!(cmp2, cmp);
        assert_eq!(sizes2, sizes);
        assert_eq!(consumed, serialized.len());
    }
}
