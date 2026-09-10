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

use std::io;

/// Block compressor: compress a chunk of data.
pub trait Compressor {
    /// Compress `input` and append compressed data to `output`.
    /// In streaming mode (non-indexed), the compressor may buffer data
    /// internally and produce partial output. Call `finish()` to flush
    /// all remaining compressed data.
    fn compress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()>;

    /// Finish the current frame and flush all remaining compressed data.
    /// After this call, `output` contains all remaining bytes and the
    /// compressor is ready for a new frame. Must be called before the
    /// final block is written.
    fn finish(&mut self, output: &mut Vec<u8>) -> io::Result<()>;

    /// Reset internal state for the next independent block (indexed mode).
    /// Does NOT flush remaining data -- call `finish()` first to produce
    /// the complete frame. This aborts any in-progress frame and reinits
    /// the compressor for a completely independent next block.
    fn reset(&mut self);

    /// Peak memory usage (bytes) for compressing one block of `input_size`.
    /// Used by the mux writer to bound parallelism and avoid OOM.
    fn compress_memory(&self, input_size: usize) -> usize {
        // Conservative default: assume we need at least input + output buffers.
        input_size * 2
    }
}

/// Block decompressor: decompress a chunk of data.
pub trait Decompressor {
    /// Decompress `input` into `output`, which must be pre-sized to
    /// the expected uncompressed size.
    fn decompress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()>;

    /// Reset internal state for the next independent block.
    fn reset(&mut self);

    /// Peak memory usage (bytes) for decompressing one block of
    /// `output_size` uncompressed bytes.
    fn decompress_memory(&self, output_size: usize) -> usize {
        output_size * 2
    }
}

/// Identifies which compression method to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMethod {
    Zstd { level: i32, window_log: u32 },
    Bwt { block_mib: u16 },
    Markov { four_bit: bool },
    Bmarkov { four_bit: bool },
    Vmarkov { four_bit: bool },
    Rans,
    Raw,
}
