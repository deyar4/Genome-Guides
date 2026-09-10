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

use super::traits::{Compressor, Decompressor};
use std::io;
use zstd::stream::raw::{CParameter, DParameter, Decoder, Encoder, InBuffer, Operation, OutBuffer};

/// Maximum zstd window log. 31 is the ceiling on 64-bit platforms
/// (ZSTD_WINDOWLOG_MAX_64). The decoder defaults to 27 for safety, which
/// rejects frames encoded with larger windows ("Frame requires too much
/// memory for decoding"). Raise it so any valid frame we produce decodes.
const ZSTD_WINDOW_LOG_MAX: u32 = 31;

/// Scratch buffer for zstd streaming I/O. 256 KiB balances syscall
/// overhead against memory usage for typical ~1 MiB blocks.
const SCRATCH_SIZE: usize = 256 * 1024;

pub struct ZstdCompressor {
    encoder: Encoder<'static>,
    scratch: Vec<u8>,
    level: i32,
    window_log: u32,
    n_threads: u32,
}

impl ZstdCompressor {
    pub fn new(level: i32, window_log: u32, n_threads: u32) -> Self {
        let mut encoder = Encoder::new(level).expect("zstd init");
        if window_log > 0 {
            encoder
                .set_parameter(CParameter::WindowLog(window_log))
                .expect("zstd windowlog");
        }
        if n_threads > 1 {
            encoder
                .set_parameter(CParameter::NbWorkers(n_threads))
                .expect("zstd nbworkers");
        }
        Self {
            encoder,
            scratch: vec![0u8; SCRATCH_SIZE],
            level,
            window_log,
            n_threads,
        }
    }
}

impl ZstdCompressor {
    /// Estimate memory for a single-threaded compressor at this level/window.
    /// Zstd internal state is dominated by the hash tables and match finder.
    fn estimate_single_thread(level: i32, window_log: u32, input_size: usize) -> usize {
        // Default window log by level (from zstd source ZSTD_defaultCLevel).
        let effective_wlog = if window_log > 0 {
            window_log
        } else {
            match level {
                ..=5 => 18,
                6..=9 => 20,
                10..=15 => 23,
                _ => 27,
            }
        };
        // Clamp to input size (zstd won't use more window than input).
        let bits = if input_size > 0 {
            (usize::BITS - input_size.leading_zeros()).min(effective_wlog)
        } else {
            0
        };
        let window = 1usize << bits;
        // Hash tables + chain tables + input/output buffers.
        // For levels <= 9: ~3x window.  For 10+: ~6x window.
        let multiplier = if level <= 9 { 3 } else { 6 };
        window * multiplier + SCRATCH_SIZE
    }

    /// Estimate peak memory for compressing `input_size` bytes, accounting
    /// for NbWorkers (each worker has its own window + hash tables).
    pub fn estimate_memory(
        level: i32,
        window_log: u32,
        n_threads: u32,
        input_size: usize,
    ) -> usize {
        let single = Self::estimate_single_thread(level, window_log, input_size);
        if n_threads <= 1 {
            single
        } else {
            // Each NbWorker has its own CCtx with window + tables.
            // Add per-worker I/O buffers (~input_size per job slot).
            single * n_threads as usize + input_size * n_threads as usize
        }
    }
}

impl Compressor for ZstdCompressor {
    fn compress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        // Feed input into the streaming encoder. In non-indexed mode the
        // frame stays open across calls so NbWorkers can pipeline jobs.
        // The caller must call finish() after the last block or reset()
        // between indexed blocks.
        let mut in_buf = InBuffer::around(input);
        while in_buf.pos() < in_buf.src.len() {
            let mut out_buf = OutBuffer::around(&mut self.scratch);
            self.encoder.run(&mut in_buf, &mut out_buf)?;
            if out_buf.pos() > 0 {
                output.extend_from_slice(out_buf.as_slice());
            }
        }
        Ok(())
    }

    fn finish(&mut self, output: &mut Vec<u8>) -> io::Result<()> {
        loop {
            let mut out_buf = OutBuffer::around(&mut self.scratch);
            let remaining = self.encoder.finish(&mut out_buf, true)?;
            if out_buf.pos() > 0 {
                output.extend_from_slice(out_buf.as_slice());
            }
            if remaining == 0 {
                break;
            }
        }
        self.encoder.reinit()?;
        Ok(())
    }

    fn reset(&mut self) {
        // End the current frame and reinit for next independent block.
        // We must drain any remaining output first, but in indexed mode
        // reset() is always called right after compress() + finish() in
        // flush_block, so there should be nothing left. Reinit is the
        // critical part -- it ensures no state leaks between blocks.
        let _ = self.encoder.reinit();
    }

    fn compress_memory(&self, input_size: usize) -> usize {
        ZstdCompressor::estimate_memory(self.level, self.window_log, self.n_threads, input_size)
    }
}

/// Compress a complete block in one shot. Thread-safe, no shared state.
/// Used by Rayon parallel compression in indexed mode.
pub fn zstd_compress_block(input: &[u8], level: i32, window_log: u32) -> io::Result<Vec<u8>> {
    let mut enc = Encoder::new(level)?;
    if window_log > 0 {
        enc.set_parameter(CParameter::WindowLog(window_log))?;
    }
    // NbWorkers=0: single-threaded. Rayon handles inter-stream parallelism.
    let mut output = Vec::new();
    let mut scratch = vec![0u8; SCRATCH_SIZE];
    let mut in_buf = InBuffer::around(input);
    while in_buf.pos() < in_buf.src.len() {
        let mut out_buf = OutBuffer::around(&mut scratch);
        enc.run(&mut in_buf, &mut out_buf)?;
        if out_buf.pos() > 0 {
            output.extend_from_slice(out_buf.as_slice());
        }
    }
    loop {
        let mut out_buf = OutBuffer::around(&mut scratch);
        let remaining = enc.finish(&mut out_buf, true)?;
        if out_buf.pos() > 0 {
            output.extend_from_slice(out_buf.as_slice());
        }
        if remaining == 0 {
            break;
        }
    }
    Ok(output)
}

pub struct ZstdDecompressor {
    decoder: Decoder<'static>,
    scratch: Vec<u8>,
}

impl Default for ZstdDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl ZstdDecompressor {
    pub fn new() -> Self {
        let mut decoder = Decoder::new().expect("zstd decoder init");
        decoder
            .set_parameter(DParameter::WindowLogMax(ZSTD_WINDOW_LOG_MAX))
            .expect("zstd windowlogmax");
        Self {
            decoder,
            scratch: vec![0u8; SCRATCH_SIZE],
        }
    }
}

impl Decompressor for ZstdDecompressor {
    fn decompress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        let mut in_buf = InBuffer::around(input);
        loop {
            let mut out_buf = OutBuffer::around(&mut self.scratch);
            self.decoder.run(&mut in_buf, &mut out_buf)?;
            if out_buf.pos() > 0 {
                output.extend_from_slice(out_buf.as_slice());
            }
            if in_buf.pos() >= in_buf.src.len() && out_buf.pos() == 0 {
                break;
            }
        }
        Ok(())
    }

    fn reset(&mut self) {
        let _ = self.decoder.reinit();
    }

    fn decompress_memory(&self, output_size: usize) -> usize {
        // Decompression needs the output buffer + scratch + decoder tables.
        // Zstd decompression is much cheaper than compression (~128 KiB + output).
        output_size + SCRATCH_SIZE + (128 * 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_round_trip_levels() {
        let data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
        for level in [1, 3, 9, 19] {
            let mut comp = ZstdCompressor::new(level, 0, 1);
            let mut compressed = Vec::new();
            comp.compress(&data, &mut compressed).unwrap();
            comp.finish(&mut compressed).unwrap();
            assert!(!compressed.is_empty());

            let mut decomp = ZstdDecompressor::new();
            let mut result = Vec::new();
            decomp.decompress(&compressed, &mut result).unwrap();
            assert_eq!(result, data, "failed at level {}", level);
        }
    }

    #[test]
    fn test_zstd_empty_input() {
        let mut comp = ZstdCompressor::new(3, 0, 1);
        let mut compressed = Vec::new();
        comp.compress(&[], &mut compressed).unwrap();
        assert!(compressed.is_empty());
    }

    #[test]
    fn test_zstd_reset() {
        let mut comp = ZstdCompressor::new(3, 0, 1);

        let data_a = b"AAAAAAAAAA";
        let mut comp_a = Vec::new();
        comp.compress(data_a, &mut comp_a).unwrap();
        comp.finish(&mut comp_a).unwrap();

        comp.reset();

        let data_b = b"BBBBBBBBBB";
        let mut comp_b = Vec::new();
        comp.compress(data_b, &mut comp_b).unwrap();
        comp.finish(&mut comp_b).unwrap();

        let mut decomp = ZstdDecompressor::new();
        let mut result_a = Vec::new();
        decomp.decompress(&comp_a, &mut result_a).unwrap();
        assert_eq!(result_a, data_a);

        let mut result_b = Vec::new();
        decomp.decompress(&comp_b, &mut result_b).unwrap();
        assert_eq!(result_b, data_b);
    }

    /// Simulate indexed mode: compress many blocks with reset() between each,
    /// then verify every block is independently decompressible.
    #[test]
    fn test_zstd_indexed_blocks_independent() {
        let mut comp = ZstdCompressor::new(3, 0, 1);
        // Generate distinct block data that shares patterns
        // (so a broken reset could cause cross-block contamination)
        let blocks: Vec<Vec<u8>> = (0..10)
            .map(|i| {
                let mut v = Vec::with_capacity(8192);
                for j in 0..8192 {
                    v.push(((j * 7 + i * 1337) & 0xFF) as u8);
                }
                v
            })
            .collect();

        // Compress each block with finish+reset between them (indexed mode pattern)
        let mut compressed_blocks: Vec<Vec<u8>> = Vec::new();
        for block in &blocks {
            let mut compressed = Vec::new();
            comp.compress(block, &mut compressed).unwrap();
            comp.finish(&mut compressed).unwrap();
            comp.reset();
            compressed_blocks.push(compressed);
        }

        // Each compressed block must decompress independently to the correct data
        for (i, (compressed, original)) in compressed_blocks.iter().zip(blocks.iter()).enumerate() {
            let mut result = Vec::new();
            // Use a FRESH decompressor for each block -- if the frame isn't
            // self-contained, this will fail.
            let mut fresh_decomp = ZstdDecompressor::new();
            fresh_decomp
                .decompress(compressed, &mut result)
                .unwrap_or_else(|e| panic!("block {} decompress failed: {}", i, e));
            assert_eq!(
                result, *original,
                "block {} content mismatch (reset not working)",
                i
            );
        }

        // Also verify: compressing the same data twice after reset gives
        // identical output (no state leakage)
        let test_data: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();
        let mut out1 = Vec::new();
        comp.compress(&test_data, &mut out1).unwrap();
        comp.finish(&mut out1).unwrap();
        comp.reset();
        let mut out2 = Vec::new();
        comp.compress(&test_data, &mut out2).unwrap();
        comp.finish(&mut out2).unwrap();
        assert_eq!(
            out1, out2,
            "reset should produce identical output for identical input"
        );
    }

    #[test]
    fn test_zstd_incompressible() {
        // Pseudo-random data shouldn't compress well
        let mut data = Vec::with_capacity(10000);
        let mut x: u32 = 42;
        for _ in 0..10000 {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((x >> 16) as u8);
        }

        let mut comp = ZstdCompressor::new(1, 0, 1);
        let mut compressed = Vec::new();
        comp.compress(&data, &mut compressed).unwrap();
        comp.finish(&mut compressed).unwrap();
        // Compressed should be at most slightly larger (zstd overhead)
        assert!(compressed.len() <= data.len() + 100);

        let mut decomp = ZstdDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_zstd_genomic_data() {
        // Genomic-like repetitive data should compress well
        let mut data = Vec::new();
        let bases = b"ACGTACGTACGTNNNN";
        for _ in 0..1000 {
            data.extend_from_slice(bases);
        }

        let mut comp = ZstdCompressor::new(3, 0, 1);
        let mut compressed = Vec::new();
        comp.compress(&data, &mut compressed).unwrap();
        comp.finish(&mut compressed).unwrap();
        assert!(compressed.len() < data.len() / 2);

        let mut decomp = ZstdDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_zstd_multithread() {
        let data: Vec<u8> = vec![b'A'; 4 * 1024 * 1024]; // 4 MiB
        let mut comp = ZstdCompressor::new(3, 0, 4);
        let mut compressed = Vec::new();
        comp.compress(&data, &mut compressed).unwrap();
        comp.finish(&mut compressed).unwrap();

        let mut decomp = ZstdDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_zstd_window_log() {
        let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let mut comp = ZstdCompressor::new(3, 20, 1); // windowLog=20 (1 MiB)
        let mut compressed = Vec::new();
        comp.compress(&data, &mut compressed).unwrap();
        comp.finish(&mut compressed).unwrap();

        let mut decomp = ZstdDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    /// Regression: frames encoded with a window log above zstd's default
    /// decoder ceiling (27) must still decode; the decoder's WindowLogMax
    /// is raised to 31 in ZstdDecompressor::new.
    #[test]
    fn test_zstd_high_window_log_decodes() {
        let data: Vec<u8> = (0..64 * 1024).map(|i| (i % 256) as u8).collect();
        let compressed = zstd_compress_block(&data, 22, 31).unwrap();

        let mut decomp = ZstdDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }
}
