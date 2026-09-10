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
use crate::index::CaseCheckpoint;
use crate::mux::indexed_reader::IndexedMuxReader;
use crate::pipeline::handler::{Handler, RecordInfo};
use crate::processor::slice_filter::{SliceFilter, SliceSpec};
use crate::stream::traits::{StreamRead, StreamSeek};
use std::io;

const CHUNK_SIZE: usize = 65536;

/// Drive events from an indexed .he file into a SliceFilter, reading only
/// the blocks needed for sequences matching the given slice specs.
/// Requires a Full-tier index with per-sequence entries.
///
/// When a slice spec has a sub-range and the sequence has case checkpoints,
/// the nearest checkpoint before the range start is used to skip reading
/// and decoding unnecessary case data.
pub fn drive_indexed_slice(
    reader: &mut IndexedMuxReader<impl StreamRead + StreamSeek>,
    file_type: FileType,
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    illumina: bool,
    slice_specs: &[SliceSpec],
    handler: &mut SliceFilter<'_>,
) -> io::Result<()> {
    let index = reader.index().clone();

    if index.sequence_entries.is_empty() {
        handler.eof()?;
        return Ok(());
    }

    let seq_count = index.sequence_entries.len();
    let stream_count = index.stream_count as usize;

    // Check if we have any name/regex-based specs (non-ordinal).
    let has_name_specs = slice_specs.iter().any(|s| !s.matcher.is_ordinal());

    // For illumina files, read the entire HDR_COORDS stream so we can
    // reconstruct full headers from the tokenized form + coordinates.
    let coords_data = if illumina && stream_count > STREAM_HDR_COORDS {
        let total = index.total_stream_sizes[STREAM_HDR_COORDS];
        let mut buf = Vec::new();
        if total > 0 {
            reader.read_stream_range_into(STREAM_HDR_COORDS, 0, total, &mut buf)?;
        }
        buf
    } else {
        Vec::new()
    };

    // Phase 1: scan headers to find matching sequences.
    let mut matches: Vec<(usize, Vec<u8>)> = Vec::new();
    let mut hdr_buf = Vec::new();
    let mut reconstructed_hdr = Vec::new();
    let mut coords_pos: usize = 0; // position in coords_data

    if has_name_specs {
        // Need to scan all headers for name/regex matches.
        let hdr_skip_mask = (1u32 << STREAM_CTRL)
            | (1 << STREAM_NUC)
            | (1 << STREAM_CASE)
            | (1 << STREAM_EXTRA)
            | if stream_count > 5 {
                1 << STREAM_QUALITY
            } else {
                0
            }
            | if illumina && stream_count > STREAM_HDR_COORDS {
                1 << STREAM_HDR_COORDS
            } else {
                0
            };
        reader.set_skip_mask(hdr_skip_mask);

        for seq_idx in 0..seq_count {
            let entry = &index.sequence_entries[seq_idx];
            reader.read_stream_range_into(
                STREAM_HDR,
                entry.header_offset,
                entry.header_offset + entry.header_length as u64,
                &mut hdr_buf,
            )?;

            // Reconstruct illumina headers from tokenized form + coords
            let header_for_match = if illumina && !coords_data.is_empty() {
                if let Some(reconstructed) = reconstruct_indexed_illumina_header(
                    &hdr_buf,
                    &coords_data,
                    &mut coords_pos,
                    &mut reconstructed_hdr,
                ) {
                    reconstructed
                } else {
                    // Raw fallback header (no null separator)
                    &hdr_buf
                }
            } else {
                &hdr_buf
            };

            if slice_specs
                .iter()
                .any(|spec| spec.matches_with_ordinal(header_for_match, seq_idx))
            {
                matches.push((seq_idx, header_for_match.to_vec()));
            }
        }
    } else {
        // Pure ordinal: skip full header scan, read only matching headers.
        let hdr_skip_mask = (1u32 << STREAM_CTRL)
            | (1 << STREAM_NUC)
            | (1 << STREAM_CASE)
            | (1 << STREAM_EXTRA)
            | if stream_count > 5 {
                1 << STREAM_QUALITY
            } else {
                0
            }
            | if illumina && stream_count > STREAM_HDR_COORDS {
                1 << STREAM_HDR_COORDS
            } else {
                0
            };
        reader.set_skip_mask(hdr_skip_mask);

        // For illumina ordinal access, we still need to scan coords up to
        // the target index to know the right coords_pos offset.
        let mut ordinal_indices: Vec<usize> = Vec::new();
        for spec in slice_specs {
            ordinal_indices.extend(spec.matcher.ordinal_indices(seq_count));
        }
        ordinal_indices.sort_unstable();
        ordinal_indices.dedup();

        if illumina && !coords_data.is_empty() {
            // Scan all headers sequentially to track coords_pos
            let mut next_ord = 0usize;
            for seq_idx in 0..seq_count {
                if next_ord >= ordinal_indices.len() {
                    break;
                }
                let entry = &index.sequence_entries[seq_idx];
                reader.read_stream_range_into(
                    STREAM_HDR,
                    entry.header_offset,
                    entry.header_offset + entry.header_length as u64,
                    &mut hdr_buf,
                )?;

                let header = if let Some(reconstructed) = reconstruct_indexed_illumina_header(
                    &hdr_buf,
                    &coords_data,
                    &mut coords_pos,
                    &mut reconstructed_hdr,
                ) {
                    reconstructed.to_vec()
                } else {
                    hdr_buf.clone()
                };

                if seq_idx == ordinal_indices[next_ord] {
                    matches.push((seq_idx, header));
                    next_ord += 1;
                }
            }
        } else {
            for idx in ordinal_indices {
                let entry = &index.sequence_entries[idx];
                reader.read_stream_range_into(
                    STREAM_HDR,
                    entry.header_offset,
                    entry.header_offset + entry.header_length as u64,
                    &mut hdr_buf,
                )?;
                matches.push((idx, hdr_buf.clone()));
            }
        }
    }

    // Phase 2: read data for matching sequences with full decompression.
    reader.set_skip_mask(0);
    reader.clear_cache();

    // Reusable buffers across all matching sequences.
    let mut stream_buf = Vec::new();

    for (seq_idx, header) in &matches {
        let seq_idx = *seq_idx;
        let entry = &index.sequence_entries[seq_idx];

        // Set ordinal before begin_record so the filter knows the position.
        handler.set_ordinal(seq_idx);

        // Find which slice spec matched this sequence (for checkpoint lookup)
        let matching_spec = slice_specs
            .iter()
            .find(|s| s.matches_with_ordinal(header, seq_idx));

        // Compute byte ranges for each stream by looking at the next sequence's offsets
        let next_nuc = if seq_idx + 1 < seq_count {
            index.sequence_entries[seq_idx + 1].nuc_offset
        } else {
            index.total_stream_sizes[STREAM_NUC]
        };
        let next_case = if seq_idx + 1 < seq_count {
            index.sequence_entries[seq_idx + 1].case_offset
        } else {
            index.total_stream_sizes[STREAM_CASE]
        };
        let next_extra = if seq_idx + 1 < seq_count {
            index.sequence_entries[seq_idx + 1].extra_offset
        } else {
            index.total_stream_sizes[STREAM_EXTRA]
        };
        let next_quality = if seq_idx + 1 < seq_count && stream_count > 5 {
            index.sequence_entries[seq_idx + 1].quality_offset
        } else if stream_count > 5 {
            index.total_stream_sizes[STREAM_QUALITY]
        } else {
            0
        };

        let nuc_bytes = next_nuc - entry.nuc_offset;
        let case_bytes = next_case - entry.case_offset;
        let extra_bytes = next_extra - entry.extra_offset;
        let quality_bytes = if stream_count > 5 {
            next_quality - entry.quality_offset
        } else {
            0
        };
        let extra_count = (extra_bytes / 5) as u32;

        // Emit begin_record
        handler.begin_record(header)?;

        // Emit record_meta
        let meta = RecordInfo {
            base_count: entry.total_bases,
            packed_bytes: nuc_bytes,
            extra_count,
            case_bytes: case_bytes as u32,
            quality_bytes: 0,
            nuc_skip: 0,
        };
        handler.record_meta(&meta)?;

        // Emit case chunks, using checkpoint optimization when available.
        if case_mode == CaseMode::Include && case_bytes > 0 {
            let checkpoint = find_best_checkpoint(matching_spec, &entry.checkpoints);
            let (case_start, case_end) = if let Some(cp) = checkpoint {
                handler.apply_case_checkpoint(cp.base_offset, cp.run_remainder, cp.is_upper);
                (
                    entry.case_offset + cp.case_byte_offset + 1,
                    entry.case_offset + case_bytes,
                )
            } else {
                (entry.case_offset, entry.case_offset + case_bytes)
            };

            if case_start < case_end {
                reader.read_stream_range_into(
                    STREAM_CASE,
                    case_start,
                    case_end,
                    &mut stream_buf,
                )?;
                for chunk in stream_buf.chunks(CHUNK_SIZE) {
                    handler.case_chunk(chunk)?;
                }
            }
        }

        // Read nuc data
        reader.read_stream_range_into(
            STREAM_NUC,
            entry.nuc_offset,
            entry.nuc_offset + nuc_bytes,
            &mut stream_buf,
        )?;

        if extra_count == 0 {
            for chunk in stream_buf.chunks(CHUNK_SIZE) {
                handler.nuc_chunk(chunk)?;
            }
        } else {
            // Slow path: need both nuc and extra data simultaneously.
            // Swap nuc_data out of stream_buf so we can reuse it for extras.
            let mut nuc_data = Vec::new();
            std::mem::swap(&mut nuc_data, &mut stream_buf);

            reader.read_stream_range_into(
                STREAM_EXTRA,
                entry.extra_offset,
                entry.extra_offset + extra_bytes,
                &mut stream_buf,
            )?;

            emit_nuc_with_extras(
                alphabet,
                &nuc_data,
                &stream_buf,
                entry.total_bases,
                extra_count,
                handler,
            )?;

            // Reclaim the larger buffer for reuse.
            if nuc_data.capacity() > stream_buf.capacity() {
                stream_buf = nuc_data;
            }
        }

        // Emit quality for FASTQ
        if file_type == FileType::Fastq && quality_bytes > 0 {
            reader.read_stream_range_into(
                STREAM_QUALITY,
                entry.quality_offset,
                entry.quality_offset + quality_bytes,
                &mut stream_buf,
            )?;
            for chunk in stream_buf.chunks(CHUNK_SIZE) {
                handler.quality_chunk(chunk)?;
            }
        }

        // Emit end_record
        let info = RecordInfo {
            base_count: entry.total_bases,
            packed_bytes: nuc_bytes,
            extra_count,
            case_bytes: case_bytes as u32,
            quality_bytes,
            nuc_skip: 0,
        };
        handler.end_record(&info)?;
    }

    handler.eof()?;
    Ok(())
}

/// Emit nuc chunks with interleaved extra events, mirroring the logic
/// in `drive_hecate_source` but operating on already-read byte slices.
fn emit_nuc_with_extras(
    alphabet: AlphabetMode,
    nuc_data: &[u8],
    extra_data: &[u8],
    base_count: u64,
    extra_count: u32,
    handler: &mut dyn Handler,
) -> io::Result<()> {
    let bases_per_byte: u64 = match alphabet {
        AlphabetMode::TwoBit => 4,
        AlphabetMode::FourBit => 2,
        AlphabetMode::EightBit => 1,
    };

    // Decode all extras from the raw extra stream data
    let mut extras: Vec<(u64, u8)> = Vec::with_capacity(extra_count as usize);
    let mut abs_pos: u64 = 0;
    let mut epos = 0usize;
    for _ in 0..extra_count {
        if epos + 5 > extra_data.len() {
            break;
        }
        let delta = u32::from_le_bytes([
            extra_data[epos],
            extra_data[epos + 1],
            extra_data[epos + 2],
            extra_data[epos + 3],
        ]) as u64;
        let upper = extra_data[epos + 4];
        epos += 5;
        abs_pos += delta;
        extras.push((abs_pos, upper));
    }

    // Emit nuc chunks with interleaved extras
    let mut base_offset: u64 = 0;
    let mut extra_idx: usize = 0;
    let mut nuc_pos: usize = 0;

    while nuc_pos < nuc_data.len() {
        let to_read = CHUNK_SIZE.min(nuc_data.len() - nuc_pos);
        let chunk = &nuc_data[nuc_pos..nuc_pos + to_read];

        let chunk_bases = if nuc_pos + to_read >= nuc_data.len() {
            base_count - base_offset
        } else {
            to_read as u64 * bases_per_byte
        };
        let chunk_base_end = base_offset + chunk_bases;

        // Emit extras that fall within this chunk
        while extra_idx < extras.len() && extras[extra_idx].0 < chunk_base_end {
            let (pos, upper) = extras[extra_idx];
            let nuc_code = extract_nuc_code_from_packed(alphabet, chunk, pos - base_offset);
            let ch = reconstruct_extra_byte_from_code(alphabet, nuc_code, upper);
            handler.extra(pos, ch)?;
            extra_idx += 1;
        }

        handler.nuc_chunk(chunk)?;
        nuc_pos += to_read;
        base_offset = chunk_base_end;
    }

    Ok(())
}

/// Reconstruct a full illumina header from its tokenized form + coords data.
/// Returns Some(slice) pointing into `out` on success, None if this is a raw
/// fallback header (no null separator found).
fn reconstruct_indexed_illumina_header<'a>(
    tokenized: &[u8],
    coords_data: &[u8],
    coords_pos: &mut usize,
    out: &'a mut Vec<u8>,
) -> Option<&'a [u8]> {
    // Tokenized format: prefix + 0x00 + suffix
    let null_pos = tokenized.iter().position(|&b| b == 0x00)?;
    let prefix = &tokenized[..null_pos];
    let suffix = &tokenized[null_pos + 1..];

    // Read 3 varints from coords_data at coords_pos
    let (tile, consumed) = decode_varint_u32(&coords_data[*coords_pos..]).ok()?;
    *coords_pos += consumed;
    let (x, consumed) = decode_varint_u32(&coords_data[*coords_pos..]).ok()?;
    *coords_pos += consumed;
    let (y, consumed) = decode_varint_u32(&coords_data[*coords_pos..]).ok()?;
    *coords_pos += consumed;

    out.clear();
    reconstruct_illumina_header(prefix, tile, x, y, suffix, out);
    Some(out.as_slice())
}

/// Find the best case checkpoint for a slice spec's sub-range.
/// Returns the checkpoint with the largest `base_offset` that is still
/// strictly before the sub-range start, or None if no useful checkpoint exists.
fn find_best_checkpoint<'a>(
    spec: Option<&SliceSpec>,
    checkpoints: &'a [CaseCheckpoint],
) -> Option<&'a CaseCheckpoint> {
    let (start, _end) = spec?.base_range()?;
    if checkpoints.is_empty() || start == 0 {
        return None;
    }
    // Binary search for the last checkpoint with base_offset <= start.
    let idx = checkpoints.partition_point(|cp| cp.base_offset <= start);
    if idx == 0 {
        None
    } else {
        Some(&checkpoints[idx - 1])
    }
}
