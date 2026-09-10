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
use crate::stream::traits::{write_u16le, write_u32le, write_u64le, write_u8, StreamWrite};
use std::io;

/// Builds the index during encoding. Updated by the codec (sequence events)
/// and by the mux writer (block flush events).
pub struct IndexBuilder {
    tier: IndexTier,
    stream_count: usize,

    // Block table - always present
    block_entries: Vec<BlockTableEntry>,

    // Sequence table - tier 1 only
    sequence_entries: Vec<SequenceEntry>,

    // Running stream position counters (absolute uncompressed byte offsets)
    stream_positions: Vec<u64>,

    // Current sequence state (tier 1 only)
    current_header_offset: u64,
    current_header_length: u32,
    current_nuc_offset: u64,
    current_nuc_bit_offset: u8,
    current_case_offset: u64,
    current_ctrl_offset: u64,
    current_extra_offset: u64,
    current_quality_offset: u64,

    // Case checkpoint tracking (tier 1 only)
    current_case_base_count: u64,
    current_case_byte_count: u64,
    current_case_run_remainder: u16,
    current_case_is_upper: bool,
    current_checkpoints: Vec<CaseCheckpoint>,
}

impl IndexBuilder {
    pub fn new(tier: IndexTier, stream_count: usize) -> Self {
        Self {
            tier,
            stream_count,
            block_entries: Vec::new(),
            sequence_entries: Vec::new(),
            stream_positions: vec![0u64; stream_count],
            current_header_offset: 0,
            current_header_length: 0,
            current_nuc_offset: 0,
            current_nuc_bit_offset: 0,
            current_case_offset: 0,
            current_ctrl_offset: 0,
            current_extra_offset: 0,
            current_quality_offset: 0,
            current_case_base_count: 0,
            current_case_byte_count: 0,
            current_case_run_remainder: 0,
            current_case_is_upper: false,
            current_checkpoints: Vec::new(),
        }
    }

    /// Called when the codec writes data to a stream. This must be called
    /// AFTER writing to the mux to keep stream positions in sync.
    pub fn advance_stream(&mut self, stream_id: usize, bytes: usize) {
        self.stream_positions[stream_id] += bytes as u64;
    }

    /// Called by the mux writer before each block flush to record the
    /// block's file offset and current cumulative stream sizes.
    pub fn record_block(&mut self, file_offset: u64, cumulative: Vec<u64>) {
        self.block_entries.push(BlockTableEntry {
            file_offset,
            cumulative_uncompressed: cumulative,
        });
    }

    /// Called when the codec starts a new record. Must be called BEFORE
    /// writing the header bytes to the mux so that stream positions are
    /// snapshotted at the correct point.
    pub fn begin_sequence(&mut self, header: &[u8], nuc_bit_offset: u8) {
        if self.tier != IndexTier::Full {
            return;
        }

        self.current_header_offset = self.stream_positions[crate::container::STREAM_HDR];
        self.current_header_length = header.len() as u32;
        self.current_nuc_offset = self.stream_positions[crate::container::STREAM_NUC];
        self.current_nuc_bit_offset = nuc_bit_offset;
        self.current_case_offset = self.stream_positions[crate::container::STREAM_CASE];
        self.current_ctrl_offset = self.stream_positions[crate::container::STREAM_CTRL];
        self.current_extra_offset = self.stream_positions[crate::container::STREAM_EXTRA];
        self.current_quality_offset = if self.stream_count > 5 {
            self.stream_positions[crate::container::STREAM_QUALITY]
        } else {
            0
        };

        // Reset case checkpoint tracking for this sequence
        self.current_case_base_count = 0;
        self.current_case_byte_count = 0;
        self.current_case_run_remainder = 0;
        self.current_case_is_upper = false;
        self.current_checkpoints.clear();
    }

    /// Called when the codec finishes a record.
    pub fn end_sequence(&mut self, total_bases: u64) {
        if self.tier != IndexTier::Full {
            return;
        }

        self.sequence_entries.push(SequenceEntry {
            header_offset: self.current_header_offset,
            header_length: self.current_header_length,
            total_bases,
            nuc_offset: self.current_nuc_offset,
            nuc_bit_offset: self.current_nuc_bit_offset,
            case_offset: self.current_case_offset,
            ctrl_offset: self.current_ctrl_offset,
            extra_offset: self.current_extra_offset,
            quality_offset: self.current_quality_offset,
            checkpoints: std::mem::take(&mut self.current_checkpoints),
        });
    }

    /// Called by the codec/case encoder to update case checkpoint tracking.
    /// `rle_byte` is a single RLE byte being written, `bases_decoded` is how
    /// many bases this byte accounts for.
    pub fn track_case_rle(&mut self, rle_byte: u8, bases_decoded: u64) {
        if self.tier != IndexTier::Full {
            return;
        }

        let old_base_count = self.current_case_base_count;
        let new_base_count = old_base_count + bases_decoded;

        // Check if we cross any checkpoint boundary
        let old_interval = old_base_count / CHECKPOINT_INTERVAL;
        let new_interval = new_base_count / CHECKPOINT_INTERVAL;

        if new_interval > old_interval {
            // We crossed at least one checkpoint boundary.
            // Record a checkpoint at each crossing.
            for i in (old_interval + 1)..=new_interval {
                let boundary = i * CHECKPOINT_INTERVAL;
                // At the checkpoint boundary, compute remainder
                let consumed_past_boundary = new_base_count - boundary;
                let remainder = if consumed_past_boundary <= rle_byte as u64 {
                    (rle_byte as u64 - (bases_decoded - consumed_past_boundary)) as u16
                } else {
                    0
                };
                self.current_checkpoints.push(CaseCheckpoint {
                    base_offset: boundary,
                    case_byte_offset: self.current_case_byte_count,
                    run_remainder: remainder,
                    is_upper: self.current_case_is_upper,
                });
            }
        }

        self.current_case_base_count = new_base_count;
        self.current_case_byte_count += 1;
        self.current_case_is_upper = !self.current_case_is_upper;
    }

    /// Get the tier of this index.
    pub fn tier(&self) -> IndexTier {
        self.tier
    }

    /// Build the final MuxIndex.
    pub fn build(self) -> MuxIndex {
        MuxIndex {
            tier: self.tier,
            stream_count: self.stream_count as u8,
            total_stream_sizes: self.stream_positions.clone(),
            block_entries: self.block_entries,
            sequence_entries: self.sequence_entries,
        }
    }

    /// Serialise the index to a writer. Returns the number of bytes written.
    pub fn write_to(&self, w: &mut dyn StreamWrite) -> io::Result<u64> {
        let mut written = 0u64;

        // Index header
        w.write_all(IDX_MAGIC)?;
        written += 3;
        write_u8(w, IDX_VERSION)?;
        written += 1;
        write_u8(w, self.tier.to_byte())?;
        written += 1;
        write_u8(w, self.stream_count as u8)?;
        written += 1;
        write_u32le(w, self.block_entries.len() as u32)?;
        written += 4;
        write_u32le(w, self.sequence_entries.len() as u32)?;
        written += 4;

        // Total stream sizes
        for &sz in &self.stream_positions {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::*;
    use crate::index::reader::read_index;
    use crate::stream::traits::StreamRead;

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
    fn test_index_builder_tier0_basic() {
        let mut ib = IndexBuilder::new(IndexTier::BlockOnly, 5);

        // Simulate two block flushes
        ib.record_block(100, vec![0, 0, 0, 0, 0]);
        ib.record_block(200, vec![1024, 512, 10, 50, 0]);

        let index = ib.build();
        assert_eq!(index.block_count(), 2);
        assert_eq!(index.sequence_count(), 0);
        assert_eq!(index.block_entries[0].file_offset, 100);
        assert_eq!(index.block_entries[1].cumulative_uncompressed[0], 1024);
    }

    #[test]
    fn test_index_builder_tier1_basic() {
        let mut ib = IndexBuilder::new(IndexTier::Full, 5);

        // Simulate writing a sequence
        ib.begin_sequence(b"chr1", 0);
        ib.advance_stream(STREAM_CTRL, 5);
        ib.advance_stream(STREAM_HDR, 4);
        ib.advance_stream(STREAM_NUC, 100);
        ib.advance_stream(STREAM_CASE, 10);
        ib.end_sequence(400);

        ib.record_block(0, vec![5, 100, 10, 4, 0]);

        let index = ib.build();
        assert_eq!(index.sequence_count(), 1);
        assert_eq!(index.sequence_entries[0].header_length, 4);
        assert_eq!(index.sequence_entries[0].total_bases, 400);
        assert_eq!(index.sequence_entries[0].header_offset, 0);
        assert_eq!(index.sequence_entries[0].nuc_offset, 0);
    }

    #[test]
    fn test_index_builder_begin_before_write() {
        let mut ib = IndexBuilder::new(IndexTier::Full, 5);

        // begin_sequence BEFORE any writes; offsets should be 0
        ib.begin_sequence(b"seq1", 0);
        assert_eq!(ib.current_header_offset, 0);
        assert_eq!(ib.current_nuc_offset, 0);

        // Now simulate writes
        ib.advance_stream(STREAM_CTRL, 5);
        ib.advance_stream(STREAM_HDR, 4);
        ib.advance_stream(STREAM_NUC, 50);
        ib.end_sequence(200);

        // Second sequence: offsets should reflect prior writes
        ib.begin_sequence(b"seq2", 0);
        assert_eq!(ib.current_header_offset, 4);
        assert_eq!(ib.current_nuc_offset, 50);

        ib.advance_stream(STREAM_CTRL, 5);
        ib.advance_stream(STREAM_HDR, 4);
        ib.advance_stream(STREAM_NUC, 30);
        ib.end_sequence(120);

        let index = ib.build();
        assert_eq!(index.sequence_count(), 2);
        assert_eq!(index.sequence_entries[0].header_offset, 0);
        assert_eq!(index.sequence_entries[0].nuc_offset, 0);
        assert_eq!(index.sequence_entries[1].header_offset, 4);
        assert_eq!(index.sequence_entries[1].nuc_offset, 50);
    }

    #[test]
    fn test_index_builder_tier0_ignores_sequences() {
        let mut ib = IndexBuilder::new(IndexTier::BlockOnly, 5);

        ib.begin_sequence(b"seq1", 0);
        ib.end_sequence(100);

        let index = ib.build();
        assert_eq!(index.sequence_count(), 0); // tier 0 ignores sequences
    }

    #[test]
    fn test_index_serialisation_round_trip_tier0() {
        let mut ib = IndexBuilder::new(IndexTier::BlockOnly, 5);
        ib.record_block(64, vec![0, 0, 0, 0, 0]);
        ib.record_block(1200, vec![500, 256, 5, 30, 0]);
        ib.record_block(2400, vec![1000, 512, 10, 60, 0]);

        let mut ws = VecStream::new();
        let bytes_written = ib.write_to(&mut ws).unwrap();
        assert!(bytes_written > 0);

        let mut rs = VecStream::from_bytes(ws.data);
        let index = read_index(&mut rs).unwrap();

        assert_eq!(index.tier, IndexTier::BlockOnly);
        assert_eq!(index.stream_count, 5);
        assert_eq!(index.block_count(), 3);
        assert_eq!(index.sequence_count(), 0);
        assert_eq!(index.block_entries[0].file_offset, 64);
        assert_eq!(index.block_entries[2].cumulative_uncompressed[1], 512);
    }

    #[test]
    fn test_index_serialisation_round_trip_tier1() {
        let mut ib = IndexBuilder::new(IndexTier::Full, 6);

        // Sequence 1
        ib.begin_sequence(b"chr1", 0);
        ib.advance_stream(STREAM_CTRL, 25);
        ib.advance_stream(STREAM_HDR, 4);
        ib.advance_stream(STREAM_NUC, 500);
        ib.advance_stream(STREAM_CASE, 20);
        ib.end_sequence(2000);

        // Sequence 2
        ib.begin_sequence(b"chr2", 2);
        ib.advance_stream(STREAM_CTRL, 25);
        ib.advance_stream(STREAM_HDR, 4);
        ib.advance_stream(STREAM_NUC, 300);
        ib.advance_stream(STREAM_CASE, 12);
        ib.end_sequence(1200);

        ib.record_block(100, vec![50, 800, 32, 8, 0, 0]);

        let mut ws = VecStream::new();
        ib.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let index = read_index(&mut rs).unwrap();

        assert_eq!(index.tier, IndexTier::Full);
        assert_eq!(index.stream_count, 6);
        assert_eq!(index.block_count(), 1);
        assert_eq!(index.sequence_count(), 2);

        let s1 = &index.sequence_entries[0];
        assert_eq!(s1.header_offset, 0);
        assert_eq!(s1.header_length, 4);
        assert_eq!(s1.total_bases, 2000);
        assert_eq!(s1.nuc_offset, 0);
        assert_eq!(s1.nuc_bit_offset, 0);

        let s2 = &index.sequence_entries[1];
        assert_eq!(s2.header_offset, 4);
        assert_eq!(s2.nuc_offset, 500);
        assert_eq!(s2.nuc_bit_offset, 2);
        assert_eq!(s2.total_bases, 1200);
    }

    #[test]
    fn test_index_serialisation_with_checkpoints() {
        let mut ib = IndexBuilder::new(IndexTier::Full, 5);

        ib.begin_sequence(b"long_seq", 0);

        // Manually add checkpoints
        ib.current_checkpoints.push(CaseCheckpoint {
            base_offset: 1_000_000,
            case_byte_offset: 100,
            run_remainder: 50,
            is_upper: true,
        });
        ib.current_checkpoints.push(CaseCheckpoint {
            base_offset: 2_000_000,
            case_byte_offset: 200,
            run_remainder: 30,
            is_upper: false,
        });

        ib.end_sequence(2_500_000);
        ib.record_block(0, vec![0, 0, 0, 0, 0]);

        let mut ws = VecStream::new();
        ib.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let index = read_index(&mut rs).unwrap();

        assert_eq!(index.sequence_entries[0].checkpoints.len(), 2);
        let cp1 = &index.sequence_entries[0].checkpoints[0];
        assert_eq!(cp1.base_offset, 1_000_000);
        assert_eq!(cp1.case_byte_offset, 100);
        assert_eq!(cp1.run_remainder, 50);
        assert!(cp1.is_upper);

        let cp2 = &index.sequence_entries[0].checkpoints[1];
        assert_eq!(cp2.base_offset, 2_000_000);
        assert!(!cp2.is_upper);
    }

    #[test]
    fn test_index_find_block() {
        let index = MuxIndex {
            tier: IndexTier::BlockOnly,
            stream_count: 5,
            total_stream_sizes: vec![3000, 1500, 30, 150, 0],
            block_entries: vec![
                BlockTableEntry {
                    file_offset: 100,
                    cumulative_uncompressed: vec![0, 0, 0, 0, 0],
                },
                BlockTableEntry {
                    file_offset: 200,
                    cumulative_uncompressed: vec![1000, 500, 10, 50, 0],
                },
                BlockTableEntry {
                    file_offset: 300,
                    cumulative_uncompressed: vec![2000, 1000, 20, 100, 0],
                },
            ],
            sequence_entries: vec![],
        };

        // Stream 0: blocks contain [0..1000), [1000..2000), [2000..?)
        assert_eq!(index.find_block(0, 0), 0);
        assert_eq!(index.find_block(0, 500), 0);
        assert_eq!(index.find_block(0, 999), 0);
        assert_eq!(index.find_block(0, 1000), 1);
        assert_eq!(index.find_block(0, 1500), 1);
        assert_eq!(index.find_block(0, 2000), 2);
        assert_eq!(index.find_block(0, 9999), 2);

        // Stream 1: blocks contain [0..500), [500..1000), [1000..?)
        assert_eq!(index.find_block(1, 0), 0);
        assert_eq!(index.find_block(1, 499), 0);
        assert_eq!(index.find_block(1, 500), 1);
        assert_eq!(index.find_block(1, 999), 1);
        assert_eq!(index.find_block(1, 1000), 2);
    }

    #[test]
    fn test_index_empty() {
        let ib = IndexBuilder::new(IndexTier::BlockOnly, 5);

        let mut ws = VecStream::new();
        ib.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let index = read_index(&mut rs).unwrap();

        assert_eq!(index.block_count(), 0);
        assert_eq!(index.sequence_count(), 0);
    }

    #[test]
    fn test_index_many_blocks() {
        let mut ib = IndexBuilder::new(IndexTier::BlockOnly, 5);
        for i in 0..100u64 {
            ib.record_block(i * 1000, vec![i * 512, i * 256, i * 5, i * 25, 0]);
        }

        let mut ws = VecStream::new();
        ib.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let index = read_index(&mut rs).unwrap();

        assert_eq!(index.block_count(), 100);
        assert_eq!(index.block_entries[50].file_offset, 50_000);
        assert_eq!(index.block_entries[99].cumulative_uncompressed[0], 99 * 512);
    }

    #[test]
    fn test_index_fastq_6_streams() {
        let mut ib = IndexBuilder::new(IndexTier::Full, 6);

        ib.begin_sequence(b"read1", 0);
        ib.advance_stream(STREAM_CTRL, 30);
        ib.advance_stream(STREAM_HDR, 5);
        ib.advance_stream(STREAM_NUC, 38);
        ib.advance_stream(STREAM_QUALITY, 150);
        ib.end_sequence(150);

        ib.record_block(0, vec![30, 38, 0, 5, 0, 150]);

        let mut ws = VecStream::new();
        ib.write_to(&mut ws).unwrap();

        let mut rs = VecStream::from_bytes(ws.data);
        let index = read_index(&mut rs).unwrap();

        assert_eq!(index.stream_count, 6);
        assert_eq!(index.sequence_entries[0].quality_offset, 0);
        assert_eq!(index.sequence_entries[0].total_bases, 150);
    }
}
