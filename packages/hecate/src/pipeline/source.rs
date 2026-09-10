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

use crate::codec::common::extract_nuc_code_from_packed;
use crate::codec::fasta::reconstruct_extra_byte_from_code;
use crate::codec::illumina::reconstruct_illumina_header;
use crate::codec::varint::decode_varint_u32;
use crate::container::*;
use crate::mux::reader::StreamingMuxReader;
use crate::pipeline::handler::{Handler, RecordInfo};
use crate::stream::traits::StreamRead;
use std::io;

const CHUNK_SIZE: usize = 65536;

/// Drive events from a Hecate .he file into a Handler.
pub fn drive_hecate_source(
    file_type: FileType,
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    illumina: bool,
    mux: &mut StreamingMuxReader<impl StreamRead>,
    handler: &mut dyn Handler,
) -> io::Result<()> {
    // When the handler doesn't need stream data, tell the mux reader to
    // skip decompressing NUC, CASE, EXTRA, and QUALITY streams entirely.
    if !handler.needs_stream_data() {
        mux.set_skip_mask(
            (1 << STREAM_NUC) | (1 << STREAM_CASE) | (1 << STREAM_EXTRA) | (1 << STREAM_QUALITY),
        );
    }
    let mut reconstructed_hdr = Vec::new();
    loop {
        let tag = mux.read_stream_u8(STREAM_CTRL)?;
        match tag {
            TAG_HEADER if illumina => {
                // Illumina tokenized header: prefix + 0x00 + suffix in STREAM_HDR,
                // varint(tile), varint(x), varint(y) in STREAM_HDR_COORDS
                let text_len = mux.read_stream_u32le(STREAM_CTRL)? as usize;
                let mut hdr_data = vec![0u8; text_len];
                mux.read_stream_exact(STREAM_HDR, &mut hdr_data)?;

                // Split at null byte
                let null_pos = hdr_data.iter().position(|&b| b == 0x00).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "illumina tokenized header missing null separator",
                    )
                })?;
                let prefix = &hdr_data[..null_pos];
                let suffix = &hdr_data[null_pos + 1..];

                // Read 3 varints from STREAM_HDR_COORDS
                let tile = read_varint_from_mux(mux)?;
                let x = read_varint_from_mux(mux)?;
                let y = read_varint_from_mux(mux)?;

                // Reconstruct the full header
                reconstructed_hdr.clear();
                reconstruct_illumina_header(prefix, tile, x, y, suffix, &mut reconstructed_hdr);
                handler.begin_record(&reconstructed_hdr)?;
            }
            TAG_HEADER_RAW if illumina => {
                // Raw fallback header in illumina file
                let header_len = mux.read_stream_u32le(STREAM_CTRL)? as usize;
                mux.ensure_available(STREAM_HDR, header_len)?;
                let hdr = mux.borrow_stream(STREAM_HDR);
                handler.begin_record(&hdr[..header_len])?;
                mux.advance_stream(STREAM_HDR, header_len);
            }
            TAG_HEADER => {
                let header_len = mux.read_stream_u32le(STREAM_CTRL)? as usize;
                mux.ensure_available(STREAM_HDR, header_len)?;
                let hdr = mux.borrow_stream(STREAM_HDR);
                handler.begin_record(&hdr[..header_len])?;
                mux.advance_stream(STREAM_HDR, header_len);
            }
            TAG_SEQ => {
                let packed_bytes = mux.read_stream_u64le(STREAM_CTRL)?;
                let base_count = mux.read_stream_u64le(STREAM_CTRL)?;
                let extra_count = mux.read_stream_u32le(STREAM_CTRL)?;
                let case_bytes = mux.read_stream_u32le(STREAM_CTRL)?;

                // Emit record_meta before any data events
                let meta = RecordInfo {
                    base_count,
                    packed_bytes,
                    extra_count,
                    case_bytes,
                    quality_bytes: 0,
                    nuc_skip: 0,
                };
                handler.record_meta(&meta)?;

                // Fast path: handler only needs metadata (e.g. listing).
                // Skip all data streams -- they were not decompressed.
                if !handler.needs_stream_data() {
                    if file_type == FileType::Fastq {
                        let qtag = mux.read_stream_u8(STREAM_CTRL)?;
                        if qtag != TAG_QUALITY {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "expected TAG_QUALITY after TAG_SEQ in FASTQ, got 0x{:02X}",
                                    qtag
                                ),
                            ));
                        }
                        let quality_len = mux.read_stream_u64le(STREAM_CTRL)?;
                        let info = RecordInfo {
                            base_count,
                            packed_bytes,
                            extra_count,
                            case_bytes,
                            quality_bytes: quality_len,
                            nuc_skip: 0,
                        };
                        handler.end_record(&info)?;
                    } else {
                        handler.end_record(&meta)?;
                    }
                    continue;
                }

                // Emit case chunks (zero-copy borrow from channel buffer)
                if case_mode == CaseMode::Include && case_bytes > 0 {
                    let mut remaining = case_bytes as usize;
                    while remaining > 0 {
                        let to_read = CHUNK_SIZE.min(remaining);
                        mux.ensure_available(STREAM_CASE, to_read)?;
                        let data = mux.borrow_stream(STREAM_CASE);
                        handler.case_chunk(&data[..to_read])?;
                        mux.advance_stream(STREAM_CASE, to_read);
                        remaining -= to_read;
                    }
                }

                let mut nuc_bytes_remaining = packed_bytes as usize;

                if extra_count == 0 {
                    // Fast path: no extras -- zero-copy borrow from channel buffer
                    while nuc_bytes_remaining > 0 {
                        let to_read = CHUNK_SIZE.min(nuc_bytes_remaining);
                        mux.ensure_available(STREAM_NUC, to_read)?;
                        let data = mux.borrow_stream(STREAM_NUC);
                        handler.nuc_chunk(&data[..to_read])?;
                        mux.advance_stream(STREAM_NUC, to_read);
                        nuc_bytes_remaining -= to_read;
                    }
                } else {
                    // Slow path: interleaved nuc + extras
                    let bases_per_byte = match alphabet {
                        AlphabetMode::TwoBit => 4u64,
                        AlphabetMode::FourBit => 2u64,
                        AlphabetMode::EightBit => 1u64,
                    };

                    let mut nuc_buf = [0u8; CHUNK_SIZE];
                    let mut base_offset: u64 = 0;
                    let mut extra_abs_pos: u64 = 0;
                    let mut extras_remaining = extra_count;
                    let mut pending_extra: Option<(u64, u8)> = None;

                    while nuc_bytes_remaining > 0 {
                        let to_read = CHUNK_SIZE.min(nuc_bytes_remaining);
                        mux.read_stream_exact(STREAM_NUC, &mut nuc_buf[..to_read])?;
                        nuc_bytes_remaining -= to_read;

                        let chunk_bases = if nuc_bytes_remaining == 0 {
                            base_count - base_offset
                        } else {
                            to_read as u64 * bases_per_byte
                        };
                        let chunk_base_end = base_offset + chunk_bases;

                        if let Some((pos, upper)) = pending_extra.take() {
                            if pos < chunk_base_end {
                                let nuc_code = extract_nuc_code_from_packed(
                                    alphabet,
                                    &nuc_buf[..to_read],
                                    pos - base_offset,
                                );
                                let ch =
                                    reconstruct_extra_byte_from_code(alphabet, nuc_code, upper);
                                handler.extra(pos, ch)?;
                            } else {
                                pending_extra = Some((pos, upper));
                            }
                        }

                        while extras_remaining > 0 && pending_extra.is_none() {
                            let delta = mux.read_stream_u32le(STREAM_EXTRA)? as u64;
                            let upper = mux.read_stream_u8(STREAM_EXTRA)?;
                            extra_abs_pos += delta;
                            extras_remaining -= 1;
                            if extra_abs_pos < chunk_base_end {
                                let nuc_code = extract_nuc_code_from_packed(
                                    alphabet,
                                    &nuc_buf[..to_read],
                                    extra_abs_pos - base_offset,
                                );
                                let ch =
                                    reconstruct_extra_byte_from_code(alphabet, nuc_code, upper);
                                handler.extra(extra_abs_pos, ch)?;
                            } else {
                                pending_extra = Some((extra_abs_pos, upper));
                            }
                        }

                        handler.nuc_chunk(&nuc_buf[..to_read])?;
                        base_offset = chunk_base_end;
                    }
                }

                let info = RecordInfo {
                    base_count,
                    packed_bytes,
                    extra_count,
                    case_bytes,
                    quality_bytes: 0,
                    nuc_skip: 0,
                };

                if file_type == FileType::Fasta {
                    handler.end_record(&info)?;
                } else {
                    let qtag = mux.read_stream_u8(STREAM_CTRL)?;
                    if qtag != TAG_QUALITY {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "expected TAG_QUALITY after TAG_SEQ in FASTQ, got 0x{:02X}",
                                qtag
                            ),
                        ));
                    }
                    let quality_len = mux.read_stream_u64le(STREAM_CTRL)?;

                    let mut remaining = quality_len as usize;
                    while remaining > 0 {
                        let to_read = CHUNK_SIZE.min(remaining);
                        mux.ensure_available(STREAM_QUALITY, to_read)?;
                        let data = mux.borrow_stream(STREAM_QUALITY);
                        handler.quality_chunk(&data[..to_read])?;
                        mux.advance_stream(STREAM_QUALITY, to_read);
                        remaining -= to_read;
                    }

                    let info = RecordInfo {
                        base_count,
                        packed_bytes,
                        extra_count,
                        case_bytes,
                        quality_bytes: quality_len,
                        nuc_skip: 0,
                    };
                    handler.end_record(&info)?;
                }
            }
            TAG_EOF => {
                handler.eof()?;
                break;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unknown control tag: 0x{:02X}", tag),
                ));
            }
        }
    }
    Ok(())
}

/// Read a single unsigned LEB128 varint from STREAM_HDR_COORDS.
fn read_varint_from_mux(mux: &mut StreamingMuxReader<impl StreamRead>) -> io::Result<u32> {
    // Read bytes one at a time until we get a complete varint
    let mut buf = Vec::with_capacity(5);
    loop {
        let byte = mux.read_stream_u8(STREAM_HDR_COORDS)?;
        buf.push(byte);
        if byte & 0x80 == 0 {
            break;
        }
        if buf.len() >= 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "varint overflow in HDR_COORDS stream",
            ));
        }
    }
    let (val, _) = decode_varint_u32(&buf)?;
    Ok(val)
}
