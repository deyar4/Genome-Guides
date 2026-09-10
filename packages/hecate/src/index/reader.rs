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

use crate::index::*;
use crate::stream::traits::{read_u16le, read_u32le, read_u64le, read_u8, StreamRead};
use std::io;

/// Read and deserialise an index from a byte stream.
pub fn read_index(r: &mut dyn StreamRead) -> io::Result<MuxIndex> {
    // Read and validate magic
    let mut magic = [0u8; 3];
    r.read_exact(&mut magic)?;
    if &magic != IDX_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid index magic",
        ));
    }

    let version = read_u8(r)?;
    if version != IDX_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported index version {}", version),
        ));
    }

    let tier_byte = read_u8(r)?;
    let tier = IndexTier::from_byte(tier_byte).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown index tier {}", tier_byte),
        )
    })?;

    let stream_count = read_u8(r)? as usize;
    let block_count = read_u32le(r)? as usize;
    let sequence_count = read_u32le(r)? as usize;

    // Read total stream sizes
    let mut total_stream_sizes = Vec::with_capacity(stream_count);
    for _ in 0..stream_count {
        total_stream_sizes.push(read_u64le(r)?);
    }

    // Read block table
    let mut block_entries = Vec::with_capacity(block_count);
    for _ in 0..block_count {
        let file_offset = read_u64le(r)?;
        let mut cumulative = Vec::with_capacity(stream_count);
        for _ in 0..stream_count {
            cumulative.push(read_u64le(r)?);
        }
        block_entries.push(BlockTableEntry {
            file_offset,
            cumulative_uncompressed: cumulative,
        });
    }

    // Read sequence table + checkpoints (tier 1 only)
    let mut sequence_entries = Vec::new();
    if tier == IndexTier::Full {
        sequence_entries.reserve(sequence_count);
        for _ in 0..sequence_count {
            let header_offset = read_u64le(r)?;
            let header_length = read_u32le(r)?;
            let total_bases = read_u64le(r)?;
            let nuc_offset = read_u64le(r)?;
            let nuc_bit_offset = read_u8(r)?;
            let case_offset = read_u64le(r)?;
            let ctrl_offset = read_u64le(r)?;
            let extra_offset = read_u64le(r)?;
            let quality_offset = read_u64le(r)?;
            let checkpoint_count = read_u16le(r)? as usize;

            let mut checkpoints = Vec::with_capacity(checkpoint_count);
            for _ in 0..checkpoint_count {
                let base_offset = read_u64le(r)?;
                let case_byte_offset = read_u64le(r)?;
                let run_remainder = read_u16le(r)?;
                let is_upper = read_u8(r)? != 0;
                checkpoints.push(CaseCheckpoint {
                    base_offset,
                    case_byte_offset,
                    run_remainder,
                    is_upper,
                });
            }

            sequence_entries.push(SequenceEntry {
                header_offset,
                header_length,
                total_bases,
                nuc_offset,
                nuc_bit_offset,
                case_offset,
                ctrl_offset,
                extra_offset,
                quality_offset,
                checkpoints,
            });
        }
    }

    Ok(MuxIndex {
        tier,
        stream_count: stream_count as u8,
        total_stream_sizes,
        block_entries,
        sequence_entries,
    })
}
