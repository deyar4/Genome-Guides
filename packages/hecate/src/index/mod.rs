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

pub mod builder;
pub mod reader;

/// Index tier determines the level of per-sequence metadata stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexTier {
    /// Tier 0: block offset table only. Default for FASTQ.
    BlockOnly,
    /// Tier 1: block table + per-sequence table with case checkpoints. Default for FASTA.
    Full,
}

impl IndexTier {
    pub fn to_byte(self) -> u8 {
        match self {
            IndexTier::BlockOnly => 0,
            IndexTier::Full => 1,
        }
    }

    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(IndexTier::BlockOnly),
            1 => Some(IndexTier::Full),
            _ => None, // Unknown tier: caller can fall back to BlockOnly
        }
    }
}

/// Magic bytes for the index header.
pub const IDX_MAGIC: &[u8; 3] = b"IDX";
pub const IDX_VERSION: u8 = 1;

/// Case checkpoint interval in bases.
pub const CHECKPOINT_INTERVAL: u64 = 1_000_000;

/// Entry in the block table. Stores the file offset and cumulative
/// uncompressed byte counts per stream at the start of this block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTableEntry {
    /// Absolute file offset where this block starts.
    pub file_offset: u64,
    /// Cumulative uncompressed bytes per stream in blocks 0..this_block (exclusive).
    /// Length = stream_count.
    pub cumulative_uncompressed: Vec<u64>,
}

/// Per-sequence entry (tier 1 only). Records stream positions at the
/// start of each sequence for random access by header name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceEntry {
    pub header_offset: u64,
    pub header_length: u32,
    pub total_bases: u64,
    pub nuc_offset: u64,
    pub nuc_bit_offset: u8,
    pub case_offset: u64,
    pub ctrl_offset: u64,
    pub extra_offset: u64,
    pub quality_offset: u64,
    pub checkpoints: Vec<CaseCheckpoint>,
}

/// Case checkpoint: periodic snapshot of case decoder state for fast
/// seeking into long sequences. Stored every CHECKPOINT_INTERVAL bases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseCheckpoint {
    /// Base index this checkpoint covers (multiple of CHECKPOINT_INTERVAL).
    pub base_offset: u64,
    /// Byte offset in the case stream at this point.
    pub case_byte_offset: u64,
    /// Remaining bases in the current RLE run.
    pub run_remainder: u16,
    /// Whether the current run is uppercase.
    pub is_upper: bool,
}

/// Full in-memory index.
#[derive(Debug, Clone)]
pub struct MuxIndex {
    pub tier: IndexTier,
    pub stream_count: u8,
    /// Total uncompressed bytes per stream after all blocks.
    pub total_stream_sizes: Vec<u64>,
    pub block_entries: Vec<BlockTableEntry>,
    pub sequence_entries: Vec<SequenceEntry>, // empty for tier 0
}

impl MuxIndex {
    /// Find the block containing the given absolute uncompressed offset
    /// in the specified stream. Binary search on cumulative sizes.
    pub fn find_block(&self, stream_id: usize, offset: u64) -> usize {
        // cumulative_uncompressed[s] is the total bytes in stream s in blocks 0..block_id.
        // We want the last block_id where cumulative <= offset.
        self.block_entries
            .partition_point(|e| e.cumulative_uncompressed[stream_id] <= offset)
            .saturating_sub(1)
    }

    /// Get the cumulative uncompressed size for a stream at a given block.
    pub fn cumulative_size(&self, stream_id: usize, block_id: usize) -> u64 {
        self.block_entries[block_id].cumulative_uncompressed[stream_id]
    }

    /// Total number of blocks.
    pub fn block_count(&self) -> usize {
        self.block_entries.len()
    }

    /// Total number of sequences (0 for tier 0).
    pub fn sequence_count(&self) -> usize {
        self.sequence_entries.len()
    }

    /// Serialise the index to a writer. Returns the number of bytes written.
    /// This mirrors the format used by `IndexBuilder::write_to()`.
    pub fn write_to(&self, w: &mut dyn crate::stream::traits::StreamWrite) -> std::io::Result<u64> {
        use crate::stream::traits::{write_u16le, write_u32le, write_u64le, write_u8};

        let mut written = 0u64;

        // Index header
        w.write_all(IDX_MAGIC)?;
        written += 3;
        write_u8(w, IDX_VERSION)?;
        written += 1;
        write_u8(w, self.tier.to_byte())?;
        written += 1;
        write_u8(w, self.stream_count)?;
        written += 1;
        write_u32le(w, self.block_entries.len() as u32)?;
        written += 4;
        write_u32le(w, self.sequence_entries.len() as u32)?;
        written += 4;

        // Total stream sizes
        for &sz in &self.total_stream_sizes {
            write_u64le(w, sz)?;
            written += 8;
        }

        // Block table
        for entry in &self.block_entries {
            write_u64le(w, entry.file_offset)?;
            written += 8;
            for &cum in &entry.cumulative_uncompressed {
                write_u64le(w, cum)?;
                written += 8;
            }
        }

        // Sequence table + checkpoints (tier 1 only)
        if self.tier == IndexTier::Full {
            for entry in &self.sequence_entries {
                write_u64le(w, entry.header_offset)?;
                write_u32le(w, entry.header_length)?;
                write_u64le(w, entry.total_bases)?;
                write_u64le(w, entry.nuc_offset)?;
                write_u8(w, entry.nuc_bit_offset)?;
                write_u64le(w, entry.case_offset)?;
                write_u64le(w, entry.ctrl_offset)?;
                write_u64le(w, entry.extra_offset)?;
                write_u64le(w, entry.quality_offset)?;
                write_u16le(w, entry.checkpoints.len() as u16)?;
                written += 63;

                for cp in &entry.checkpoints {
                    write_u64le(w, cp.base_offset)?;
                    write_u64le(w, cp.case_byte_offset)?;
                    write_u16le(w, cp.run_remainder)?;
                    write_u8(w, cp.is_upper as u8)?;
                    written += 19;
                }
            }
        }

        Ok(written)
    }
}
