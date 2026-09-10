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

use crate::codec::common::*;
use crate::codec::illumina::parse_illumina_header;
use crate::codec::varint::encode_varint_u32;
use crate::container::*;
use crate::mux::writer::MuxWriter;
use crate::stream::traits::{BorrowRead, StreamWrite};
use memchr::memchr;
use std::io;

/// Maximum single-sequence allocation (16 GiB) to prevent OOM from crafted files.
const MAX_SEQ_ALLOC: u64 = 16 << 30;

/// Encode FASTQ text into a Hecate mux writer.
pub fn encode_fastq(
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    illumina: bool,
    quality_lut: Option<&[u8; 256]>,
    input: &mut dyn BorrowRead,
    mux: &mut MuxWriter<impl StreamWrite>,
) -> io::Result<()> {
    match (alphabet, case_mode) {
        (AlphabetMode::TwoBit, CaseMode::Include) => {
            encode_fastq_inner::<2, 0>(illumina, quality_lut, input, mux)
        }
        (AlphabetMode::TwoBit, CaseMode::Ignore) => {
            encode_fastq_inner::<2, 1>(illumina, quality_lut, input, mux)
        }
        (AlphabetMode::FourBit, CaseMode::Include) => {
            encode_fastq_inner::<4, 0>(illumina, quality_lut, input, mux)
        }
        (AlphabetMode::FourBit, CaseMode::Ignore) => {
            encode_fastq_inner::<4, 1>(illumina, quality_lut, input, mux)
        }
        (AlphabetMode::EightBit, CaseMode::Include) => {
            encode_fastq_inner::<8, 0>(illumina, quality_lut, input, mux)
        }
        (AlphabetMode::EightBit, CaseMode::Ignore) => {
            encode_fastq_inner::<8, 1>(illumina, quality_lut, input, mux)
        }
        (AlphabetMode::EightBit, CaseMode::Joint) => {
            encode_fastq_inner::<8, 2>(illumina, quality_lut, input, mux)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid alphabet/case combination",
        )),
    }
}

#[derive(PartialEq)]
enum FastqState {
    BeforeRecord,
    InHeader,
    InSequence,
    InPlus,
    InQuality,
}

fn encode_fastq_inner<const ALPHABET: u8, const CASE_MODE: u8>(
    illumina: bool,
    quality_lut: Option<&[u8; 256]>,
    input: &mut dyn BorrowRead,
    mux: &mut MuxWriter<impl StreamWrite>,
) -> io::Result<()> {
    let mut state = FastqState::BeforeRecord;
    let mut header_buf = Vec::new();
    let mut nuc_buf = Vec::new();
    let mut extra_buf = Vec::new();
    let mut quality_buf = Vec::new();
    let mut case_enc = CaseEncoder::new();
    let mut bit_pos = 0u8;
    let mut base_count = 0u64;
    let mut extra_count = 0u32;
    let mut last_extra_pos = 0u64;
    let mut quality_count = 0u64;
    let mut at_line_start = true;

    loop {
        let chunk = input.fill_buf(1)?;
        if chunk.is_empty() {
            break;
        }
        let chunk_len = chunk.len();

        let mut i = 0;
        while i < chunk_len {
            match state {
                FastqState::BeforeRecord => {
                    // Scan for '@' using memchr
                    match memchr(b'@', &chunk[i..]) {
                        Some(pos) => {
                            for &b in &chunk[i..i + pos] {
                                if b != b'\n' && b != b'\r' && b != b' ' && b != b'\t' {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "expected '@' at FASTQ record start",
                                    ));
                                }
                            }
                            i += pos + 1;
                            state = FastqState::InHeader;
                        }
                        None => {
                            for &b in &chunk[i..] {
                                if b != b'\n' && b != b'\r' && b != b' ' && b != b'\t' {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "expected '@' at FASTQ record start",
                                    ));
                                }
                            }
                            i = chunk_len;
                        }
                    }
                }
                FastqState::InHeader => {
                    // Find end of header line
                    match memchr(b'\n', &chunk[i..]) {
                        Some(pos) => {
                            let line = &chunk[i..i + pos];
                            if line.last() == Some(&b'\r') {
                                header_buf.extend_from_slice(&line[..line.len() - 1]);
                            } else {
                                header_buf.extend_from_slice(line);
                            }
                            mux.begin_sequence(&header_buf, bit_pos);
                            if illumina {
                                if let Some(parsed) = parse_illumina_header(&header_buf) {
                                    emit_header_illumina(mux, &parsed)?;
                                } else {
                                    emit_header_raw_fallback(mux, &header_buf)?;
                                }
                            } else {
                                emit_header_fastq(mux, &header_buf)?;
                            }
                            header_buf.clear();
                            i += pos + 1;
                            state = FastqState::InSequence;
                            at_line_start = true;
                        }
                        None => {
                            for &b in &chunk[i..] {
                                if b != b'\r' {
                                    header_buf.push(b);
                                }
                            }
                            i = chunk_len;
                        }
                    }
                }
                FastqState::InSequence => {
                    // Find next newline using memchr
                    match memchr(b'\n', &chunk[i..]) {
                        Some(pos) => {
                            let line = &chunk[i..i + pos];
                            let line = if line.last() == Some(&b'\r') {
                                &line[..line.len() - 1]
                            } else {
                                line
                            };
                            if !line.is_empty() && line[0] == b'+' && at_line_start {
                                if base_count == 0 {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "empty sequence in FASTQ",
                                    ));
                                }
                                i += pos + 1;
                                state = FastqState::InQuality;
                                quality_count = 0;
                            } else {
                                // Batch-encode nucleotides on this line
                                crate::codec::fasta::encode_sequence_batch_pub::<ALPHABET, CASE_MODE>(
                                    line,
                                    &mut nuc_buf,
                                    &mut case_enc,
                                    &mut extra_buf,
                                    &mut bit_pos,
                                    &mut base_count,
                                    &mut extra_count,
                                    &mut last_extra_pos,
                                );
                                i += pos + 1;
                                at_line_start = true;
                            }
                        }
                        None => {
                            // No newline found: encode remaining bytes
                            let remaining = &chunk[i..];
                            if !remaining.is_empty() && remaining[0] == b'+' && at_line_start {
                                if base_count == 0 {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "empty sequence in FASTQ",
                                    ));
                                }
                                state = FastqState::InPlus;
                                i = chunk_len;
                            } else {
                                // Filter out \r, then batch-encode
                                for &byte in remaining {
                                    if byte != b'\r' {
                                        at_line_start = false;
                                        encode_base_fastq::<ALPHABET, CASE_MODE>(
                                            byte,
                                            &mut nuc_buf,
                                            &mut case_enc,
                                            &mut extra_buf,
                                            &mut bit_pos,
                                            &mut base_count,
                                            &mut extra_count,
                                            &mut last_extra_pos,
                                        );
                                    }
                                }
                                i = chunk_len;
                            }
                        }
                    }
                }
                FastqState::InPlus => {
                    // Find end of + line
                    match memchr(b'\n', &chunk[i..]) {
                        Some(pos) => {
                            i += pos + 1;
                            state = FastqState::InQuality;
                            quality_count = 0;
                        }
                        None => {
                            i = chunk_len;
                        }
                    }
                }
                FastqState::InQuality => {
                    // Find next newline, batch-copy quality bytes
                    match memchr(b'\n', &chunk[i..]) {
                        Some(pos) => {
                            let line = &chunk[i..i + pos];
                            let line = if line.last() == Some(&b'\r') {
                                &line[..line.len() - 1]
                            } else {
                                line
                            };
                            let remaining_needed = base_count - quality_count;
                            let take = line.len().min(remaining_needed as usize);
                            quality_buf.extend_from_slice(&line[..take]);
                            quality_count += take as u64;
                            i += pos + 1;

                            if quality_count == base_count {
                                if let Some(lut) = quality_lut {
                                    for q in quality_buf.iter_mut() {
                                        *q = lut[*q as usize];
                                    }
                                }
                                emit_sequence_fastq::<CASE_MODE>(
                                    mux,
                                    &nuc_buf,
                                    &extra_buf,
                                    &mut case_enc,
                                    base_count,
                                    extra_count,
                                    &quality_buf,
                                )?;
                                mux.end_sequence(base_count);
                                nuc_buf.clear();
                                extra_buf.clear();
                                quality_buf.clear();
                                case_enc.reset();
                                bit_pos = 0;
                                base_count = 0;
                                extra_count = 0;
                                last_extra_pos = 0;
                                quality_count = 0;
                                state = FastqState::BeforeRecord;
                            }
                        }
                        None => {
                            // No newline: batch-copy quality bytes from remaining
                            let remaining = &chunk[i..];
                            for &byte in remaining {
                                if byte != b'\n' && byte != b'\r' {
                                    quality_buf.push(byte);
                                    quality_count += 1;

                                    if quality_count == base_count {
                                        if let Some(lut) = quality_lut {
                                            for q in quality_buf.iter_mut() {
                                                *q = lut[*q as usize];
                                            }
                                        }
                                        emit_sequence_fastq::<CASE_MODE>(
                                            mux,
                                            &nuc_buf,
                                            &extra_buf,
                                            &mut case_enc,
                                            base_count,
                                            extra_count,
                                            &quality_buf,
                                        )?;
                                        mux.end_sequence(base_count);
                                        nuc_buf.clear();
                                        extra_buf.clear();
                                        quality_buf.clear();
                                        case_enc.reset();
                                        bit_pos = 0;
                                        base_count = 0;
                                        extra_count = 0;
                                        last_extra_pos = 0;
                                        quality_count = 0;
                                        state = FastqState::BeforeRecord;
                                        break;
                                    }
                                }
                            }
                            i = chunk_len;
                        }
                    }
                }
            }
        }

        input.consume(chunk_len);
    }

    // Check for truncated input
    if state == FastqState::InSequence
        || state == FastqState::InPlus
        || state == FastqState::InQuality
    {
        if state == FastqState::InQuality && quality_count > 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "quality length mismatch: expected {} got {}",
                    base_count, quality_count
                ),
            ));
        }
        if state != FastqState::BeforeRecord && base_count > 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated FASTQ input",
            ));
        }
    }

    mux.write_to_stream(STREAM_CTRL, &[TAG_EOF])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn encode_base_fastq<const ALPHABET: u8, const CASE_MODE: u8>(
    byte: u8,
    nuc_buf: &mut Vec<u8>,
    case_enc: &mut CaseEncoder,
    extra_buf: &mut Vec<u8>,
    bit_pos: &mut u8,
    base_count: &mut u64,
    extra_count: &mut u32,
    last_extra_pos: &mut u64,
) {
    crate::codec::fasta::encode_base_pub::<ALPHABET, CASE_MODE>(
        byte,
        nuc_buf,
        case_enc,
        extra_buf,
        bit_pos,
        base_count,
        extra_count,
        last_extra_pos,
    );
}

fn emit_header_fastq(mux: &mut MuxWriter<impl StreamWrite>, header: &[u8]) -> io::Result<()> {
    let mut ctrl = [0u8; 5];
    ctrl[0] = TAG_HEADER;
    ctrl[1..5].copy_from_slice(&(header.len() as u32).to_le_bytes());
    mux.write_to_stream(STREAM_CTRL, &ctrl)?;
    mux.write_to_stream(STREAM_HDR, header)?;
    Ok(())
}

fn emit_header_illumina(
    mux: &mut MuxWriter<impl StreamWrite>,
    parsed: &crate::codec::illumina::IlluminaHeader<'_>,
) -> io::Result<()> {
    // Build tokenized header: prefix + 0x00 + suffix
    let text_len = parsed.prefix.len() + 1 + parsed.suffix.len();
    let mut ctrl = [0u8; 5];
    ctrl[0] = TAG_HEADER;
    ctrl[1..5].copy_from_slice(&(text_len as u32).to_le_bytes());
    mux.write_to_stream(STREAM_CTRL, &ctrl)?;

    // Write prefix + null separator + suffix to STREAM_HDR
    mux.write_to_stream(STREAM_HDR, parsed.prefix)?;
    mux.write_to_stream(STREAM_HDR, &[0x00])?;
    mux.write_to_stream(STREAM_HDR, parsed.suffix)?;

    // Write varint-encoded coordinates to STREAM_HDR_COORDS
    let mut coord_buf = Vec::new();
    encode_varint_u32(&mut coord_buf, parsed.tile);
    encode_varint_u32(&mut coord_buf, parsed.x);
    encode_varint_u32(&mut coord_buf, parsed.y);
    mux.write_to_stream(STREAM_HDR_COORDS, &coord_buf)?;

    Ok(())
}

fn emit_header_raw_fallback(
    mux: &mut MuxWriter<impl StreamWrite>,
    header: &[u8],
) -> io::Result<()> {
    let mut ctrl = [0u8; 5];
    ctrl[0] = TAG_HEADER_RAW;
    ctrl[1..5].copy_from_slice(&(header.len() as u32).to_le_bytes());
    mux.write_to_stream(STREAM_CTRL, &ctrl)?;
    mux.write_to_stream(STREAM_HDR, header)?;
    Ok(())
}

fn emit_sequence_fastq<const CASE_MODE: u8>(
    mux: &mut MuxWriter<impl StreamWrite>,
    nuc: &[u8],
    extra: &[u8],
    case_enc: &mut CaseEncoder,
    base_count: u64,
    extra_count: u32,
    quality: &[u8],
) -> io::Result<()> {
    let case_len = if CASE_MODE == 0 {
        let rle = case_enc.finish();
        for &byte in rle {
            mux.track_case_rle(byte, byte as u64);
        }
        rle.len() as u32
    } else {
        0u32
    };

    // TAG_SEQ
    let mut ctrl = Vec::with_capacity(25);
    ctrl.push(TAG_SEQ);
    ctrl.extend_from_slice(&(nuc.len() as u64).to_le_bytes());
    ctrl.extend_from_slice(&base_count.to_le_bytes());
    ctrl.extend_from_slice(&extra_count.to_le_bytes());
    ctrl.extend_from_slice(&case_len.to_le_bytes());
    mux.write_to_stream(STREAM_CTRL, &ctrl)?;
    mux.write_to_stream(STREAM_NUC, nuc)?;
    if CASE_MODE == 0 {
        let rle = case_enc.rle_data();
        if !rle.is_empty() {
            mux.write_to_stream(STREAM_CASE, rle)?;
        }
    }
    if !extra.is_empty() {
        mux.write_to_stream(STREAM_EXTRA, extra)?;
    }

    // TAG_QUALITY
    let mut qctrl = Vec::with_capacity(9);
    qctrl.push(TAG_QUALITY);
    qctrl.extend_from_slice(&(quality.len() as u64).to_le_bytes());
    mux.write_to_stream(STREAM_CTRL, &qctrl)?;
    mux.write_to_stream(STREAM_QUALITY, quality)?;

    Ok(())
}

/// Decode a Hecate FASTQ from a streaming mux reader and emit text.
pub fn decode_fastq(
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    mux: &mut crate::mux::reader::StreamingMuxReader<impl crate::stream::traits::StreamRead>,
    output: &mut dyn StreamWrite,
) -> io::Result<()> {
    // Reusable buffer to avoid per-record mmap/munmap page faults
    let mut text_buf = Vec::new();
    let mut have_seq = false;

    loop {
        let tag = mux.read_stream_u8(STREAM_CTRL)?;
        match tag {
            TAG_HEADER => {
                let header_len = mux.read_stream_u32le(STREAM_CTRL)?;
                output.write_all(b"@")?;
                let mut hdr = vec![0u8; header_len as usize];
                mux.read_stream_exact(STREAM_HDR, &mut hdr)?;
                output.write_all(&hdr)?;
                output.write_all(b"\n")?;
            }
            TAG_SEQ => {
                let packed_bytes = mux.read_stream_u64le(STREAM_CTRL)?;
                let base_count = mux.read_stream_u64le(STREAM_CTRL)?;
                let extra_count = mux.read_stream_u32le(STREAM_CTRL)?;
                let case_bytes = mux.read_stream_u32le(STREAM_CTRL)?;

                if packed_bytes > MAX_SEQ_ALLOC || base_count > MAX_SEQ_ALLOC {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "sequence size exceeds limit: packed_bytes={}, base_count={}",
                            packed_bytes, base_count
                        ),
                    ));
                }

                let mut nuc_data = vec![0u8; packed_bytes as usize];
                mux.read_stream_exact(STREAM_NUC, &mut nuc_data)?;

                let case_data = if case_mode == CaseMode::Include {
                    let mut cd = vec![0u8; case_bytes as usize];
                    mux.read_stream_exact(STREAM_CASE, &mut cd)?;
                    cd
                } else {
                    Vec::new()
                };

                // Read extras in new delta+upper format
                let mut extras = Vec::with_capacity(extra_count as usize);
                let mut abs_pos = 0u64;
                for _ in 0..extra_count {
                    let delta = mux.read_stream_u32le(STREAM_EXTRA)? as u64;
                    let upper = mux.read_stream_u8(STREAM_EXTRA)?;
                    abs_pos += delta;
                    let ch = crate::codec::fasta::reconstruct_extra_byte_from_code(
                        alphabet,
                        crate::codec::common::extract_nuc_code_from_packed(
                            alphabet, &nuc_data, abs_pos,
                        ),
                        upper,
                    );
                    extras.push((abs_pos, ch));
                }

                decode_bases(
                    alphabet,
                    case_mode,
                    &nuc_data,
                    &case_data,
                    &extras,
                    base_count,
                    &mut text_buf,
                );
                have_seq = true;
            }
            TAG_QUALITY => {
                let quality_len = mux.read_stream_u64le(STREAM_CTRL)?;
                let mut quality = vec![0u8; quality_len as usize];
                mux.read_stream_exact(STREAM_QUALITY, &mut quality)?;

                if have_seq {
                    output.write_all(&text_buf)?;
                    output.write_all(b"\n+\n")?;
                    output.write_all(&quality)?;
                    output.write_all(b"\n")?;
                    have_seq = false;
                }
            }
            TAG_EOF => break,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unexpected control tag: 0x{:02X}", tag),
                ));
            }
        }
    }
    Ok(())
}

fn decode_bases(
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    nuc: &[u8],
    case_data: &[u8],
    extras: &[(u64, u8)],
    base_count: u64,
    text_buf: &mut Vec<u8>,
) {
    crate::codec::common::decode_bases_into(
        alphabet, case_mode, nuc, case_data, extras, base_count, text_buf,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mux::reader::StreamingMuxReader;
    use crate::stream::buffered::BufferedReader;
    use crate::stream::traits::StreamRead;

    struct VecOutput {
        data: Vec<u8>,
    }
    impl VecOutput {
        fn new() -> Self {
            Self { data: Vec::new() }
        }
    }
    impl StreamWrite for VecOutput {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct SliceInput<'a> {
        data: &'a [u8],
        pos: usize,
    }
    impl<'a> SliceInput<'a> {
        fn new(data: &'a [u8]) -> Self {
            Self { data, pos: 0 }
        }
    }
    impl<'a> StreamRead for SliceInput<'a> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let avail = self.data.len() - self.pos;
            let n = buf.len().min(avail);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    fn round_trip_fastq(input: &str, alphabet: AlphabetMode, case_mode: CaseMode) -> String {
        let mut out = VecOutput::new();
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..6).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 6, 1024 * 1024, comps, vec![None; 6], None, 0);
            let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
            encode_fastq(alphabet, case_mode, false, None, &mut br, &mut mux).unwrap();
            let _ = mux.finalize().unwrap();
        }

        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..6).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 6, decomps, false);
        let mut decoded = VecOutput::new();
        decode_fastq(alphabet, case_mode, &mut reader, &mut decoded).unwrap();
        String::from_utf8(decoded.data).unwrap()
    }

    #[test]
    fn test_fastq_round_trip_2bit_ignore() {
        let input = "@read1\nACGT\n+\nIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, "@read1\nACGT\n+\nIIII\n");
    }

    #[test]
    fn test_fastq_round_trip_2bit_include() {
        let input = "@read1\nAcGt\n+\nIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Include);
        assert_eq!(output, "@read1\nAcGt\n+\nIIII\n");
    }

    #[test]
    fn test_fastq_round_trip_multiple_reads() {
        let input = "@r1\nACGT\n+\n!!!!\n@r2\nTTTT\n+\nHHHH\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, "@r1\nACGT\n+\n!!!!\n@r2\nTTTT\n+\nHHHH\n");
    }

    #[test]
    fn test_fastq_round_trip_4bit() {
        let input = "@r1\nACGTN\n+\nIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::FourBit, CaseMode::Ignore);
        assert_eq!(output, "@r1\nACGTN\n+\nIIIII\n");
    }

    #[test]
    fn test_fastq_round_trip_8bit_joint() {
        let input = "@r1\nAcGt\n+\nIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::EightBit, CaseMode::Joint);
        assert_eq!(output, "@r1\nAcGt\n+\nIIII\n");
    }

    #[test]
    fn test_fastq_round_trip_2bit_all_n() {
        let input = "@r1\nACGTNACGT\n+\nIIIIIIIII\n@r2\nNNNNNNNN\n+\n!!!!!!!!\n@r3\nACGTACGTN\n+\nIIIIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Include);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_round_trip_2bit_all_n_pipeline() {
        // Same test but through the pipeline (drive_hecate_source + FastqTextSink)
        let input = "@r1\nACGTNACGT\n+\nIIIIIIIII\n@r2\nNNNNNNNN\n+\n!!!!!!!!\n@r3\nACGTACGTN\n+\nIIIIIIIII\n";
        let mut out = VecOutput::new();
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..6).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 6, 1024 * 1024, comps, vec![None; 6], None, 0);
            let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
            encode_fastq(
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                None,
                &mut br,
                &mut mux,
            )
            .unwrap();
            let _ = mux.finalize().unwrap();
        }

        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..6).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 6, decomps, false);
        let mut decoded = VecOutput::new();
        {
            use crate::pipeline::sink::FastqTextSink;
            let mut sink =
                FastqTextSink::new(&mut decoded, AlphabetMode::TwoBit, CaseMode::Include);
            crate::pipeline::source::drive_hecate_source(
                FileType::Fastq,
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                &mut reader,
                &mut sink,
            )
            .unwrap();
        }
        let output = String::from_utf8(decoded.data).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_direct_decoder_ignore() {
        let input = "@r1\nACGTSWDNRYKMBHV\n+\nIIIIIIIIIIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_direct_decoder_include() {
        let input = "@r1\nACGTSWDNRYKMBHV\n+\nIIIIIIIIIIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Include);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_multi_record_direct_ignore() {
        let input = "@r1\nACGTSACGT\n+\nIIIIIIIII\n@r2\nWWWWDDDD\n+\n!!!!!!!!\n@r3\nACGTACGTS\n+\nIIIIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_multi_record_direct_include() {
        let input = "@r1\nACGTSACGT\n+\nIIIIIIIII\n@r2\nWWWWDDDD\n+\n!!!!!!!!\n@r3\nACGTACGTS\n+\nIIIIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Include);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_pipeline_ignore() {
        let input = "@r1\nACGTSWDNRYKMBHV\n+\nIIIIIIIIIIIIIII\n";
        let mut out = VecOutput::new();
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..6).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 6, 1024 * 1024, comps, vec![None; 6], None, 0);
            let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
            encode_fastq(
                AlphabetMode::TwoBit,
                CaseMode::Ignore,
                false,
                None,
                &mut br,
                &mut mux,
            )
            .unwrap();
            let _ = mux.finalize().unwrap();
        }

        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..6).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 6, decomps, false);
        let mut decoded = VecOutput::new();
        {
            use crate::pipeline::sink::FastqTextSink;
            let mut sink = FastqTextSink::new(&mut decoded, AlphabetMode::TwoBit, CaseMode::Ignore);
            crate::pipeline::source::drive_hecate_source(
                FileType::Fastq,
                AlphabetMode::TwoBit,
                CaseMode::Ignore,
                false,
                &mut reader,
                &mut sink,
            )
            .unwrap();
        }
        let output = String::from_utf8(decoded.data).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_pipeline_include() {
        let input = "@r1\nACGTSWDNRYKMBHV\n+\nIIIIIIIIIIIIIII\n";
        let mut out = VecOutput::new();
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..6).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 6, 1024 * 1024, comps, vec![None; 6], None, 0);
            let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
            encode_fastq(
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                None,
                &mut br,
                &mut mux,
            )
            .unwrap();
            let _ = mux.finalize().unwrap();
        }

        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..6).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 6, decomps, false);
        let mut decoded = VecOutput::new();
        {
            use crate::pipeline::sink::FastqTextSink;
            let mut sink =
                FastqTextSink::new(&mut decoded, AlphabetMode::TwoBit, CaseMode::Include);
            crate::pipeline::source::drive_hecate_source(
                FileType::Fastq,
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                &mut reader,
                &mut sink,
            )
            .unwrap();
        }
        let output = String::from_utf8(decoded.data).unwrap();
        assert_eq!(output, input);
    }

    fn build_iupac_input(prefix_len: usize) -> String {
        let prefix = "ACGT".repeat((prefix_len + 3) / 4);
        let seq = format!("{}S{}", &prefix[..prefix_len], &"ACGT".repeat(4));
        let qual = "I".repeat(seq.len());
        format!("@r1\n{}\n+\n{}\n", seq, qual)
    }

    #[test]
    fn test_fastq_iupac_prefix4() {
        let input = build_iupac_input(4);
        let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    // Minimal reproducer: S followed by enough in-alphabet chars
    #[test]
    fn test_fastq_iupac_s_then_acgt() {
        let input = "@r1\nACGTSACGT\n+\nIIIIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_s_only() {
        let input = "@r1\nACGTS\n+\nIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_s_then_one() {
        let input = "@r1\nACGTSA\n+\nIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_s_then_three() {
        let input = "@r1\nACGTSACG\n+\nIIIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_s_then_four() {
        let input = "@r1\nACGTSACGT\n+\nIIIIIIIII\n";
        let output = round_trip_fastq(input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    // Bisect exact failing length: S at position 4, vary suffix length
    #[test]
    fn test_fastq_iupac_s_suffix_len() {
        for suffix_len in 1..=20 {
            let suffix = "ACGT".repeat(5);
            let seq = format!("ACGTS{}", &suffix[..suffix_len]);
            let qual = "I".repeat(seq.len());
            let input = format!("@r1\n{}\n+\n{}\n", seq, qual);
            let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Ignore);
            assert_eq!(output, input, "FAILED at suffix_len={}", suffix_len);
        }
    }

    #[test]
    fn test_fastq_iupac_prefix8() {
        let input = build_iupac_input(8);
        let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_prefix16() {
        let input = build_iupac_input(16);
        let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_prefix32() {
        let input = build_iupac_input(32);
        let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_prefix64() {
        let input = build_iupac_input(64);
        let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_prefix128() {
        let input = build_iupac_input(128);
        let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    fn build_iupac_long_input() -> String {
        let seq = "ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTSACGTACGTACGTACGTWACGTACGTACGTACGTDACGTACGTACGTACGTACGTACGTACGTACGT".to_ascii_uppercase();
        let qual = "I".repeat(seq.len());
        format!("@r1\n{}\n+\n{}\n", seq, qual)
    }

    #[test]
    fn test_fastq_iupac_long_seq_direct_include() {
        let input = build_iupac_long_input();
        let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Include);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_long_seq_pipeline_include() {
        let input = build_iupac_long_input();
        let mut out = VecOutput::new();
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..6).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 6, 1024 * 1024, comps, vec![None; 6], None, 0);
            let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
            encode_fastq(
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                None,
                &mut br,
                &mut mux,
            )
            .unwrap();
            let _ = mux.finalize().unwrap();
        }

        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..6).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 6, decomps, false);
        let mut decoded = VecOutput::new();
        {
            use crate::pipeline::sink::FastqTextSink;
            let mut sink =
                FastqTextSink::new(&mut decoded, AlphabetMode::TwoBit, CaseMode::Include);
            crate::pipeline::source::drive_hecate_source(
                FileType::Fastq,
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                &mut reader,
                &mut sink,
            )
            .unwrap();
        }
        let output = String::from_utf8(decoded.data).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_long_seq_direct_ignore() {
        let input = build_iupac_long_input();
        let output = round_trip_fastq(&input, AlphabetMode::TwoBit, CaseMode::Ignore);
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_long_seq_pipeline_ignore() {
        let input = build_iupac_long_input();
        let mut out = VecOutput::new();
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..6).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 6, 1024 * 1024, comps, vec![None; 6], None, 0);
            let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
            encode_fastq(
                AlphabetMode::TwoBit,
                CaseMode::Ignore,
                false,
                None,
                &mut br,
                &mut mux,
            )
            .unwrap();
            let _ = mux.finalize().unwrap();
        }

        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..6).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 6, decomps, false);
        let mut decoded = VecOutput::new();
        {
            use crate::pipeline::sink::FastqTextSink;
            let mut sink = FastqTextSink::new(&mut decoded, AlphabetMode::TwoBit, CaseMode::Ignore);
            crate::pipeline::source::drive_hecate_source(
                FileType::Fastq,
                AlphabetMode::TwoBit,
                CaseMode::Ignore,
                false,
                &mut reader,
                &mut sink,
            )
            .unwrap();
        }
        let output = String::from_utf8(decoded.data).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn test_fastq_iupac_multi_pipeline_include() {
        let input = "@r1\nACGTSACGT\n+\nIIIIIIIII\n@r2\nWWWWDDDD\n+\n!!!!!!!!\n@r3\nACGTACGTS\n+\nIIIIIIIII\n";
        let mut out = VecOutput::new();
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..6).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 6, 1024 * 1024, comps, vec![None; 6], None, 0);
            let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
            encode_fastq(
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                None,
                &mut br,
                &mut mux,
            )
            .unwrap();
            let _ = mux.finalize().unwrap();
        }

        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..6).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 6, decomps, false);
        let mut decoded = VecOutput::new();
        {
            use crate::pipeline::sink::FastqTextSink;
            let mut sink =
                FastqTextSink::new(&mut decoded, AlphabetMode::TwoBit, CaseMode::Include);
            crate::pipeline::source::drive_hecate_source(
                FileType::Fastq,
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                &mut reader,
                &mut sink,
            )
            .unwrap();
        }
        let output = String::from_utf8(decoded.data).unwrap();
        assert_eq!(output, input);
    }

    /// BorrowRead that returns at most `max_chunk` bytes per fill_buf, forcing
    /// buffer boundaries in the middle of quality lines.
    struct SmallChunkInput<'a> {
        data: &'a [u8],
        pos: usize,
        max_chunk: usize,
    }
    impl<'a> SmallChunkInput<'a> {
        fn new(data: &'a [u8], max_chunk: usize) -> Self {
            Self {
                data,
                pos: 0,
                max_chunk,
            }
        }
    }
    impl<'a> BorrowRead for SmallChunkInput<'a> {
        fn fill_buf(&mut self, _min: usize) -> io::Result<&[u8]> {
            let remaining = &self.data[self.pos..];
            let n = remaining.len().min(self.max_chunk);
            Ok(&remaining[..n])
        }
        fn consume(&mut self, n: usize) {
            self.pos += n;
        }
    }

    fn round_trip_small_chunks(input: &str, chunk_size: usize) -> String {
        let mut out = VecOutput::new();
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..6).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 6, 1024 * 1024, comps, vec![None; 6], None, 0);
            let mut br = SmallChunkInput::new(input.as_bytes(), chunk_size);
            encode_fastq(
                AlphabetMode::TwoBit,
                CaseMode::Include,
                false,
                None,
                &mut br,
                &mut mux,
            )
            .unwrap();
            let _ = mux.finalize().unwrap();
        }

        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..6).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 6, decomps, false);
        let mut decoded = VecOutput::new();
        decode_fastq(
            AlphabetMode::TwoBit,
            CaseMode::Include,
            &mut reader,
            &mut decoded,
        )
        .unwrap();
        String::from_utf8(decoded.data).unwrap()
    }

    #[test]
    fn test_fastq_small_chunk_all_n_pair() {
        // Two consecutive all-N reads. With small chunk sizes, the quality
        // line may complete without a trailing newline in the current chunk,
        // triggering the no-newline quality path, which must still reset
        // last_extra_pos.
        let input = "@r1\nNNNNNNNN\n+\n!!!!!!!!\n@r2\nNNNNNNNN\n+\n!!!!!!!!\n";
        for chunk_size in 1..=input.len() {
            let output = round_trip_small_chunks(input, chunk_size);
            assert_eq!(output, input, "failed with chunk_size={}", chunk_size);
        }
    }

    #[test]
    fn test_fastq_small_chunk_normal_then_all_n() {
        // Normal read followed by all-N read: the first record's
        // last_extra_pos must not leak into the second record's encoding.
        let input = "@r1\nACGTNACGT\n+\nIIIIIIIII\n@r2\nNNNNNNNN\n+\n!!!!!!!!\n";
        for chunk_size in 1..=input.len() {
            let output = round_trip_small_chunks(input, chunk_size);
            assert_eq!(output, input, "failed with chunk_size={}", chunk_size);
        }
    }

    #[test]
    fn test_fastq_small_chunk_three_records_with_extras() {
        let input = "@r1\nACGTNACGT\n+\nIIIIIIIII\n\
                      @r2\nNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNN\n+\nIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII\n\
                      @r3\nNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNN\n+\nIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII\n";
        for chunk_size in [1, 5, 10, 17, 29, 37, 50, input.len()] {
            let output = round_trip_small_chunks(input, chunk_size);
            assert_eq!(output, input, "failed with chunk_size={}", chunk_size);
        }
    }
}
