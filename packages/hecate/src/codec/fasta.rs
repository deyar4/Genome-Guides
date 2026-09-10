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
use crate::container::*;
use crate::mux::writer::MuxWriter;
use crate::stream::traits::{BorrowRead, StreamWrite};
use memchr::{memchr, memchr3};
use std::io;

/// Maximum single-sequence allocation (16 GiB) to prevent OOM from crafted files.
const MAX_SEQ_ALLOC: u64 = 16 << 30;

// --- SIMD Encoding (x86_64 SSSE3) ---

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn encode_16_bases_2bit(src: &[u8]) -> Option<[u8; 4]> {
    use std::arch::x86_64::*;
    // LUT indexed by low nibble of ASCII byte -> 2-bit code (0xFF = invalid).
    // CGAT ordering: C=0x43(nib 3)->0, G=0x47(nib 7)->1, A=0x41(nib 1)->2, T=0x54(nib 4)->3
    let lut = _mm_setr_epi8(
        -1, 2, -1, 0, 3, -1, -1, 1, // nibbles 0-7
        -1, -1, -1, -1, -1, -1, -1, -1, // nibbles 8-15
    );
    let input = _mm_loadu_si128(src.as_ptr() as *const __m128i);
    let nibbles = _mm_and_si128(input, _mm_set1_epi8(0x0F));
    let codes = _mm_shuffle_epi8(lut, nibbles);

    // Check for invalid codes (any 0xFF -> sign bit set)
    if _mm_movemask_epi8(codes) != 0 {
        return None;
    }

    // Verify codes by reconstructing expected uppercase bytes and comparing.
    // The low-nibble LUT alone cannot distinguish e.g. C(0x43) from S(0x53)
    // since they share nibble 3.  Reverse-map codes -> expected uppercase
    // ASCII and reject if any byte doesn't match.
    let decode_lut = _mm_setr_epi8(0x43, 0x47, 0x41, 0x54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    let expected = _mm_shuffle_epi8(decode_lut, codes);
    let uppercased = _mm_and_si128(input, _mm_set1_epi8(-33)); // clear bit 5
    let eq = _mm_cmpeq_epi8(expected, uppercased);
    if _mm_movemask_epi8(eq) != 0xFFFF {
        return None;
    }

    // Pack 16 2-bit codes into 4 bytes using multiply-add chain.
    // Each group of 4 codes [c0, c1, c2, c3] -> c0 + 4*c1 + 16*c2 + 64*c3
    let weights = _mm_setr_epi8(1, 4, 16, 64, 1, 4, 16, 64, 1, 4, 16, 64, 1, 4, 16, 64);
    let pairs = _mm_maddubs_epi16(codes, weights);
    let quads = _mm_hadd_epi16(pairs, pairs);
    let packed_u8 = _mm_packus_epi16(quads, _mm_setzero_si128());
    let packed = _mm_extract_epi32(packed_u8, 0) as u32;
    Some(packed.to_le_bytes())
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn encode_16_bases_2bit_case(src: &[u8]) -> Option<([u8; 4], u16)> {
    use std::arch::x86_64::*;
    let lut = _mm_setr_epi8(-1, 2, -1, 0, 3, -1, -1, 1, -1, -1, -1, -1, -1, -1, -1, -1);
    let input = _mm_loadu_si128(src.as_ptr() as *const __m128i);
    let nibbles = _mm_and_si128(input, _mm_set1_epi8(0x0F));
    let codes = _mm_shuffle_epi8(lut, nibbles);

    if _mm_movemask_epi8(codes) != 0 {
        return None;
    }

    // Verify codes by reconstructing expected uppercase bytes and comparing.
    let decode_lut = _mm_setr_epi8(0x43, 0x47, 0x41, 0x54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    let expected = _mm_shuffle_epi8(decode_lut, codes);
    let uppercased = _mm_and_si128(input, _mm_set1_epi8(-33));
    let eq = _mm_cmpeq_epi8(expected, uppercased);
    if _mm_movemask_epi8(eq) != 0xFFFF {
        return None;
    }

    // Extract case: uppercase has bit 5 clear, lowercase has it set.
    // Compare with 0x60: byte > 0x60 means lowercase.
    let threshold = _mm_set1_epi8(0x60);
    let is_lower = _mm_cmpgt_epi8(input, threshold);
    let case_bits = !_mm_movemask_epi8(is_lower) as u16;

    let weights = _mm_setr_epi8(1, 4, 16, 64, 1, 4, 16, 64, 1, 4, 16, 64, 1, 4, 16, 64);
    let pairs = _mm_maddubs_epi16(codes, weights);
    let quads = _mm_hadd_epi16(pairs, pairs);
    let packed_u8 = _mm_packus_epi16(quads, _mm_setzero_si128());
    let packed = _mm_extract_epi32(packed_u8, 0) as u32;
    Some((packed.to_le_bytes(), case_bits))
}

/// Encode FASTA text into a Hecate mux writer.
pub fn encode_fasta(
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    input: &mut dyn BorrowRead,
    mux: &mut MuxWriter<impl StreamWrite>,
) -> io::Result<()> {
    match (alphabet, case_mode) {
        (AlphabetMode::TwoBit, CaseMode::Include) => encode_fasta_inner::<2, 0>(input, mux),
        (AlphabetMode::TwoBit, CaseMode::Ignore) => encode_fasta_inner::<2, 1>(input, mux),
        (AlphabetMode::FourBit, CaseMode::Include) => encode_fasta_inner::<4, 0>(input, mux),
        (AlphabetMode::FourBit, CaseMode::Ignore) => encode_fasta_inner::<4, 1>(input, mux),
        (AlphabetMode::EightBit, CaseMode::Include) => encode_fasta_inner::<8, 0>(input, mux),
        (AlphabetMode::EightBit, CaseMode::Ignore) => encode_fasta_inner::<8, 1>(input, mux),
        (AlphabetMode::EightBit, CaseMode::Joint) => encode_fasta_inner::<8, 2>(input, mux),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid alphabet/case combination",
        )),
    }
}

#[derive(PartialEq)]
enum FastaState {
    BeforeRecord,
    InHeader,
    InSequence,
}

fn encode_fasta_inner<const ALPHABET: u8, const CASE_MODE: u8>(
    input: &mut dyn BorrowRead,
    mux: &mut MuxWriter<impl StreamWrite>,
) -> io::Result<()> {
    let mut state = FastaState::BeforeRecord;
    let mut header_buf = Vec::new();
    let mut nuc_buf = Vec::new();
    let mut extra_buf = Vec::new();
    let mut case_enc = CaseEncoder::new();
    let mut bit_pos = 0u8;
    let mut base_count = 0u64;
    let mut extra_count = 0u32;
    let mut last_extra_pos = 0u64;

    loop {
        let chunk = input.fill_buf(1)?;
        if chunk.is_empty() {
            break;
        }
        let chunk_len = chunk.len();

        let mut i = 0;
        while i < chunk_len {
            match state {
                FastaState::BeforeRecord => {
                    // Scan for '>' using memchr
                    match memchr(b'>', &chunk[i..]) {
                        Some(pos) => {
                            // Verify everything before '>' is whitespace
                            for &b in &chunk[i..i + pos] {
                                if b != b'\n' && b != b'\r' && b != b' ' && b != b'\t' {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "expected '>' at record start",
                                    ));
                                }
                            }
                            i += pos + 1;
                            state = FastaState::InHeader;
                        }
                        None => {
                            for &b in &chunk[i..] {
                                if b != b'\n' && b != b'\r' && b != b' ' && b != b'\t' {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "expected '>' at record start",
                                    ));
                                }
                            }
                            i = chunk_len;
                        }
                    }
                }
                FastaState::InHeader => {
                    // Find end of header line
                    match memchr(b'\n', &chunk[i..]) {
                        Some(pos) => {
                            let line = &chunk[i..i + pos];
                            // Strip \r from header content
                            if line.last() == Some(&b'\r') {
                                header_buf.extend_from_slice(&line[..line.len() - 1]);
                            } else {
                                header_buf.extend_from_slice(line);
                            }
                            mux.begin_sequence(&header_buf, bit_pos);
                            emit_header(mux, &header_buf)?;
                            header_buf.clear();
                            i += pos + 1;
                            state = FastaState::InSequence;
                        }
                        None => {
                            // No newline: buffer remaining (strip \r)
                            for &b in &chunk[i..] {
                                if b != b'\r' {
                                    header_buf.push(b);
                                }
                            }
                            i = chunk_len;
                        }
                    }
                }
                FastaState::InSequence => {
                    // Find next delimiter: newline, carriage return, or '>'
                    let remaining = &chunk[i..];
                    let end = memchr3(b'\n', b'\r', b'>', remaining).unwrap_or(remaining.len());

                    // Batch-encode nucleotides up to the delimiter
                    let seq = &remaining[..end];
                    encode_sequence_batch::<ALPHABET, CASE_MODE>(
                        seq,
                        &mut nuc_buf,
                        &mut case_enc,
                        &mut extra_buf,
                        &mut bit_pos,
                        &mut base_count,
                        &mut extra_count,
                        &mut last_extra_pos,
                    );
                    i += end;

                    // Handle delimiter
                    if i < chunk_len {
                        let delim = chunk[i];
                        i += 1;
                        if delim == b'>' {
                            if base_count == 0 {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "empty sequence",
                                ));
                            }
                            emit_sequence::<CASE_MODE>(
                                mux,
                                &nuc_buf,
                                &extra_buf,
                                &mut case_enc,
                                base_count,
                                extra_count,
                            )?;
                            mux.end_sequence(base_count);
                            nuc_buf.clear();
                            extra_buf.clear();
                            case_enc.reset();
                            bit_pos = 0;
                            base_count = 0;
                            extra_count = 0;
                            last_extra_pos = 0;
                            state = FastaState::InHeader;
                        }
                        // \n and \r are simply skipped
                    }
                }
            }
        }

        input.consume(chunk_len);
    }

    // Finalize last record
    if state == FastaState::InSequence {
        if base_count == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "empty sequence"));
        }
        emit_sequence::<CASE_MODE>(
            mux,
            &nuc_buf,
            &extra_buf,
            &mut case_enc,
            base_count,
            extra_count,
        )?;
        mux.end_sequence(base_count);
    }

    // Write EOF tag
    mux.write_to_stream(STREAM_CTRL, &[TAG_EOF])?;

    Ok(())
}

/// Public wrapper for FASTQ codec to reuse.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn encode_base_pub<const ALPHABET: u8, const CASE_MODE: u8>(
    byte: u8,
    nuc_buf: &mut Vec<u8>,
    case_enc: &mut CaseEncoder,
    extra_buf: &mut Vec<u8>,
    bit_pos: &mut u8,
    base_count: &mut u64,
    extra_count: &mut u32,
    last_extra_pos: &mut u64,
) {
    encode_base::<ALPHABET, CASE_MODE>(
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

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn encode_base<const ALPHABET: u8, const CASE_MODE: u8>(
    byte: u8,
    nuc_buf: &mut Vec<u8>,
    case_enc: &mut CaseEncoder,
    extra_buf: &mut Vec<u8>,
    bit_pos: &mut u8,
    base_count: &mut u64,
    extra_count: &mut u32,
    last_extra_pos: &mut u64,
) {
    if ALPHABET == 8 {
        if CASE_MODE == 2 {
            nuc_buf.push(byte);
        } else if CASE_MODE == 1 {
            nuc_buf.push(byte.to_ascii_uppercase());
        } else {
            nuc_buf.push(byte.to_ascii_uppercase());
            case_enc.push_bit(CASE_BIT[byte as usize]);
        }
    } else {
        let code = if ALPHABET == 2 {
            TWO_BIT_ENCODE[byte as usize]
        } else {
            FOUR_BIT_ENCODE[byte as usize]
        };

        if code == 0xFF {
            // Out-of-alphabet: store lower bits in nuc, upper bits + delta in extra
            let (nuc_code, upper) = if ALPHABET == 2 {
                (byte & 0x03, byte >> 2)
            } else {
                (byte & 0x0F, byte >> 4)
            };
            let delta = (*base_count - *last_extra_pos) as u32;
            extra_buf.extend_from_slice(&delta.to_le_bytes());
            extra_buf.push(upper);
            *extra_count += 1;
            *last_extra_pos = *base_count;
            pack_code(ALPHABET, nuc_buf, bit_pos, nuc_code);
        } else {
            pack_code(ALPHABET, nuc_buf, bit_pos, code);
        }

        if CASE_MODE == 0 {
            case_enc.push_bit(CASE_BIT[byte as usize]);
        }
    }

    *base_count += 1;
}

/// Batch-encode a sequence region, using fast paths when byte-aligned.
///
/// Key optimizations vs. the naive per-base approach:
/// - 2-bit SIMD: 16 bases/iter with SSSE3, case bits deposited via push_bits
/// - 2-bit scalar: 8 bases/iter (2 packed bytes), then 4 bases/iter tail
/// - 4-bit scalar: 8 bases/iter (4 packed bytes), then 2 bases/iter tail
/// - 8-bit: bulk copy with optional uppercasing, case bits in chunks of 8
/// - No per-pair whitespace checks in batch inner loops (whitespace maps to
///   0xFF in encoding tables and is caught by the validity check, or is
///   handled by the top-of-loop skip for 8-bit)
/// - Case bits collected into bitmap via push_bits (no serial RLE dependency)
#[allow(clippy::too_many_arguments)]
#[inline]
fn encode_sequence_batch<const ALPHABET: u8, const CASE_MODE: u8>(
    seq: &[u8],
    nuc_buf: &mut Vec<u8>,
    case_enc: &mut CaseEncoder,
    extra_buf: &mut Vec<u8>,
    bit_pos: &mut u8,
    base_count: &mut u64,
    extra_count: &mut u32,
    last_extra_pos: &mut u64,
) {
    let mut j = 0;
    let len = seq.len();

    while j < len {
        // Skip whitespace (rare in real FASTA/FASTQ data)
        if seq[j] == b' ' || seq[j] == b'\t' {
            j += 1;
            continue;
        }

        // --- 8-bit: bulk copy with optional uppercasing ---
        if ALPHABET == 8 {
            // Find the end of the non-whitespace run
            let remaining = &seq[j..];
            let run_len = remaining
                .iter()
                .position(|&b| b == b' ' || b == b'\t')
                .unwrap_or(remaining.len());

            if run_len > 0 {
                let batch = &remaining[..run_len];
                if CASE_MODE == 2 {
                    // Joint: raw copy
                    nuc_buf.extend_from_slice(batch);
                } else {
                    // Uppercase the batch
                    let start = nuc_buf.len();
                    nuc_buf.extend_from_slice(batch);
                    for b in nuc_buf[start..].iter_mut() {
                        *b = b.to_ascii_uppercase();
                    }
                    if CASE_MODE == 0 {
                        // Collect case bits in chunks of 8
                        let mut k = 0;
                        while k + 8 <= run_len {
                            case_enc.push_bits(8, case_mask_8(&batch[k..]));
                            k += 8;
                        }
                        while k + 4 <= run_len {
                            case_enc.push_bits(4, case_mask_4(&batch[k..]));
                            k += 4;
                        }
                        for i in k..run_len {
                            case_enc.push_bit(CASE_BIT[batch[i] as usize]);
                        }
                    }
                }
                *base_count += run_len as u64;
                j += run_len;
                continue;
            }
        }

        // --- 2-bit: SIMD and scalar batch packing ---
        if ALPHABET == 2 && *bit_pos == 0 {
            // SIMD path: 16 bases at a time
            #[cfg(target_arch = "x86_64")]
            if is_x86_feature_detected!("ssse3") {
                while j + 16 <= len {
                    if CASE_MODE == 0 {
                        if let Some((packed, case_bits)) =
                            unsafe { encode_16_bases_2bit_case(&seq[j..j + 16]) }
                        {
                            nuc_buf.extend_from_slice(&packed);
                            case_enc.push_bits(16, case_bits as u64);
                            *base_count += 16;
                            j += 16;
                        } else {
                            break;
                        }
                    } else if let Some(packed) = unsafe { encode_16_bases_2bit(&seq[j..j + 16]) } {
                        nuc_buf.extend_from_slice(&packed);
                        *base_count += 16;
                        j += 16;
                    } else {
                        break;
                    }
                }
            }

            // Scalar batch: 8 bases -> 2 bytes
            while j + 8 <= len {
                if let Some(packed) = try_pack_8_bases_2bit(&seq[j..j + 8]) {
                    nuc_buf.extend_from_slice(&packed);
                    if CASE_MODE == 0 {
                        case_enc.push_bits(8, case_mask_8(&seq[j..]));
                    }
                    *base_count += 8;
                    j += 8;
                } else {
                    break;
                }
            }

            // Scalar batch: 4 bases -> 1 byte (tail)
            while j + 4 <= len {
                if let Some(packed) = try_pack_4_bases_2bit(&seq[j..j + 4]) {
                    nuc_buf.push(packed);
                    if CASE_MODE == 0 {
                        case_enc.push_bits(4, case_mask_4(&seq[j..]));
                    }
                    *base_count += 4;
                    j += 4;
                } else {
                    break;
                }
            }
        } else if ALPHABET == 4 && *bit_pos == 0 {
            // 4-bit batch: 8 bases -> 4 bytes
            while j + 8 <= len {
                if let Some(packed) = try_pack_8_bases_4bit(&seq[j..j + 8]) {
                    nuc_buf.extend_from_slice(&packed);
                    if CASE_MODE == 0 {
                        case_enc.push_bits(8, case_mask_8(&seq[j..]));
                    }
                    *base_count += 8;
                    j += 8;
                } else {
                    break;
                }
            }

            // 4-bit batch: 2 bases -> 1 byte (tail)
            while j + 2 <= len {
                if let Some(packed) = try_pack_2_bases_4bit(&seq[j..j + 2]) {
                    nuc_buf.push(packed);
                    if CASE_MODE == 0 {
                        case_enc.push_bits(2, case_mask_2(&seq[j..]));
                    }
                    *base_count += 2;
                    j += 2;
                } else {
                    break;
                }
            }
        }

        // Scalar fallback for remaining/unaligned/OOA bases
        if j < len && seq[j] != b' ' && seq[j] != b'\t' {
            encode_base::<ALPHABET, CASE_MODE>(
                seq[j],
                nuc_buf,
                case_enc,
                extra_buf,
                bit_pos,
                base_count,
                extra_count,
                last_extra_pos,
            );
            j += 1;
        }
    }
}

/// Public wrapper for FASTQ batch encoding.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn encode_sequence_batch_pub<const ALPHABET: u8, const CASE_MODE: u8>(
    seq: &[u8],
    nuc_buf: &mut Vec<u8>,
    case_enc: &mut CaseEncoder,
    extra_buf: &mut Vec<u8>,
    bit_pos: &mut u8,
    base_count: &mut u64,
    extra_count: &mut u32,
    last_extra_pos: &mut u64,
) {
    encode_sequence_batch::<ALPHABET, CASE_MODE>(
        seq,
        nuc_buf,
        case_enc,
        extra_buf,
        bit_pos,
        base_count,
        extra_count,
        last_extra_pos,
    );
}

fn emit_header(mux: &mut MuxWriter<impl StreamWrite>, header: &[u8]) -> io::Result<()> {
    let mut ctrl = [0u8; 5];
    ctrl[0] = TAG_HEADER;
    ctrl[1..5].copy_from_slice(&(header.len() as u32).to_le_bytes());
    mux.write_to_stream(STREAM_CTRL, &ctrl)?;
    mux.write_to_stream(STREAM_HDR, header)?;
    Ok(())
}

fn emit_sequence<const CASE_MODE: u8>(
    mux: &mut MuxWriter<impl StreamWrite>,
    nuc: &[u8],
    extra: &[u8],
    case_enc: &mut CaseEncoder,
    base_count: u64,
    extra_count: u32,
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

    let mut ctrl = Vec::with_capacity(25);
    ctrl.push(TAG_SEQ);
    ctrl.extend_from_slice(&(nuc.len() as u64).to_le_bytes()); // packed_bytes
    ctrl.extend_from_slice(&base_count.to_le_bytes()); // base_count
    ctrl.extend_from_slice(&extra_count.to_le_bytes()); // extra_count
    ctrl.extend_from_slice(&case_len.to_le_bytes()); // case_bytes
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
    Ok(())
}

/// Reconstruct a full byte from upper bits (stored in extra stream) and
/// lower bits (stored in the nuc stream at the given position).
#[inline]
fn reconstruct_extra_byte(alphabet: AlphabetMode, nuc: &[u8], pos: u64, upper: u8) -> u8 {
    match alphabet {
        AlphabetMode::TwoBit => {
            let byte_idx = pos as usize / 4;
            let bit_offset = (pos as usize % 4) * 2;
            let nuc_code = (nuc[byte_idx] >> bit_offset) & 0x03;
            (upper << 2) | nuc_code
        }
        AlphabetMode::FourBit => {
            let byte_idx = pos as usize / 2;
            let bit_offset = (pos as usize % 2) * 4;
            let nuc_code = (nuc[byte_idx] >> bit_offset) & 0x0F;
            (upper << 4) | nuc_code
        }
        AlphabetMode::EightBit => upper, // no extras for 8-bit
    }
}

/// Reconstruct a full byte from upper bits and nuc code (already extracted).
/// Public for use in pipeline source.
#[inline]
pub fn reconstruct_extra_byte_from_code(alphabet: AlphabetMode, nuc_code: u8, upper: u8) -> u8 {
    match alphabet {
        AlphabetMode::TwoBit => (upper << 2) | (nuc_code & 0x03),
        AlphabetMode::FourBit => (upper << 4) | (nuc_code & 0x0F),
        AlphabetMode::EightBit => upper,
    }
}

/// Decode a Hecate FASTA from a streaming mux reader and emit text.
pub fn decode_fasta(
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    mux: &mut crate::mux::reader::StreamingMuxReader<impl crate::stream::traits::StreamRead>,
    output: &mut dyn StreamWrite,
    line_width: usize,
) -> io::Result<()> {
    // Reusable buffers to avoid per-record mmap/munmap page faults
    let mut text_buf = Vec::new();
    let mut write_buf = Vec::new();

    loop {
        let tag = mux.read_stream_u8(STREAM_CTRL)?;
        match tag {
            TAG_HEADER => {
                let header_len = mux.read_stream_u32le(STREAM_CTRL)?;
                output.write_all(b">")?;
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

                // Read extras in new delta+upper format, reconstruct full bytes
                let mut extras = Vec::with_capacity(extra_count as usize);
                let mut abs_pos = 0u64;
                for _ in 0..extra_count {
                    let delta = mux.read_stream_u32le(STREAM_EXTRA)? as u64;
                    let upper = mux.read_stream_u8(STREAM_EXTRA)?;
                    abs_pos += delta;
                    // Reconstruct full byte from upper bits + nuc lower bits
                    let ch = reconstruct_extra_byte(alphabet, &nuc_data, abs_pos, upper);
                    extras.push((abs_pos, ch));
                }

                decode_sequence(
                    alphabet,
                    case_mode,
                    &nuc_data,
                    &case_data,
                    &extras,
                    base_count,
                    output,
                    line_width,
                    &mut text_buf,
                    &mut write_buf,
                )?;
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

#[allow(clippy::too_many_arguments)]
fn decode_sequence(
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    nuc: &[u8],
    case_data: &[u8],
    extras: &[(u64, u8)],
    base_count: u64,
    output: &mut dyn StreamWrite,
    line_width: usize,
    text_buf: &mut Vec<u8>,
    write_buf: &mut Vec<u8>,
) -> io::Result<()> {
    let bc = base_count as usize;

    // Phases 1-3: unpack, extras, case
    crate::codec::common::decode_bases_into(
        alphabet, case_mode, nuc, case_data, extras, base_count, text_buf,
    );

    // Phase 4: Line wrapping + output.
    if line_width == 0 {
        output.write_all(text_buf)?;
        output.write_all(b"\n")?;
    } else {
        let num_newlines = bc.div_ceil(line_width);
        write_buf.clear();
        write_buf.reserve(bc + num_newlines);
        for chunk in text_buf.chunks(line_width) {
            write_buf.extend_from_slice(chunk);
            write_buf.push(b'\n');
        }
        output.write_all(write_buf)?;
    }

    Ok(())
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

    fn round_trip_fasta(
        input: &str,
        alphabet: AlphabetMode,
        case_mode: CaseMode,
        line_width: usize,
    ) -> String {
        let mut out = VecOutput::new();

        // Encode
        {
            let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
                (0..5).map(|_| None).collect();
            let mut mux = MuxWriter::new(&mut out, 5, 1024 * 1024, comps, vec![None; 5], None, 0);
            let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
            encode_fasta(alphabet, case_mode, &mut br, &mut mux).unwrap();
            let _ = mux.finalize().unwrap();
        }

        // Decode
        let decomps: Vec<Option<Box<dyn crate::compress::Decompressor>>> =
            (0..5).map(|_| None).collect();
        let mut reader = StreamingMuxReader::new(SliceInput::new(&out.data), 5, decomps, false);
        let mut decoded = VecOutput::new();
        decode_fasta(alphabet, case_mode, &mut reader, &mut decoded, line_width).unwrap();

        String::from_utf8(decoded.data).unwrap()
    }

    #[test]
    fn test_round_trip_2bit_include() {
        let input = ">seq1\nACGTACGT\n>seq2\nTTTTAAAA\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Include, 80);
        assert_eq!(output, ">seq1\nACGTACGT\n>seq2\nTTTTAAAA\n");
    }

    #[test]
    fn test_round_trip_2bit_ignore() {
        let input = ">seq1\nacgtACGT\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Ignore, 80);
        assert_eq!(output, ">seq1\nACGTACGT\n");
    }

    #[test]
    fn test_round_trip_2bit_mixed_case() {
        let input = ">s\nAAccGGtt\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Include, 80);
        assert_eq!(output, ">s\nAAccGGtt\n");
    }

    #[test]
    fn test_round_trip_4bit_include() {
        let input = ">seq\nACGTNRSWK\n";
        let output = round_trip_fasta(input, AlphabetMode::FourBit, CaseMode::Include, 80);
        assert_eq!(output, ">seq\nACGTNRSWK\n");
    }

    #[test]
    fn test_round_trip_4bit_ignore() {
        let input = ">seq\nacgtnrswk\n";
        let output = round_trip_fasta(input, AlphabetMode::FourBit, CaseMode::Ignore, 80);
        assert_eq!(output, ">seq\nACGTNRSWK\n");
    }

    #[test]
    fn test_round_trip_8bit_include() {
        let input = ">seq\nAcGtNn\n";
        let output = round_trip_fasta(input, AlphabetMode::EightBit, CaseMode::Include, 80);
        assert_eq!(output, ">seq\nAcGtNn\n");
    }

    #[test]
    fn test_round_trip_8bit_ignore() {
        let input = ">seq\nAcGtNn\n";
        let output = round_trip_fasta(input, AlphabetMode::EightBit, CaseMode::Ignore, 80);
        assert_eq!(output, ">seq\nACGTNN\n");
    }

    #[test]
    fn test_round_trip_8bit_joint() {
        let input = ">seq\nAcGtNn\n";
        let output = round_trip_fasta(input, AlphabetMode::EightBit, CaseMode::Joint, 80);
        assert_eq!(output, ">seq\nAcGtNn\n");
    }

    #[test]
    fn test_multiline_sequence() {
        let input = ">s\nACGT\nACGT\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Ignore, 80);
        assert_eq!(output, ">s\nACGTACGT\n");
    }

    #[test]
    fn test_line_wrapping() {
        let input = ">s\nACGTACGTACGT\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Ignore, 4);
        assert_eq!(output, ">s\nACGT\nACGT\nACGT\n");
    }

    #[test]
    fn test_line_wrapping_zero() {
        let input = ">s\nACGT\nACGT\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Ignore, 0);
        assert_eq!(output, ">s\nACGTACGT\n");
    }

    #[test]
    fn test_2bit_with_extras() {
        let input = ">s\nACNGT\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Include, 80);
        assert_eq!(output, ">s\nACNGT\n");
    }

    #[test]
    fn test_multiple_sequences() {
        let input = ">s1\nACGT\n>s2\nTTTT\n>s3\nAAAA\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Include, 80);
        assert_eq!(output, ">s1\nACGT\n>s2\nTTTT\n>s3\nAAAA\n");
    }

    #[test]
    fn test_empty_sequence_error() {
        let input = ">s1\n>s2\nACGT\n";
        let mut out = VecOutput::new();
        let comps: Vec<Option<Box<dyn crate::compress::Compressor>>> =
            (0..5).map(|_| None).collect();
        let mut mux = MuxWriter::new(&mut out, 5, 1024 * 1024, comps, vec![None; 5], None, 0);
        let mut br = BufferedReader::new(SliceInput::new(input.as_bytes()));
        let result = encode_fasta(AlphabetMode::TwoBit, CaseMode::Include, &mut br, &mut mux);
        assert!(result.is_err());
    }

    #[test]
    fn test_single_base() {
        let input = ">s\nA\n";
        let output = round_trip_fasta(input, AlphabetMode::TwoBit, CaseMode::Include, 80);
        assert_eq!(output, ">s\nA\n");
    }

    // --- Direct SIMD function tests ---

    #[cfg(target_arch = "x86_64")]
    fn scalar_encode_16_2bit(src: &[u8; 16]) -> Option<[u8; 4]> {
        let mut packed = [0u8; 4];
        for (i, &b) in src.iter().enumerate() {
            let code = TWO_BIT_ENCODE[b as usize];
            if code == 0xFF {
                return None;
            }
            packed[i / 4] |= code << ((i % 4) * 2);
        }
        Some(packed)
    }

    #[cfg(target_arch = "x86_64")]
    fn scalar_encode_16_2bit_case(src: &[u8; 16]) -> Option<([u8; 4], u16)> {
        let mut packed = [0u8; 4];
        let mut case_bits: u16 = 0;
        for (i, &b) in src.iter().enumerate() {
            let code = TWO_BIT_ENCODE[b as usize];
            if code == 0xFF {
                return None;
            }
            packed[i / 4] |= code << ((i % 4) * 2);
            if CASE_BIT[b as usize] == 1 {
                case_bits |= 1 << i;
            }
        }
        Some((packed, case_bits))
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_cgat() {
        // "CGATCGATCGATCGAT" - all 4 nucleotides cycling, exactly 16 bases
        let src = b"CGATCGATCGATCGAT";
        let simd = unsafe { encode_16_bases_2bit(src) };
        let scalar = scalar_encode_16_2bit(src);
        assert_eq!(simd, scalar);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_all_c() {
        let src = b"CCCCCCCCCCCCCCCC";
        let simd = unsafe { encode_16_bases_2bit(src) };
        let scalar = scalar_encode_16_2bit(src);
        assert_eq!(simd, scalar);
        assert_eq!(simd, Some([0x00, 0x00, 0x00, 0x00]));
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_all_g() {
        let src = b"GGGGGGGGGGGGGGGG";
        let simd = unsafe { encode_16_bases_2bit(src) };
        let scalar = scalar_encode_16_2bit(src);
        assert_eq!(simd, scalar);
        assert_eq!(simd, Some([0x55, 0x55, 0x55, 0x55]));
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_all_a() {
        let src = b"AAAAAAAAAAAAAAAA";
        let simd = unsafe { encode_16_bases_2bit(src) };
        let scalar = scalar_encode_16_2bit(src);
        assert_eq!(simd, scalar);
        assert_eq!(simd, Some([0xAA, 0xAA, 0xAA, 0xAA]));
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_all_t() {
        let src = b"TTTTTTTTTTTTTTTT";
        let simd = unsafe { encode_16_bases_2bit(src) };
        let scalar = scalar_encode_16_2bit(src);
        assert_eq!(simd, scalar);
        assert_eq!(simd, Some([0xFF, 0xFF, 0xFF, 0xFF]));
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_lowercase() {
        let src = b"cgatcgatcgatcgat";
        let simd = unsafe { encode_16_bases_2bit(src) };
        let upper = b"CGATCGATCGATCGAT";
        let scalar = scalar_encode_16_2bit(upper);
        // Lowercase should produce the same packed codes as uppercase.
        assert_eq!(simd, scalar);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_mixed_case() {
        let src = b"CgAtCgAtCgAtCgAt";
        let simd = unsafe { encode_16_bases_2bit(src) };
        let scalar = scalar_encode_16_2bit(src);
        assert_eq!(simd, scalar);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_invalid_returns_none() {
        // N is not a valid 2-bit nucleotide
        let src = b"ACGTACGTACGTACGN";
        assert_eq!(unsafe { encode_16_bases_2bit(src) }, None);
        // Newline
        let src2 = b"ACGTACGTACGTACG\n";
        assert_eq!(unsafe { encode_16_bases_2bit(src2) }, None);
        // Space
        let src3 = b" CGTACGTACGTACGT";
        assert_eq!(unsafe { encode_16_bases_2bit(src3) }, None);
        // IUPAC chars S/W/D share low nibbles with C/G/T -- must be rejected
        assert_eq!(unsafe { encode_16_bases_2bit(b"ACGTACGTACGTACGS") }, None); // S (0x53) vs C (0x43)
        assert_eq!(unsafe { encode_16_bases_2bit(b"ACGTACGTACGTACGW") }, None); // W (0x57) vs G (0x47)
        assert_eq!(unsafe { encode_16_bases_2bit(b"ACGTACGTACGTACGD") }, None); // D (0x44) vs T (0x54)
        assert_eq!(unsafe { encode_16_bases_2bit(b"SACGTACGTACGTACG") }, None); // S at start
        assert_eq!(unsafe { encode_16_bases_2bit(b"ACGTACGWACGTACGT") }, None); // W in middle
                                                                                // Also test the _case variant
        assert_eq!(
            unsafe { encode_16_bases_2bit_case(b"ACGTACGTACGTACGS") },
            None
        );
        assert_eq!(
            unsafe { encode_16_bases_2bit_case(b"ACGTACGTACGTACGW") },
            None
        );
        assert_eq!(
            unsafe { encode_16_bases_2bit_case(b"ACGTACGTACGTACGD") },
            None
        );
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_case_basic() {
        let src = b"CGATCGATCGATCGAT";
        let simd = unsafe { encode_16_bases_2bit_case(src) };
        let scalar = scalar_encode_16_2bit_case(src);
        assert_eq!(simd, scalar);
        // All uppercase -> case_bits should be 0xFFFF
        assert_eq!(simd.unwrap().1, 0xFFFF);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_case_all_lower() {
        let src = b"cgatcgatcgatcgat";
        let simd = unsafe { encode_16_bases_2bit_case(src) };
        let scalar = scalar_encode_16_2bit_case(src);
        assert_eq!(simd, scalar);
        // All lowercase -> case_bits should be 0x0000
        assert_eq!(simd.unwrap().1, 0x0000);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_case_alternating() {
        let src = b"CgAtCgAtCgAtCgAt";
        let simd = unsafe { encode_16_bases_2bit_case(src) };
        let scalar = scalar_encode_16_2bit_case(src);
        assert_eq!(simd, scalar);
        // Uppercase at even positions, lowercase at odd -> 0x5555
        assert_eq!(simd.unwrap().1, 0x5555);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_encode_case_invalid() {
        let src = b"ACGTACGTACGTACGN";
        assert_eq!(unsafe { encode_16_bases_2bit_case(src) }, None);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_vs_scalar_exhaustive_patterns() {
        // Test many different 16-byte nucleotide patterns against scalar.
        let bases = [b'C', b'G', b'A', b'T', b'c', b'g', b'a', b't'];
        let mut src = [0u8; 16];
        // Cycle through patterns: each position gets a different base
        for offset in 0..8 {
            for i in 0..16 {
                src[i] = bases[(i + offset) % 8];
            }
            let simd = unsafe { encode_16_bases_2bit(&src) };
            let scalar = scalar_encode_16_2bit(&src);
            assert_eq!(
                simd,
                scalar,
                "mismatch at offset {offset}: src={:?}",
                std::str::from_utf8(&src)
            );

            let simd_case = unsafe { encode_16_bases_2bit_case(&src) };
            let scalar_case = scalar_encode_16_2bit_case(&src);
            assert_eq!(simd_case, scalar_case, "case mismatch at offset {offset}");
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_simd_vs_scalar_random_patterns() {
        // Pseudo-random patterns to stress-test bit manipulation.
        let bases_upper = [b'C', b'G', b'A', b'T'];
        let bases_lower = [b'c', b'g', b'a', b't'];
        let mut src = [0u8; 16];
        // Use a simple LCG for deterministic "random" patterns.
        let mut rng: u32 = 0xDEADBEEF;
        for _ in 0..1000 {
            for b in src.iter_mut() {
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                let idx = ((rng >> 16) & 3) as usize;
                let upper = ((rng >> 18) & 1) != 0;
                *b = if upper {
                    bases_upper[idx]
                } else {
                    bases_lower[idx]
                };
            }
            let simd = unsafe { encode_16_bases_2bit(&src) };
            let scalar = scalar_encode_16_2bit(&src);
            assert_eq!(
                simd,
                scalar,
                "mismatch: src={:?}",
                std::str::from_utf8(&src)
            );

            let simd_case = unsafe { encode_16_bases_2bit_case(&src) };
            let scalar_case = scalar_encode_16_2bit_case(&src);
            assert_eq!(
                simd_case,
                scalar_case,
                "case mismatch: src={:?}",
                std::str::from_utf8(&src)
            );
        }
    }

    #[test]
    fn test_round_trip_2bit_long_sequence() {
        // Sequence longer than 16 bases to exercise SIMD path + scalar tail.
        let seq = "ACGTACGTACGTACGTACGTACGTACGTACGTACG"; // 35 bases
        let input = format!(">s\n{}\n", seq);
        let output = round_trip_fasta(&input, AlphabetMode::TwoBit, CaseMode::Include, 0);
        assert_eq!(output, format!(">s\n{}\n", seq));
    }

    #[test]
    fn test_round_trip_2bit_mixed_case_long() {
        // 32 bases with mixed case to test SIMD case extraction.
        let seq = "AAccGGttAAccGGttAAccGGttAAccGGtt";
        let input = format!(">s\n{}\n", seq);
        let output = round_trip_fasta(&input, AlphabetMode::TwoBit, CaseMode::Include, 0);
        assert_eq!(output, format!(">s\n{}\n", seq));
    }

    #[test]
    fn test_round_trip_2bit_exactly_16() {
        let seq = "ACGTACGTACGTACGT"; // exactly 16 bases
        let input = format!(">s\n{}\n", seq);
        let output = round_trip_fasta(&input, AlphabetMode::TwoBit, CaseMode::Include, 0);
        assert_eq!(output, format!(">s\n{}\n", seq));
    }

    #[test]
    fn test_round_trip_2bit_exactly_32() {
        let seq = "ACGTACGTACGTACGTACGTACGTACGTACGT"; // exactly 32 bases
        let input = format!(">s\n{}\n", seq);
        let output = round_trip_fasta(&input, AlphabetMode::TwoBit, CaseMode::Include, 0);
        assert_eq!(output, format!(">s\n{}\n", seq));
    }

    #[test]
    fn test_round_trip_2bit_17_bases() {
        // 17 = 16 + 1: one SIMD batch + one scalar byte.
        let seq = "ACGTACGTACGTACGTA";
        let input = format!(">s\n{}\n", seq);
        let output = round_trip_fasta(&input, AlphabetMode::TwoBit, CaseMode::Include, 0);
        assert_eq!(output, format!(">s\n{}\n", seq));
    }
}
