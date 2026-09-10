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

use crate::compress::bwt_codec::BwtCompressor;
use crate::compress::rans_codec::RansCompressor;
use crate::compress::traits::Compressor;
use crate::compress::markov_codec::MarkovCompressor;
use crate::compress::xz_codec::{xz_compress_block, XzCompressor, XzParams};
use crate::compress::zstd_codec::{zstd_compress_block, ZstdCompressor};
use crate::container::MethodSpec;
use crate::index::builder::IndexBuilder;
use crate::index::IndexTier;
use crate::mux::block::BlockHeader;
use crate::stream::traits::StreamWrite;
use crate::util::available_memory_bytes;
use rayon::prelude::*;
use std::io;

pub const DEFAULT_BLOCK_BUDGET: usize = 1024 * 1024; // 1 MiB

/// Estimate peak memory for compressing one block-stream with the given
/// method.  Used to bound Rayon parallelism and avoid OOM.
/// Parallel tasks are single-threaded (NbWorkers=1).
fn compress_task_memory(spec: &MethodSpec, input_size: usize) -> usize {
    match spec {
        MethodSpec::Zstd { level, window_log } => {
            ZstdCompressor::estimate_memory(*level as i32, *window_log, 1, input_size)
        }
        MethodSpec::Xz {
            preset,
            extreme,
            dict_size,
            lc,
            pb,
        } => {
            let params = XzParams {
                preset: *preset,
                extreme: *extreme,
                dict_size: *dict_size,
                lc: *lc,
                pb: *pb,
            };
            XzCompressor::estimate_memory(&params, 1, input_size)
        }
        MethodSpec::Bwt { block_mib } => {
            BwtCompressor::estimate_compress_memory(*block_mib, input_size)
        }
        MethodSpec::Markov { .. } => {
            MarkovCompressor::estimate_memory(input_size)
        }
        MethodSpec::Rans => {
            // Encode tables + histogram + output buffer
            input_size * 2 + 2 * 1024 * 1024
        }
    }
}

/// Uncompressed block data queued for batched parallel compression.
struct PendingBlock {
    buffers: Vec<Vec<u8>>,
    uncompressed_sizes: Vec<u64>,
    crc32: u32,
    /// Per-stream cumulative uncompressed byte counts *before* this block.
    cumulative_before: Vec<u64>,
}

/// Block-based multiplexer: accumulates per-stream data, flushes blocks
/// when total pending data exceeds the budget.
///
/// In indexed mode, blocks are batched and compressed in parallel across
/// (block, stream) pairs using Rayon work-stealing, keeping cores busy even
/// when there are only a handful of streams and most are tiny.
pub struct MuxWriter<W: StreamWrite> {
    output: W,
    stream_count: usize,
    block_budget: usize,
    buffers: Vec<Vec<u8>>,
    total_pending: usize,
    compressors: Vec<Option<Box<dyn Compressor>>>,
    method_specs: Vec<Option<MethodSpec>>,
    indexed: bool,
    /// Use batched parallel compression.  True when indexed, or when all
    /// methods are block-independent (BWT / raw) so blocks can be compressed
    /// concurrently via Rayon instead of sequentially.
    batch_compress: bool,
    // Index building state
    block_offsets: Vec<u64>,
    cumulative_sizes: Vec<Vec<u64>>, // [stream][block] cumulative uncompressed bytes
    current_file_offset: u64,
    compress_buf: Vec<u8>,
    compressed_data: Vec<Vec<u8>>,
    index_builder: Option<IndexBuilder>,
    // Batched compression
    pending_blocks: Vec<PendingBlock>,
    batch_threshold: usize,
    /// Maximum concurrent compression threads (bounded by memory).
    max_compress_threads: usize,
}

impl<W: StreamWrite> MuxWriter<W> {
    pub fn new(
        output: W,
        stream_count: usize,
        block_budget: usize,
        compressors: Vec<Option<Box<dyn Compressor>>>,
        method_specs: Vec<Option<MethodSpec>>,
        index_tier: Option<IndexTier>,
        initial_offset: u64,
    ) -> Self {
        assert_eq!(compressors.len(), stream_count);
        assert_eq!(method_specs.len(), stream_count);
        assert!(
            stream_count <= 8,
            "stream_count {} exceeds flags u8 capacity (max 8)",
            stream_count
        );
        let buffers = (0..stream_count).map(|_| Vec::new()).collect();
        let cumulative_sizes = (0..stream_count).map(|_| vec![0u64]).collect();
        let indexed = index_tier.is_some();
        let index_builder = index_tier.map(|tier| IndexBuilder::new(tier, stream_count));

        // Batch compression when all methods are block-independent (BWT,
        // rANS, or raw).  Zstd and xz use streaming frames with worker
        // pools, so they must stay on the sequential per-block path
        // (unless we're indexed, in which case every block is a fresh
        // frame anyway).
        let batch_compress = indexed
            || method_specs.iter().all(|ms| {
                ms.is_none()
                    || matches!(
                        ms,
                        Some(
                            MethodSpec::Bwt { .. }
                                | MethodSpec::Markov { .. }
                                | MethodSpec::Rans
                        )
                    )
            });

        // Compute max parallel threads from memory budget.  Each Rayon task
        // compresses one (block, stream) pair; the heaviest stream
        // dominates.  Bound concurrent tasks so total peak memory stays
        // within available RAM (MemAvailable on Linux, avail_phys on
        // Windows).  We deduct the pending-block overhead (uncompressed
        // data held during compression) and apply a 50% safety margin
        // for output buffers, I/O, and other process overhead.
        let cores = num_cpus::get().max(1);
        let batch_threshold = cores.max(4);
        let max_compress_threads = if batch_compress {
            let max_per_task = method_specs
                .iter()
                .filter_map(|ms| ms.as_ref())
                .map(|ms| compress_task_memory(ms, block_budget))
                .max()
                .unwrap_or(block_budget);
            let avail = available_memory_bytes();
            // Pending blocks stay resident while compressors run.
            let pending_overhead = batch_threshold * block_budget;
            let budget = avail.saturating_sub(pending_overhead) / 2;
            let by_mem = if max_per_task > 0 {
                (budget / max_per_task).max(1)
            } else {
                cores
            };
            by_mem.min(cores)
        } else {
            cores
        };

        Self {
            output,
            stream_count,
            block_budget,
            buffers,
            total_pending: 0,
            compressors,
            method_specs,
            indexed,
            batch_compress,
            block_offsets: Vec::new(),
            cumulative_sizes,
            current_file_offset: initial_offset,
            compress_buf: Vec::new(),
            compressed_data: (0..stream_count).map(|_| Vec::new()).collect(),
            index_builder,
            pending_blocks: Vec::new(),
            batch_threshold,
            max_compress_threads,
        }
    }

    /// Force sequential (non-batched) compression. Required when a compressor
    /// captures per-base state that must be produced in stream order. Not
    /// compatible with indexed mode, which needs per-block batching.
    pub fn force_sequential(&mut self) {
        self.batch_compress = false;
    }

    /// Write data to a specific stream.
    pub fn write_to_stream(&mut self, stream_id: usize, data: &[u8]) -> io::Result<()> {
        self.buffers[stream_id].extend_from_slice(data);
        self.total_pending += data.len();
        if let Some(ref mut ib) = self.index_builder {
            ib.advance_stream(stream_id, data.len());
        }
        if self.total_pending >= self.block_budget {
            self.flush_block()?;
        }
        Ok(())
    }

    /// Force-flush a block (e.g., at end of file).
    pub fn flush_block(&mut self) -> io::Result<()> {
        if self.total_pending == 0 {
            return Ok(());
        }

        // Batched mode (indexed, or block-independent methods like BWT):
        // enqueue for parallel compression across (block, stream) pairs.
        if self.batch_compress {
            return self.enqueue_indexed_block();
        }

        // Non-indexed mode: compress and write immediately (streaming frames).
        let uncompressed_sizes: Vec<u64> = self.buffers.iter().map(|b| b.len() as u64).collect();

        let mut flags: u8 = 0;
        for s in 0..self.stream_count {
            self.compressed_data[s].clear();
            if self.buffers[s].is_empty() {
                flags |= 1 << s;
                continue;
            }
            if let Some(comp) = &mut self.compressors[s] {
                self.compress_buf.clear();
                comp.compress(&self.buffers[s], &mut self.compress_buf)?;
                // Non-indexed: keep frame open for zstd NbWorkers pipelining.
                // No raw fallback -- data is part of a streaming frame.
                std::mem::swap(&mut self.compressed_data[s], &mut self.compress_buf);
            } else {
                flags |= 1 << s;
                std::mem::swap(&mut self.compressed_data[s], &mut self.buffers[s]);
            }
        }

        // CRC over stored (compressed) data.
        let mut crc_val = 0u32;
        for s in 0..self.stream_count {
            if !self.compressed_data[s].is_empty() {
                crc_val = crc32c::crc32c_append(crc_val, &self.compressed_data[s]);
            }
        }

        // Write block header + data.
        let compressed_sizes: Vec<u64> = self
            .compressed_data
            .iter()
            .map(|c| c.len() as u64)
            .collect();
        let hdr = BlockHeader {
            flags,
            compressed_sizes: compressed_sizes.clone(),
            uncompressed_sizes,
            crc32: crc_val,
        };
        hdr.write_to(&mut self.output)?;

        for s in 0..self.stream_count {
            if !self.compressed_data[s].is_empty() {
                self.output.write_all(&self.compressed_data[s])?;
            }
        }

        let header_size = BlockHeader::header_size(self.stream_count);
        let data_size: usize = compressed_sizes.iter().map(|&s| s as usize).sum();
        self.current_file_offset += (header_size + data_size) as u64;

        for buf in &mut self.buffers {
            buf.clear();
        }
        self.total_pending = 0;

        Ok(())
    }

    /// Push current buffers into the pending batch (indexed mode only).
    fn enqueue_indexed_block(&mut self) -> io::Result<()> {
        let uncompressed_sizes: Vec<u64> = self.buffers.iter().map(|b| b.len() as u64).collect();

        // CRC over uncompressed data (indexed mode property).
        let mut crc_val = 0u32;
        for s in 0..self.stream_count {
            if !self.buffers[s].is_empty() {
                crc_val = crc32c::crc32c_append(crc_val, &self.buffers[s]);
            }
        }

        // Record cumulative sizes (needed by index regardless of when
        // the block is actually compressed and written).
        let mut cumulative_before = Vec::with_capacity(self.stream_count);
        for s in 0..self.stream_count {
            let prev = *self.cumulative_sizes[s].last().unwrap();
            cumulative_before.push(prev);
            self.cumulative_sizes[s].push(prev + uncompressed_sizes[s] as u64);
        }

        // Move buffers into the pending block.
        let block_buffers: Vec<Vec<u8>> = self.buffers.iter_mut().map(std::mem::take).collect();

        self.pending_blocks.push(PendingBlock {
            buffers: block_buffers,
            uncompressed_sizes,
            crc32: crc_val,
            cumulative_before,
        });

        self.total_pending = 0;

        if self.pending_blocks.len() >= self.batch_threshold {
            self.flush_pending()?;
        }

        Ok(())
    }

    /// Compress and write all pending blocks.
    fn flush_pending(&mut self) -> io::Result<()> {
        if self.pending_blocks.is_empty() {
            return Ok(());
        }

        // Use work-stealing parallelism when all methods are stateless
        // per-block (zstd, BWT with small blocks, rANS, or Markov in
        // indexed mode where state is reset per block anyway).
        // In non-indexed mode Markov preserves model state across blocks
        // for better compression, so it stays sequential.
        // Markov instances are ~2.3 GiB each, but max_compress_threads
        // already bounds concurrency to fit within the memory budget.
        let can_parallel = (0..self.stream_count).all(|s| {
            self.method_specs[s].is_none()
                || matches!(&self.method_specs[s], Some(MethodSpec::Zstd { .. }))
                || matches!(&self.method_specs[s], Some(MethodSpec::Xz { .. }))
                || matches!(&self.method_specs[s], Some(MethodSpec::Bwt { block_mib }) if *block_mib <= 16)
                || matches!(&self.method_specs[s], Some(MethodSpec::Rans))
                || (self.indexed && matches!(&self.method_specs[s], Some(MethodSpec::Markov { .. })))
        });

        if can_parallel {
            self.flush_pending_parallel()
        } else {
            self.flush_pending_sequential()
        }
    }

    /// Compress all pending blocks in parallel across (block, stream) pairs.
    fn flush_pending_parallel(&mut self) -> io::Result<()> {
        let sc = self.stream_count;

        // Build work items: (block_idx, stream_idx) for non-empty,
        // compressed streams.
        let work: Vec<(usize, usize)> = (0..self.pending_blocks.len())
            .flat_map(|bi| (0..sc).map(move |si| (bi, si)))
            .filter(|&(bi, si)| {
                !self.pending_blocks[bi].buffers[si].is_empty() && self.method_specs[si].is_some()
            })
            .collect();

        // Phase 1: Parallel compress.  Only immutable borrows of
        // pending_blocks and method_specs -- no &mut self needed.
        // Use a scoped thread pool bounded by available memory so we
        // don't OOM when methods are memory-hungry (e.g. large BWT).
        let pending = &self.pending_blocks;
        let specs = &self.method_specs;
        let n_threads = self.max_compress_threads;

        let compress_work = |bi: usize, si: usize| -> ((usize, usize), io::Result<Vec<u8>>) {
            let data = &pending[bi].buffers[si];
            let result = match &specs[si] {
                Some(MethodSpec::Zstd { level, window_log }) => {
                    zstd_compress_block(data, *level as i32, *window_log)
                }
                Some(MethodSpec::Xz {
                    preset,
                    extreme,
                    dict_size,
                    lc,
                    pb,
                }) => {
                    let params = XzParams {
                        preset: *preset,
                        extreme: *extreme,
                        dict_size: *dict_size,
                        lc: *lc,
                        pb: *pb,
                    };
                    xz_compress_block(data, &params)
                }
                Some(MethodSpec::Bwt { block_mib }) => {
                    // OMP threads = 1: Rayon owns the parallelism.
                    BwtCompressor::new(*block_mib, 1).and_then(|mut comp| {
                        let mut out = Vec::new();
                        comp.compress(data, &mut out)?;
                        Ok(out)
                    })
                }
                Some(MethodSpec::Rans) => {
                    let mut comp = RansCompressor::new();
                    let mut out = Vec::new();
                    comp.compress(data, &mut out)
                        .and_then(|_| comp.finish(&mut out))
                        .map(|_| out)
                }
                Some(MethodSpec::Markov { four_bit }) => {
                    let mut comp = MarkovCompressor::new(*four_bit);
                    let mut out = Vec::new();
                    comp.compress(data, &mut out)
                        .and_then(|_| comp.finish(&mut out))
                        .map(|_| out)
                }
                None => unreachable!(),
            };
            ((bi, si), result)
        };

        let results: Vec<((usize, usize), io::Result<Vec<u8>>)> = if n_threads < num_cpus::get() {
            // Build a smaller pool to stay within memory budget.
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(n_threads)
                .build()
                .map_err(|e| io::Error::other(e))?;
            pool.install(|| {
                work.par_iter()
                    .map(|&(bi, si)| compress_work(bi, si))
                    .collect()
            })
        } else {
            work.par_iter()
                .map(|&(bi, si)| compress_work(bi, si))
                .collect()
        };

        // Phase 2: Build per-block compressed data arrays.
        let num_blocks = self.pending_blocks.len();
        let mut compressed: Vec<Vec<Vec<u8>>> = (0..num_blocks)
            .map(|_| (0..sc).map(|_| Vec::new()).collect())
            .collect();
        let mut flags: Vec<u8> = vec![0; num_blocks];

        // Initialise empty and raw (no compressor) streams.
        for bi in 0..num_blocks {
            for si in 0..sc {
                if self.pending_blocks[bi].buffers[si].is_empty() {
                    flags[bi] |= 1 << si;
                } else if self.method_specs[si].is_none() {
                    flags[bi] |= 1 << si;
                    std::mem::swap(
                        &mut compressed[bi][si],
                        &mut self.pending_blocks[bi].buffers[si],
                    );
                }
            }
        }

        // Scatter compression results; apply raw fallback.
        for ((bi, si), result) in results {
            let c = result?;
            if c.len() >= self.pending_blocks[bi].buffers[si].len() {
                flags[bi] |= 1 << si;
                std::mem::swap(
                    &mut compressed[bi][si],
                    &mut self.pending_blocks[bi].buffers[si],
                );
            } else {
                compressed[bi][si] = c;
            }
        }

        // Phase 3: Write blocks sequentially (preserves ordering).
        for bi in 0..num_blocks {
            self.block_offsets.push(self.current_file_offset);
            if let Some(ref mut ib) = self.index_builder {
                ib.record_block(
                    self.current_file_offset,
                    self.pending_blocks[bi].cumulative_before.clone(),
                );
            }

            let compressed_sizes: Vec<u64> =
                compressed[bi].iter().map(|c| c.len() as u64).collect();
            // Indexed mode: CRC over uncompressed data (pre-computed).
            // Non-indexed batched mode: CRC over compressed/stored data.
            let crc = if self.indexed {
                self.pending_blocks[bi].crc32
            } else {
                let mut c = 0u32;
                for stream_data in &compressed[bi] {
                    if !stream_data.is_empty() {
                        c = crc32c::crc32c_append(c, stream_data);
                    }
                }
                c
            };
            let hdr = BlockHeader {
                flags: flags[bi],
                compressed_sizes: compressed_sizes.clone(),
                uncompressed_sizes: self.pending_blocks[bi].uncompressed_sizes.clone(),
                crc32: crc,
            };
            hdr.write_to(&mut self.output)?;

            for si in 0..sc {
                if !compressed[bi][si].is_empty() {
                    self.output.write_all(&compressed[bi][si])?;
                }
            }

            let header_size = BlockHeader::header_size(sc);
            let data_size: usize = compressed_sizes.iter().map(|&s| s as usize).sum();
            self.current_file_offset += (header_size + data_size) as u64;
        }

        self.pending_blocks.clear();
        Ok(())
    }

    /// Compress pending blocks one at a time using the stored compressor
    /// objects (for large BWT where OMP parallelism within a single block
    /// is valuable).
    fn flush_pending_sequential(&mut self) -> io::Result<()> {
        let sc = self.stream_count;

        for bi in 0..self.pending_blocks.len() {
            self.block_offsets.push(self.current_file_offset);
            if let Some(ref mut ib) = self.index_builder {
                ib.record_block(
                    self.current_file_offset,
                    self.pending_blocks[bi].cumulative_before.clone(),
                );
            }

            let mut flags: u8 = 0;
            let mut block_compressed: Vec<Vec<u8>> = (0..sc).map(|_| Vec::new()).collect();

            for s in 0..sc {
                if self.pending_blocks[bi].buffers[s].is_empty() {
                    flags |= 1 << s;
                    continue;
                }
                if let Some(comp) = &mut self.compressors[s] {
                    self.compress_buf.clear();
                    comp.compress(&self.pending_blocks[bi].buffers[s], &mut self.compress_buf)?;
                    comp.finish(&mut self.compress_buf)?;
                    // Markov preserves model state across blocks in
                    // non-indexed mode for better compression.  All other
                    // codecs (and indexed Markov) reset per block.
                    let keep_state = !self.indexed
                        && matches!(&self.method_specs[s], Some(MethodSpec::Markov { .. }));
                    if !keep_state {
                        comp.reset();
                    }
                    if self.compress_buf.len() >= self.pending_blocks[bi].buffers[s].len() {
                        flags |= 1 << s;
                        std::mem::swap(
                            &mut block_compressed[s],
                            &mut self.pending_blocks[bi].buffers[s],
                        );
                    } else {
                        std::mem::swap(&mut block_compressed[s], &mut self.compress_buf);
                    }
                } else {
                    flags |= 1 << s;
                    std::mem::swap(
                        &mut block_compressed[s],
                        &mut self.pending_blocks[bi].buffers[s],
                    );
                }
            }

            let compressed_sizes: Vec<u64> =
                block_compressed.iter().map(|c| c.len() as u64).collect();
            let crc = if self.indexed {
                self.pending_blocks[bi].crc32
            } else {
                let mut c = 0u32;
                for stream_data in &block_compressed {
                    if !stream_data.is_empty() {
                        c = crc32c::crc32c_append(c, stream_data);
                    }
                }
                c
            };
            let hdr = BlockHeader {
                flags,
                compressed_sizes: compressed_sizes.clone(),
                uncompressed_sizes: self.pending_blocks[bi].uncompressed_sizes.clone(),
                crc32: crc,
            };
            hdr.write_to(&mut self.output)?;

            for s in 0..sc {
                if !block_compressed[s].is_empty() {
                    self.output.write_all(&block_compressed[s])?;
                }
            }

            let header_size = BlockHeader::header_size(sc);
            let data_size: usize = compressed_sizes.iter().map(|&s| s as usize).sum();
            self.current_file_offset += (header_size + data_size) as u64;
        }

        self.pending_blocks.clear();
        Ok(())
    }

    /// Number of blocks written so far (including pending).
    pub fn block_count(&self) -> usize {
        self.block_offsets.len() + self.pending_blocks.len()
    }

    /// Get block offsets (for index building).
    pub fn block_offsets(&self) -> &[u64] {
        &self.block_offsets
    }

    /// Get cumulative sizes (for index building). [stream][block+1] format.
    pub fn cumulative_sizes(&self) -> &[Vec<u64>] {
        &self.cumulative_sizes
    }

    /// Current file offset (for trailer writing).
    pub fn current_file_offset(&self) -> u64 {
        self.current_file_offset
    }

    /// Delegate: begin a new sequence in the index builder.
    pub fn begin_sequence(&mut self, header: &[u8], nuc_bit_offset: u8) {
        if let Some(ref mut ib) = self.index_builder {
            ib.begin_sequence(header, nuc_bit_offset);
        }
    }

    /// Delegate: end the current sequence in the index builder.
    pub fn end_sequence(&mut self, total_bases: u64) {
        if let Some(ref mut ib) = self.index_builder {
            ib.end_sequence(total_bases);
        }
    }

    /// Delegate: track a case RLE byte in the index builder.
    pub fn track_case_rle(&mut self, rle_byte: u8, bases_decoded: u64) {
        if let Some(ref mut ib) = self.index_builder {
            ib.track_case_rle(rle_byte, bases_decoded);
        }
    }

    /// Finalize: flush remaining data. Returns the inner writer,
    /// the index builder (if indexing was enabled), and the final file offset.
    pub fn finalize(mut self) -> io::Result<(W, Option<IndexBuilder>, u64)> {
        self.flush_block()?;
        // Flush any remaining pending blocks (batched mode).
        self.flush_pending()?;

        // In non-indexed mode, finish all compressor frames and write a
        // trailing block with the remaining compressed data.
        if !self.indexed {
            let mut trailing: Vec<Vec<u8>> = Vec::with_capacity(self.stream_count);
            let mut has_trailing = false;
            let mut flags: u8 = 0;

            for s in 0..self.stream_count {
                if let Some(comp) = &mut self.compressors[s] {
                    let mut finish_buf = Vec::new();
                    comp.finish(&mut finish_buf)?;
                    if finish_buf.is_empty() {
                        flags |= 1 << s;
                    } else {
                        has_trailing = true;
                    }
                    trailing.push(finish_buf);
                } else {
                    flags |= 1 << s;
                    trailing.push(Vec::new());
                }
            }

            if has_trailing {
                let mut crc_val = 0u32;
                for cd in &trailing {
                    if !cd.is_empty() {
                        crc_val = crc32c::crc32c_append(crc_val, cd);
                    }
                }

                let compressed_sizes: Vec<u64> = trailing.iter().map(|c| c.len() as u64).collect();
                let uncompressed_sizes: Vec<u64> = vec![0; self.stream_count];

                let hdr = BlockHeader {
                    flags,
                    compressed_sizes: compressed_sizes.clone(),
                    uncompressed_sizes,
                    crc32: crc_val,
                };
                hdr.write_to(&mut self.output)?;

                for s in 0..self.stream_count {
                    if !trailing[s].is_empty() {
                        self.output.write_all(&trailing[s])?;
                    }
                }

                let header_size = BlockHeader::header_size(self.stream_count);
                let data_size: usize = compressed_sizes.iter().map(|&s| s as usize).sum();
                self.current_file_offset += (header_size + data_size) as u64;
            }
        }

        self.output.flush()?;
        let offset = self.current_file_offset;
        Ok((self.output, self.index_builder, offset))
    }
}
