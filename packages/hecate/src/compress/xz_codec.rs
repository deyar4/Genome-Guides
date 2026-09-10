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
use liblzma::stream::{
    Action, Check, Filters, LzmaOptions, MtStreamBuilder, Status, Stream, PRESET_EXTREME,
};
use std::io;

const SCRATCH_SIZE: usize = 256 * 1024;

/// xz/LZMA2 compressor parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XzParams {
    /// Preset level 0-9.
    pub preset: u8,
    /// Slower "extreme" variant (xz -e).
    pub extreme: bool,
    /// Dictionary size in bytes (0 = use preset default).
    pub dict_size: u32,
    /// Literal context bits 0-4 (0xFF = use preset default).
    pub lc: u8,
    /// Position bits 0-4 (0xFF = use preset default).
    pub pb: u8,
}

impl XzParams {
    pub fn new_preset(preset: u8, extreme: bool) -> Self {
        Self {
            preset,
            extreme,
            dict_size: 0,
            lc: 0xFF,
            pb: 0xFF,
        }
    }

    fn preset_value(&self) -> u32 {
        let p = (self.preset as u32) & 0x0F;
        if self.extreme {
            p | PRESET_EXTREME
        } else {
            p
        }
    }

    fn has_overrides(&self) -> bool {
        self.dict_size != 0 || self.lc != 0xFF || self.pb != 0xFF
    }
}

fn xz_err(e: liblzma::stream::Error) -> io::Error {
    io::Error::other(format!("xz: {:?}", e))
}

fn make_filters(params: &XzParams) -> io::Result<Filters> {
    let mut opts = LzmaOptions::new_preset(params.preset_value()).map_err(xz_err)?;
    if params.dict_size != 0 {
        opts.dict_size(params.dict_size);
    }
    if params.lc != 0xFF {
        opts.literal_context_bits(params.lc as u32);
    }
    if params.pb != 0xFF {
        opts.position_bits(params.pb as u32);
    }
    let mut filters = Filters::new();
    filters.lzma2(&opts);
    Ok(filters)
}

fn build_encoder(params: &XzParams, n_threads: u32) -> io::Result<Stream> {
    if n_threads > 1 {
        let mut b = MtStreamBuilder::new();
        b.threads(n_threads).check(Check::Crc64);
        if params.has_overrides() {
            b.filters(make_filters(params)?);
        } else {
            b.preset(params.preset_value());
        }
        b.encoder().map_err(xz_err)
    } else if params.has_overrides() {
        Stream::new_stream_encoder(&make_filters(params)?, Check::Crc64).map_err(xz_err)
    } else {
        Stream::new_easy_encoder(params.preset_value(), Check::Crc64).map_err(xz_err)
    }
}

pub struct XzCompressor {
    params: XzParams,
    n_threads: u32,
    stream: Stream,
    scratch: Vec<u8>,
}

impl XzCompressor {
    pub fn new(params: XzParams, n_threads: u32) -> Self {
        let stream = build_encoder(&params, n_threads).expect("xz encoder init");
        Self {
            params,
            n_threads,
            stream,
            scratch: vec![0u8; SCRATCH_SIZE],
        }
    }

    /// Estimate single-threaded encoder memory at this preset level.
    pub fn estimate_single_thread(params: &XzParams) -> usize {
        let bytes = unsafe { liblzma_sys::lzma_easy_encoder_memusage(params.preset_value()) };
        if bytes == u64::MAX {
            // liblzma returns UINT64_MAX on invalid preset; fall back.
            128 * 1024 * 1024
        } else {
            bytes as usize + SCRATCH_SIZE
        }
    }

    /// Estimate peak memory for compressing `input_size` bytes with the given
    /// thread count.  Each MT worker has its own LZMA2 dict + state.
    pub fn estimate_memory(params: &XzParams, n_threads: u32, input_size: usize) -> usize {
        let single = Self::estimate_single_thread(params);
        if n_threads <= 1 {
            single
        } else {
            // MT mode: ~3 * block_size per worker plus the encoder state.
            // Use input_size as an upper bound on block_size.
            single * n_threads as usize + input_size * n_threads as usize
        }
    }
}

impl Compressor for XzCompressor {
    fn compress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        let mut consumed = 0;
        while consumed < input.len() {
            let before_in = self.stream.total_in();
            let before_out = self.stream.total_out();
            self.stream
                .process(&input[consumed..], &mut self.scratch, Action::Run)
                .map_err(xz_err)?;
            let in_delta = (self.stream.total_in() - before_in) as usize;
            let out_delta = (self.stream.total_out() - before_out) as usize;
            consumed += in_delta;
            if out_delta > 0 {
                output.extend_from_slice(&self.scratch[..out_delta]);
            }
            if in_delta == 0 && out_delta == 0 {
                return Err(io::Error::other("xz: encoder stalled"));
            }
        }
        Ok(())
    }

    fn finish(&mut self, output: &mut Vec<u8>) -> io::Result<()> {
        loop {
            let before_out = self.stream.total_out();
            let status = self
                .stream
                .process(&[], &mut self.scratch, Action::Finish)
                .map_err(xz_err)?;
            let out_delta = (self.stream.total_out() - before_out) as usize;
            if out_delta > 0 {
                output.extend_from_slice(&self.scratch[..out_delta]);
            }
            if status == Status::StreamEnd {
                break;
            }
            if out_delta == 0 {
                return Err(io::Error::other("xz: finish stalled"));
            }
        }
        // Reinit for next frame.
        self.stream = build_encoder(&self.params, self.n_threads)?;
        Ok(())
    }

    fn reset(&mut self) {
        if let Ok(s) = build_encoder(&self.params, self.n_threads) {
            self.stream = s;
        }
    }

    fn compress_memory(&self, input_size: usize) -> usize {
        XzCompressor::estimate_memory(&self.params, self.n_threads, input_size)
    }
}

/// Compress a complete block in one shot.  Thread-safe; used by Rayon
/// parallel compression in indexed mode.
pub fn xz_compress_block(input: &[u8], params: &XzParams) -> io::Result<Vec<u8>> {
    let mut stream = build_encoder(params, 1)?;
    let mut output = Vec::new();
    let mut scratch = vec![0u8; SCRATCH_SIZE];

    let mut consumed = 0;
    while consumed < input.len() {
        let before_in = stream.total_in();
        let before_out = stream.total_out();
        stream
            .process(&input[consumed..], &mut scratch, Action::Run)
            .map_err(xz_err)?;
        let in_delta = (stream.total_in() - before_in) as usize;
        let out_delta = (stream.total_out() - before_out) as usize;
        consumed += in_delta;
        if out_delta > 0 {
            output.extend_from_slice(&scratch[..out_delta]);
        }
        if in_delta == 0 && out_delta == 0 {
            return Err(io::Error::other("xz: encoder stalled"));
        }
    }
    loop {
        let before_out = stream.total_out();
        let status = stream
            .process(&[], &mut scratch, Action::Finish)
            .map_err(xz_err)?;
        let out_delta = (stream.total_out() - before_out) as usize;
        if out_delta > 0 {
            output.extend_from_slice(&scratch[..out_delta]);
        }
        if status == Status::StreamEnd {
            break;
        }
        if out_delta == 0 {
            return Err(io::Error::other("xz: finish stalled"));
        }
    }
    Ok(output)
}

pub struct XzDecompressor {
    stream: Stream,
    scratch: Vec<u8>,
}

impl Default for XzDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl XzDecompressor {
    pub fn new() -> Self {
        let stream = Stream::new_stream_decoder(u64::MAX, 0).expect("xz decoder init");
        Self {
            stream,
            scratch: vec![0u8; SCRATCH_SIZE],
        }
    }
}

impl Decompressor for XzDecompressor {
    fn decompress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        let mut consumed = 0;
        loop {
            let before_in = self.stream.total_in();
            let before_out = self.stream.total_out();
            let status = self
                .stream
                .process(&input[consumed..], &mut self.scratch, Action::Run)
                .map_err(xz_err)?;
            let in_delta = (self.stream.total_in() - before_in) as usize;
            let out_delta = (self.stream.total_out() - before_out) as usize;
            consumed += in_delta;
            if out_delta > 0 {
                output.extend_from_slice(&self.scratch[..out_delta]);
            }
            if status == Status::StreamEnd {
                break;
            }
            if consumed >= input.len() && out_delta == 0 {
                break;
            }
            if in_delta == 0 && out_delta == 0 {
                return Err(io::Error::other("xz: decoder stalled"));
            }
        }
        Ok(())
    }

    fn reset(&mut self) {
        if let Ok(s) = Stream::new_stream_decoder(u64::MAX, 0) {
            self.stream = s;
        }
    }

    fn decompress_memory(&self, output_size: usize) -> usize {
        // xz decompression dominated by the dict.  Be generous: 64 MiB cap
        // matches the default --memlimit-decompress on most systems.
        output_size + SCRATCH_SIZE + 64 * 1024 * 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xz_round_trip_presets() {
        let data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
        for preset in [0u8, 3, 6, 9] {
            for extreme in [false, true] {
                let params = XzParams::new_preset(preset, extreme);
                let mut comp = XzCompressor::new(params, 1);
                let mut compressed = Vec::new();
                comp.compress(&data, &mut compressed).unwrap();
                comp.finish(&mut compressed).unwrap();
                assert!(!compressed.is_empty());

                let mut decomp = XzDecompressor::new();
                let mut result = Vec::new();
                decomp.decompress(&compressed, &mut result).unwrap();
                assert_eq!(result, data, "preset {} extreme {}", preset, extreme);
            }
        }
    }

    #[test]
    fn test_xz_empty_input() {
        let mut comp = XzCompressor::new(XzParams::new_preset(3, false), 1);
        let mut compressed = Vec::new();
        comp.compress(&[], &mut compressed).unwrap();
        assert!(compressed.is_empty());
    }

    #[test]
    fn test_xz_reset() {
        let mut comp = XzCompressor::new(XzParams::new_preset(3, false), 1);

        let data_a = b"AAAAAAAAAA";
        let mut comp_a = Vec::new();
        comp.compress(data_a, &mut comp_a).unwrap();
        comp.finish(&mut comp_a).unwrap();

        comp.reset();

        let data_b = b"BBBBBBBBBB";
        let mut comp_b = Vec::new();
        comp.compress(data_b, &mut comp_b).unwrap();
        comp.finish(&mut comp_b).unwrap();

        let mut decomp = XzDecompressor::new();
        let mut result_a = Vec::new();
        decomp.decompress(&comp_a, &mut result_a).unwrap();
        assert_eq!(result_a, data_a);

        decomp.reset();
        let mut result_b = Vec::new();
        decomp.decompress(&comp_b, &mut result_b).unwrap();
        assert_eq!(result_b, data_b);
    }

    #[test]
    fn test_xz_one_shot_block() {
        let data: Vec<u8> = (0..50_000).map(|i| (i % 256) as u8).collect();
        let params = XzParams::new_preset(6, false);
        let compressed = xz_compress_block(&data, &params).unwrap();

        let mut decomp = XzDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_xz_custom_params() {
        let data: Vec<u8> = (0..50_000).map(|i| (i % 256) as u8).collect();
        let params = XzParams {
            preset: 9,
            extreme: true,
            dict_size: 1 << 24, // 16 MiB
            lc: 4,
            pb: 0,
        };
        let mut comp = XzCompressor::new(params, 1);
        let mut compressed = Vec::new();
        comp.compress(&data, &mut compressed).unwrap();
        comp.finish(&mut compressed).unwrap();

        let mut decomp = XzDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_xz_genomic_data() {
        let mut data = Vec::new();
        let bases = b"ACGTACGTACGTNNNN";
        for _ in 0..1000 {
            data.extend_from_slice(bases);
        }

        let params = XzParams::new_preset(6, false);
        let mut comp = XzCompressor::new(params, 1);
        let mut compressed = Vec::new();
        comp.compress(&data, &mut compressed).unwrap();
        comp.finish(&mut compressed).unwrap();
        assert!(compressed.len() < data.len() / 2);

        let mut decomp = XzDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_xz_multithread() {
        let data: Vec<u8> = vec![b'A'; 4 * 1024 * 1024];
        let params = XzParams::new_preset(3, false);
        let mut comp = XzCompressor::new(params, 4);
        let mut compressed = Vec::new();
        comp.compress(&data, &mut compressed).unwrap();
        comp.finish(&mut compressed).unwrap();

        let mut decomp = XzDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_xz_indexed_blocks_independent() {
        let mut comp = XzCompressor::new(XzParams::new_preset(3, false), 1);
        let blocks: Vec<Vec<u8>> = (0..5)
            .map(|i| {
                let mut v = Vec::with_capacity(8192);
                for j in 0..8192 {
                    v.push(((j * 7 + i * 1337) & 0xFF) as u8);
                }
                v
            })
            .collect();

        let mut compressed_blocks: Vec<Vec<u8>> = Vec::new();
        for block in &blocks {
            let mut compressed = Vec::new();
            comp.compress(block, &mut compressed).unwrap();
            comp.finish(&mut compressed).unwrap();
            comp.reset();
            compressed_blocks.push(compressed);
        }

        for (i, (compressed, original)) in compressed_blocks.iter().zip(blocks.iter()).enumerate() {
            let mut result = Vec::new();
            let mut fresh_decomp = XzDecompressor::new();
            fresh_decomp
                .decompress(compressed, &mut result)
                .unwrap_or_else(|e| panic!("block {} decompress failed: {}", i, e));
            assert_eq!(result, *original, "block {} content mismatch", i);
        }
    }
}
