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

use crate::stream::traits::{
    read_u16le, read_u32le, read_u64le, read_u8, write_u16le, write_u32le, write_u64le, write_u8,
    StreamRead, StreamWrite,
};
use std::io;

// --- Constants ---

pub const MAGIC: &[u8; 3] = b"HEC";
pub const CONTAINER_VERSION: u8 = 0x04;
pub const CMP_MAGIC: &[u8; 3] = b"CMP";
pub const CMP_VERSION: u8 = 0x01;

pub const FLAG_INDEXED: u8 = 0x01;
pub const FLAG_COMPRESSED: u8 = 0x02;
pub const FLAG_REFERENTIAL: u8 = 0x04;
pub const FLAG_ILLUMINA: u8 = 0x08;

pub const FASTA_STREAM_COUNT: u8 = 5;
pub const FASTQ_STREAM_COUNT: u8 = 6;
pub const ILLUMINA_FASTQ_STREAM_COUNT: u8 = 7;

// Stream IDs
pub const STREAM_CTRL: usize = 0;
pub const STREAM_NUC: usize = 1;
pub const STREAM_CASE: usize = 2;
pub const STREAM_HDR: usize = 3;
pub const STREAM_EXTRA: usize = 4;
pub const STREAM_QUALITY: usize = 5;
pub const STREAM_HDR_COORDS: usize = 6;

// Control stream tags
pub const TAG_HEADER: u8 = 0x01;
pub const TAG_SEQ: u8 = 0x02;
pub const TAG_QUALITY: u8 = 0x03;
pub const TAG_HEADER_RAW: u8 = 0x04;
pub const TAG_EOF: u8 = 0xFF;

// --- Enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphabetMode {
    TwoBit,
    FourBit,
    EightBit,
}

impl AlphabetMode {
    pub fn to_byte(self) -> u8 {
        match self {
            AlphabetMode::TwoBit => b'2',
            AlphabetMode::FourBit => b'4',
            AlphabetMode::EightBit => b'8',
        }
    }

    pub fn from_byte(b: u8) -> io::Result<Self> {
        match b {
            b'2' => Ok(AlphabetMode::TwoBit),
            b'4' => Ok(AlphabetMode::FourBit),
            b'8' => Ok(AlphabetMode::EightBit),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown alphabet mode: 0x{:02X}", b),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseMode {
    Include,
    Ignore,
    Joint,
}

impl CaseMode {
    pub fn to_byte(self) -> u8 {
        match self {
            CaseMode::Include => 0x00,
            CaseMode::Ignore => 0x01,
            CaseMode::Joint => 0x02,
        }
    }

    pub fn from_byte(b: u8) -> io::Result<Self> {
        match b {
            0x00 => Ok(CaseMode::Include),
            0x01 => Ok(CaseMode::Ignore),
            0x02 => Ok(CaseMode::Joint),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown case mode: 0x{:02X}", b),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Fasta,
    Fastq,
}

impl FileType {
    pub fn to_byte(self) -> u8 {
        match self {
            FileType::Fasta => b'F',
            FileType::Fastq => b'Q',
        }
    }

    pub fn from_byte(b: u8) -> io::Result<Self> {
        match b {
            b'F' => Ok(FileType::Fasta),
            b'Q' => Ok(FileType::Fastq),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown file type: 0x{:02X}", b),
            )),
        }
    }

    pub fn stream_count(self) -> u8 {
        match self {
            FileType::Fasta => FASTA_STREAM_COUNT,
            FileType::Fastq => FASTQ_STREAM_COUNT,
        }
    }
}

// --- FileHeader ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHeader {
    pub alphabet: AlphabetMode,
    pub file_type: FileType,
    pub case_mode: CaseMode,
    pub indexed: bool,
    pub compressed: bool,
    pub referential: bool,
    pub illumina: bool,
}

impl FileHeader {
    pub fn stream_count(&self) -> u8 {
        if self.illumina {
            ILLUMINA_FASTQ_STREAM_COUNT
        } else {
            self.file_type.stream_count()
        }
    }

    pub fn validate(&self) -> io::Result<()> {
        if self.case_mode == CaseMode::Joint && self.alphabet != AlphabetMode::EightBit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "joint case mode is only valid with 8-bit alphabet",
            ));
        }
        if self.illumina && self.file_type != FileType::Fastq {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "illumina mode is only valid with FASTQ files",
            ));
        }
        Ok(())
    }

    pub fn write_to(&self, w: &mut dyn StreamWrite) -> io::Result<()> {
        self.validate()?;
        w.write_all(MAGIC)?;
        write_u8(w, CONTAINER_VERSION)?;
        write_u8(w, self.alphabet.to_byte())?;
        write_u8(w, self.file_type.to_byte())?;
        write_u8(w, self.case_mode.to_byte())?;
        let mut flags: u8 = 0;
        if self.indexed {
            flags |= FLAG_INDEXED;
        }
        if self.compressed {
            flags |= FLAG_COMPRESSED;
        }
        if self.referential {
            flags |= FLAG_REFERENTIAL;
        }
        if self.illumina {
            flags |= FLAG_ILLUMINA;
        }
        write_u8(w, flags)?;
        Ok(())
    }

    pub fn read_from(r: &mut dyn StreamRead) -> io::Result<Self> {
        let mut magic = [0u8; 3];
        r.read_exact(&mut magic)?;
        if &magic != MAGIC {
            let hint = if magic[0] == b'>' {
                " (file looks like FASTA, not a .he archive)"
            } else if magic[0] == b'@' {
                " (file looks like FASTQ, not a .he archive)"
            } else {
                ""
            };
            let got = if magic.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
                format!("\"{}\"", std::str::from_utf8(&magic).unwrap_or("???"))
            } else {
                format!("{:?}", magic)
            };
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "not a hecate file (expected magic \"HEC\", got {}){}",
                    got, hint
                ),
            ));
        }
        let version = read_u8(r)?;
        if version != CONTAINER_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported container version: {}", version),
            ));
        }
        let alphabet = AlphabetMode::from_byte(read_u8(r)?)?;
        let file_type = FileType::from_byte(read_u8(r)?)?;
        let case_mode = CaseMode::from_byte(read_u8(r)?)?;
        let flags = read_u8(r)?;

        let hdr = FileHeader {
            alphabet,
            file_type,
            case_mode,
            indexed: flags & FLAG_INDEXED != 0,
            compressed: flags & FLAG_COMPRESSED != 0,
            referential: flags & FLAG_REFERENTIAL != 0,
            illumina: flags & FLAG_ILLUMINA != 0,
        };
        hdr.validate()?;
        Ok(hdr)
    }
}

// --- MethodSpec ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodSpec {
    Zstd {
        level: i8,
        window_log: u32,
    },
    Xz {
        preset: u8,
        extreme: bool,
        dict_size: u32,
        lc: u8,
        pb: u8,
    },
    Bwt {
        block_mib: u16,
    },
    Markov {
        four_bit: bool,
    },
    Rans,
}

impl MethodSpec {
    pub fn write_to(&self, w: &mut dyn StreamWrite) -> io::Result<()> {
        match self {
            MethodSpec::Zstd { level, window_log } => {
                write_u8(w, b'Z')?;
                write_u8(w, *level as u8)?;
                write_u32le(w, *window_log)?;
            }
            MethodSpec::Xz {
                preset,
                extreme,
                dict_size,
                lc,
                pb,
            } => {
                write_u8(w, b'X')?;
                let p = (*preset & 0x0F) | if *extreme { 0x80 } else { 0 };
                write_u8(w, p)?;
                write_u32le(w, *dict_size)?;
                write_u8(w, *lc)?;
                write_u8(w, *pb)?;
            }
            MethodSpec::Bwt { block_mib } => {
                write_u8(w, b'B')?;
                write_u16le(w, *block_mib)?;
            }
            MethodSpec::Markov { four_bit } => {
                write_u8(w, b'M')?;
                write_u8(w, *four_bit as u8)?;
            }
            MethodSpec::Rans => {
                write_u8(w, b'R')?;
            }
        }
        Ok(())
    }

    pub fn read_from(r: &mut dyn StreamRead) -> io::Result<Self> {
        let tag = read_u8(r)?;
        match tag {
            b'Z' => {
                let level = read_u8(r)? as i8;
                let window_log = read_u32le(r)?;
                Ok(MethodSpec::Zstd { level, window_log })
            }
            b'X' => {
                let p = read_u8(r)?;
                let preset = p & 0x0F;
                let extreme = (p & 0x80) != 0;
                let dict_size = read_u32le(r)?;
                let lc = read_u8(r)?;
                let pb = read_u8(r)?;
                Ok(MethodSpec::Xz {
                    preset,
                    extreme,
                    dict_size,
                    lc,
                    pb,
                })
            }
            b'B' => {
                let block_mib = read_u16le(r)?;
                Ok(MethodSpec::Bwt { block_mib })
            }
            b'M' => {
                let four_bit = read_u8(r)? != 0;
                Ok(MethodSpec::Markov { four_bit })
            }
            b'R' => Ok(MethodSpec::Rans),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown compression method tag: 0x{:02X}", tag),
            )),
        }
    }
}

// --- CompressionHeader ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionHeader {
    pub stream_count: u8,
    pub methods: Vec<MethodSpec>,
    pub pattern: Vec<u8>,
}

impl CompressionHeader {
    pub fn validate(&self, expected_stream_count: u8) -> io::Result<()> {
        if self.stream_count != expected_stream_count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "compression header stream_count {} does not match expected {}",
                    self.stream_count, expected_stream_count
                ),
            ));
        }
        if self.pattern.len() != self.stream_count as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "pattern length {} does not match stream_count {}",
                    self.pattern.len(),
                    self.stream_count
                ),
            ));
        }
        for &idx in &self.pattern {
            if idx != 0xFF && idx as usize >= self.methods.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "pattern index {} out of range (have {} methods)",
                        idx,
                        self.methods.len()
                    ),
                ));
            }
        }
        Ok(())
    }

    pub fn write_to(&self, w: &mut dyn StreamWrite) -> io::Result<()> {
        w.write_all(CMP_MAGIC)?;
        write_u8(w, CMP_VERSION)?;
        write_u8(w, self.stream_count)?;
        write_u8(w, self.methods.len() as u8)?;
        for m in &self.methods {
            m.write_to(w)?;
        }
        w.write_all(&self.pattern)?;
        Ok(())
    }

    pub fn read_from(r: &mut dyn StreamRead) -> io::Result<Self> {
        let mut magic = [0u8; 3];
        r.read_exact(&mut magic)?;
        if &magic != CMP_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bad compression header magic: {:?}", magic),
            ));
        }
        let version = read_u8(r)?;
        if version != CMP_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported compression header version: {}", version),
            ));
        }
        let stream_count = read_u8(r)?;
        let method_count = read_u8(r)?;

        let mut methods = Vec::with_capacity(method_count as usize);
        for _ in 0..method_count {
            methods.push(MethodSpec::read_from(r)?);
        }

        let mut pattern = vec![0u8; stream_count as usize];
        r.read_exact(&mut pattern)?;

        // Validate: every pattern index must refer to a valid method or be 0xFF (none).
        for (i, &idx) in pattern.iter().enumerate() {
            if idx != 0xFF && idx as usize >= methods.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "compression pattern[{}] = {} but only {} methods defined",
                        i,
                        idx,
                        methods.len()
                    ),
                ));
            }
        }

        Ok(CompressionHeader {
            stream_count,
            methods,
            pattern,
        })
    }
}

// --- Trailer ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trailer {
    pub index_offset: u64,
}

impl Trailer {
    pub const SIZE: usize = 11;

    pub fn write_to(&self, w: &mut dyn StreamWrite) -> io::Result<()> {
        write_u64le(w, self.index_offset)?;
        w.write_all(MAGIC)?;
        Ok(())
    }

    pub fn read_from(r: &mut dyn StreamRead) -> io::Result<Self> {
        let index_offset = read_u64le(r)?;
        let mut magic = [0u8; 3];
        r.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bad trailer magic: {:?}", magic),
            ));
        }
        Ok(Trailer { index_offset })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: write bytes to a Vec, then read them back.
    struct VecStream {
        data: Vec<u8>,
        pos: usize,
    }

    impl VecStream {
        fn new() -> Self {
            Self {
                data: Vec::new(),
                pos: 0,
            }
        }
        fn from_bytes(data: Vec<u8>) -> Self {
            Self { data, pos: 0 }
        }
    }

    impl StreamWrite for VecStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl StreamRead for VecStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let avail = self.data.len() - self.pos;
            let n = avail.min(buf.len());
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn test_file_header_round_trip() {
        let combos = [
            (AlphabetMode::TwoBit, FileType::Fasta, CaseMode::Include),
            (AlphabetMode::TwoBit, FileType::Fasta, CaseMode::Ignore),
            (AlphabetMode::FourBit, FileType::Fasta, CaseMode::Include),
            (AlphabetMode::FourBit, FileType::Fasta, CaseMode::Ignore),
            (AlphabetMode::EightBit, FileType::Fasta, CaseMode::Include),
            (AlphabetMode::EightBit, FileType::Fasta, CaseMode::Ignore),
            (AlphabetMode::EightBit, FileType::Fasta, CaseMode::Joint),
            (AlphabetMode::TwoBit, FileType::Fastq, CaseMode::Include),
            (AlphabetMode::TwoBit, FileType::Fastq, CaseMode::Ignore),
            (AlphabetMode::FourBit, FileType::Fastq, CaseMode::Include),
            (AlphabetMode::FourBit, FileType::Fastq, CaseMode::Ignore),
            (AlphabetMode::EightBit, FileType::Fastq, CaseMode::Include),
            (AlphabetMode::EightBit, FileType::Fastq, CaseMode::Ignore),
            (AlphabetMode::EightBit, FileType::Fastq, CaseMode::Joint),
        ];
        for (alphabet, file_type, case_mode) in &combos {
            for indexed in [false, true] {
                for compressed in [false, true] {
                    let hdr = FileHeader {
                        alphabet: *alphabet,
                        file_type: *file_type,
                        case_mode: *case_mode,
                        indexed,
                        compressed,
                        referential: false,
                        illumina: false,
                    };
                    let mut ws = VecStream::new();
                    hdr.write_to(&mut ws).unwrap();
                    assert_eq!(ws.data.len(), 8);

                    let mut rs = VecStream::from_bytes(ws.data);
                    let hdr2 = FileHeader::read_from(&mut rs).unwrap();
                    assert_eq!(hdr, hdr2);
                }
            }
        }
    }

    #[test]
    fn test_file_header_joint_with_non_8bit_error() {
        let hdr = FileHeader {
            alphabet: AlphabetMode::TwoBit,
            file_type: FileType::Fasta,
            case_mode: CaseMode::Joint,
            indexed: false,
            compressed: false,
            referential: false,
            illumina: false,
        };
        let mut ws = VecStream::new();
        assert!(hdr.write_to(&mut ws).is_err());
    }

    #[test]
    fn test_file_header_bad_magic() {
        let data = vec![b'X', b'Y', b'Z', 0x03, b'2', b'F', 0x00, 0x00];
        let mut rs = VecStream::from_bytes(data);
        assert!(FileHeader::read_from(&mut rs).is_err());
    }

    #[test]
    fn test_file_header_bad_version() {
        let data = vec![b'H', b'E', b'C', 0x99, b'2', b'F', 0x00, 0x00];
        let mut rs = VecStream::from_bytes(data);
        assert!(FileHeader::read_from(&mut rs).is_err());
    }

    #[test]
    fn test_compression_header_round_trip() {
        let ch = CompressionHeader {
            stream_count: 5,
            methods: vec![
                MethodSpec::Zstd {
                    level: 3,
                    window_log: 0,
                },
                MethodSpec::Bwt { block_mib: 32 },
            ],
            pattern: vec![0x00, 0x01, 0x00, 0xFF, 0x00],
        };

        let mut ws = VecStream::new();
        ch.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let ch2 = CompressionHeader::read_from(&mut rs).unwrap();
        assert_eq!(ch, ch2);
    }

    #[test]
    fn test_compression_header_validation() {
        // Wrong stream count for FASTA
        let ch = CompressionHeader {
            stream_count: 6,
            methods: vec![MethodSpec::Zstd {
                level: 3,
                window_log: 0,
            }],
            pattern: vec![0x00; 6],
        };
        assert!(ch.validate(FASTA_STREAM_COUNT).is_err());
        assert!(ch.validate(FASTQ_STREAM_COUNT).is_ok());
    }

    #[test]
    fn test_compression_header_pattern_out_of_range() {
        let ch = CompressionHeader {
            stream_count: 5,
            methods: vec![MethodSpec::Zstd {
                level: 3,
                window_log: 0,
            }],
            pattern: vec![0x00, 0x01, 0x00, 0xFF, 0x00], // 0x01 out of range (only 1 method)
        };
        assert!(ch.validate(FASTA_STREAM_COUNT).is_err());
    }

    #[test]
    fn test_compression_header_empty_methods() {
        let ch = CompressionHeader {
            stream_count: 5,
            methods: vec![],
            pattern: vec![0xFF; 5], // all raw
        };
        let mut ws = VecStream::new();
        ch.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let ch2 = CompressionHeader::read_from(&mut rs).unwrap();
        assert_eq!(ch, ch2);
        assert!(ch2.validate(FASTA_STREAM_COUNT).is_ok());
    }

    #[test]
    fn test_trailer_round_trip() {
        let t = Trailer {
            index_offset: 0x12345678_9ABCDEF0,
        };
        let mut ws = VecStream::new();
        t.write_to(&mut ws).unwrap();
        assert_eq!(ws.data.len(), 11);

        let mut rs = VecStream::from_bytes(ws.data);
        let t2 = Trailer::read_from(&mut rs).unwrap();
        assert_eq!(t, t2);
    }

    #[test]
    fn test_trailer_zero_index() {
        let t = Trailer { index_offset: 0 };
        let mut ws = VecStream::new();
        t.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let t2 = Trailer::read_from(&mut rs).unwrap();
        assert_eq!(t2.index_offset, 0);
    }

    #[test]
    fn test_trailer_bad_magic() {
        let mut data = vec![0u8; 8]; // index_offset = 0
        data.extend_from_slice(b"BAD");
        let mut rs = VecStream::from_bytes(data);
        assert!(Trailer::read_from(&mut rs).is_err());
    }

    #[test]
    fn test_method_spec_zstd_round_trip() {
        let m = MethodSpec::Zstd {
            level: 19,
            window_log: 24,
        };
        let mut ws = VecStream::new();
        m.write_to(&mut ws).unwrap();
        assert_eq!(ws.data.len(), 6);

        let mut rs = VecStream::from_bytes(ws.data);
        let m2 = MethodSpec::read_from(&mut rs).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn test_method_spec_bwt_round_trip() {
        let m = MethodSpec::Bwt { block_mib: 64 };
        let mut ws = VecStream::new();
        m.write_to(&mut ws).unwrap();
        assert_eq!(ws.data.len(), 3);

        let mut rs = VecStream::from_bytes(ws.data);
        let m2 = MethodSpec::read_from(&mut rs).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn test_method_spec_markov_round_trip() {
        for four_bit in [false, true] {
            let m = MethodSpec::Markov { four_bit };
            let mut ws = VecStream::new();
            m.write_to(&mut ws).unwrap();
            assert_eq!(ws.data.len(), 2);

            let mut rs = VecStream::from_bytes(ws.data);
            let m2 = MethodSpec::read_from(&mut rs).unwrap();
            assert_eq!(m, m2);
        }
    }

    #[test]
    fn test_method_spec_unknown_tag() {
        let data = vec![b'X', 0, 0];
        let mut rs = VecStream::from_bytes(data);
        assert!(MethodSpec::read_from(&mut rs).is_err());
    }

    #[test]
    fn test_mixed_methods_round_trip() {
        let ch = CompressionHeader {
            stream_count: 6,
            methods: vec![
                MethodSpec::Zstd {
                    level: 3,
                    window_log: 0,
                },
                MethodSpec::Bwt { block_mib: 32 },
                MethodSpec::Zstd {
                    level: 19,
                    window_log: 28,
                },
            ],
            pattern: vec![0x00, 0x01, 0x00, 0xFF, 0x02, 0x00],
        };

        let mut ws = VecStream::new();
        ch.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let ch2 = CompressionHeader::read_from(&mut rs).unwrap();
        assert_eq!(ch, ch2);
        assert!(ch2.validate(FASTQ_STREAM_COUNT).is_ok());
    }
}
