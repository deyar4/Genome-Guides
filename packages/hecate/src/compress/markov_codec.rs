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

// Nucleotide codec. Each stream selects one of three model machines by the
// size of its first block (genome scale correlates with block size):
//
//   len <  200 KiB              -> vmarkov  (5-mixture brute force)
//   200 KiB <= len < 3/4*budget -> bmarkov  (bacterial-tuned block codec)
//   len >= 3/4*budget           -> markov   (general block codec)
//
// where `budget` = DEFAULT_BLOCK_BUDGET (1 MiB). The choice is a one-time
// decision per stream (no per-symbol cost) and is recorded in a single tag
// byte per block so the decoder can route each block independently. Framing
// (256 MiB frame loop, 2-/4-bit block packing, range coder) is shared; the
// three model machines are instantiated lazily and persist across the blocks
// they handle.

use super::traits::{Compressor, Decompressor};
use crate::markov::block_codec::{BlockCodec as MarkovBlock, BLOCK_SIZE};
use crate::markov::bmarkov::BlockCodec as BmarkovBlock;
use crate::markov::rangecoder::{RangeDecoder, RangeEncoder};
use crate::markov::vmarkov::{
    decode_with, encode_with, V0BlockCodec, V1BlockCodec, V2BlockCodec, V3BlockCodec, V4BlockCodec,
    N_MIXTURES,
};
use crate::mux::writer::DEFAULT_BLOCK_BUDGET;
use std::io;
use std::sync::{Arc, Mutex};

/// Shared destination for per-base model probabilities (quantised: `q/100000`).
pub type ProbSink = Arc<Mutex<Vec<u32>>>;

const MAX_DECOMPRESS: usize = 256 << 20;

const TAG_MARKOV: u8 = 0;
const TAG_BMARKOV: u8 = 1;
const TAG_VMARKOV: u8 = 2;

const VMARKOV_MAX: usize = 200_000;

#[inline]
fn bmarkov_max() -> usize {
    DEFAULT_BLOCK_BUDGET * 3 / 4
}

#[inline]
fn dispatch(len: usize) -> u8 {
    if len < VMARKOV_MAX {
        TAG_VMARKOV
    } else if len < bmarkov_max() {
        TAG_BMARKOV
    } else {
        TAG_MARKOV
    }
}

// --- Framing shared by the single-mixture block codecs ---------------------
// The markov and bmarkov block codecs expose the same block interface; one
// frame/pack/range-coder loop drives either through this trait.

trait NucBlockCodec {
    fn encode_block(&mut self, block: &mut [u8; BLOCK_SIZE], four_bit: bool, enc: &mut RangeEncoder);
    fn decode_block(&mut self, four_bit: bool, dec: &mut RangeDecoder, out: &mut [u8; BLOCK_SIZE]);
}

impl NucBlockCodec for MarkovBlock {
    #[inline]
    fn encode_block(
        &mut self,
        block: &mut [u8; BLOCK_SIZE],
        four_bit: bool,
        enc: &mut RangeEncoder,
    ) {
        MarkovBlock::encode_block(self, block, four_bit, enc)
    }
    #[inline]
    fn decode_block(&mut self, four_bit: bool, dec: &mut RangeDecoder, out: &mut [u8; BLOCK_SIZE]) {
        MarkovBlock::decode_block(self, four_bit, dec, out)
    }
}

impl NucBlockCodec for BmarkovBlock {
    #[inline]
    fn encode_block(
        &mut self,
        block: &mut [u8; BLOCK_SIZE],
        four_bit: bool,
        enc: &mut RangeEncoder,
    ) {
        BmarkovBlock::encode_block(self, block, four_bit, enc)
    }
    #[inline]
    fn decode_block(&mut self, four_bit: bool, dec: &mut RangeDecoder, out: &mut [u8; BLOCK_SIZE]) {
        BmarkovBlock::decode_block(self, four_bit, dec, out)
    }
}

/// Frame + pack + range-code a block of nucleotides with a single-mixture codec.
/// Emits one `orig_size(8) | comp_size(8) | data` frame per <=256 MiB chunk.
fn encode_nuc<C: NucBlockCodec>(
    codec: &mut C,
    input: &[u8],
    four_bit: bool,
    output: &mut Vec<u8>,
    mut probs: Option<&mut Vec<u32>>,
) {
    for frame in input.chunks(MAX_DECOMPRESS) {
        let mut enc = RangeEncoder::with_capacity(frame.len());
        if probs.is_some() {
            enc.enable_prob_log();
        }
        if four_bit {
            let ppb = BLOCK_SIZE / 2;
            let mut off = 0;
            while off < frame.len() {
                let mut block = [0u8; BLOCK_SIZE];
                let chunk = &frame[off..(off + ppb).min(frame.len())];
                for (j, &b) in chunk.iter().enumerate() {
                    block[j * 2] = b & 0x0F;
                    block[j * 2 + 1] = (b >> 4) & 0x0F;
                }
                codec.encode_block(&mut block, true, &mut enc);
                off += ppb;
            }
        } else {
            let ppb = BLOCK_SIZE / 4;
            let mut off = 0;
            while off < frame.len() {
                let mut block = [0u8; BLOCK_SIZE];
                let chunk = &frame[off..(off + ppb).min(frame.len())];
                for (j, &b) in chunk.iter().enumerate() {
                    block[j * 4] = b & 0x03;
                    block[j * 4 + 1] = (b >> 2) & 0x03;
                    block[j * 4 + 2] = (b >> 4) & 0x03;
                    block[j * 4 + 3] = (b >> 6) & 0x03;
                }
                codec.encode_block(&mut block, false, &mut enc);
                off += ppb;
            }
        }
        enc.finish();
        if let Some(fp) = enc.take_prob_log() {
            if let Some(pl) = probs.as_deref_mut() {
                // Drop padding symbols in the last (short) block.
                let real = frame.len() * if four_bit { 2 } else { 4 };
                pl.extend_from_slice(&fp[..real.min(fp.len())]);
            }
        }
        let compressed = enc.into_output();
        output.extend_from_slice(&(frame.len() as u64).to_le_bytes());
        output.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
        output.extend_from_slice(&compressed);
    }
}

fn decode_nuc<C: NucBlockCodec>(
    codec: &mut C,
    input: &[u8],
    four_bit: bool,
    output: &mut Vec<u8>,
) -> io::Result<()> {
    let mut pos = 0;
    while pos < input.len() {
        if pos + 16 > input.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated markov frame header",
            ));
        }
        let orig_size = u64::from_le_bytes(input[pos..pos + 8].try_into().unwrap()) as usize;
        pos += 8;
        if orig_size > MAX_DECOMPRESS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "markov frame size exceeds limit",
            ));
        }
        let comp_size = u64::from_le_bytes(input[pos..pos + 8].try_into().unwrap()) as usize;
        pos += 8;
        if pos + comp_size > input.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated markov compressed data",
            ));
        }
        let mut dec = RangeDecoder::new(&input[pos..pos + comp_size]);
        pos += comp_size;
        let mut decoded = 0usize;
        if four_bit {
            let ppb = BLOCK_SIZE / 2;
            while decoded < orig_size {
                let mut block = [0u8; BLOCK_SIZE];
                codec.decode_block(true, &mut dec, &mut block);
                let pack = ppb.min(orig_size - decoded);
                for j in 0..pack {
                    output.push((block[j * 2] & 0x0F) | ((block[j * 2 + 1] & 0x0F) << 4));
                }
                decoded += ppb;
            }
        } else {
            let ppb = BLOCK_SIZE / 4;
            while decoded < orig_size {
                let mut block = [0u8; BLOCK_SIZE];
                codec.decode_block(false, &mut dec, &mut block);
                let pack = ppb.min(orig_size - decoded);
                for j in 0..pack {
                    output.push(
                        (block[j * 4] & 0x03)
                            | ((block[j * 4 + 1] & 0x03) << 2)
                            | ((block[j * 4 + 2] & 0x03) << 4)
                            | ((block[j * 4 + 3] & 0x03) << 6),
                    );
                }
                decoded += ppb;
            }
        }
    }
    Ok(())
}

// --- vmarkov mixture set ---------------------------------------------------

struct VmarkovSet {
    v0: V0BlockCodec,
    v1: V1BlockCodec,
    v2: V2BlockCodec,
    v3: V3BlockCodec,
    v4: V4BlockCodec,
}

impl VmarkovSet {
    fn new() -> Self {
        Self {
            v0: V0BlockCodec::new(),
            v1: V1BlockCodec::new(),
            v2: V2BlockCodec::new(),
            v3: V3BlockCodec::new(),
            v4: V4BlockCodec::new(),
        }
    }

    fn reset_all(&mut self) {
        use crate::markov::vmarkov::MixtureBlockCodec;
        self.v0.reset();
        self.v1.reset();
        self.v2.reset();
        self.v3.reset();
        self.v4.reset();
    }

    /// Brute-force all five mixtures on `input`; append the winner as
    /// `mixture_idx(1) | orig_size(8) | frame_size(8) | frame` per <=256 MiB chunk.
    fn encode(
        &mut self,
        input: &[u8],
        four_bit: bool,
        output: &mut Vec<u8>,
        mut probs: Option<&mut Vec<u32>>,
    ) {
        let cap = probs.is_some();
        for chunk in input.chunks(MAX_DECOMPRESS) {
            self.reset_all();
            let (f0, p0) = encode_with(&mut self.v0, chunk, four_bit, cap);
            let (f1, p1) = encode_with(&mut self.v1, chunk, four_bit, cap);
            let (f2, p2) = encode_with(&mut self.v2, chunk, four_bit, cap);
            let (f3, p3) = encode_with(&mut self.v3, chunk, four_bit, cap);
            let (f4, p4) = encode_with(&mut self.v4, chunk, four_bit, cap);
            let frames: [&Vec<u8>; N_MIXTURES] = [&f0, &f1, &f2, &f3, &f4];
            let mut best = 0usize;
            for i in 1..N_MIXTURES {
                if frames[i].len() < frames[best].len() {
                    best = i;
                }
            }
            output.push(best as u8);
            output.extend_from_slice(&(chunk.len() as u64).to_le_bytes());
            output.extend_from_slice(&(frames[best].len() as u64).to_le_bytes());
            output.extend_from_slice(frames[best]);
            if let Some(pl) = probs.as_deref_mut() {
                let wp = [&p0, &p1, &p2, &p3, &p4][best];
                let real = chunk.len() * if four_bit { 2 } else { 4 };
                pl.extend_from_slice(&wp[..real.min(wp.len())]);
            }
        }
    }

    fn decode(&mut self, input: &[u8], four_bit: bool, output: &mut Vec<u8>) -> io::Result<()> {
        let mut pos = 0;
        while pos < input.len() {
            if pos + 17 > input.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated vmarkov frame header",
                ));
            }
            let mix = input[pos] as usize;
            pos += 1;
            if mix >= N_MIXTURES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "vmarkov mixture index out of range",
                ));
            }
            let orig_size = u64::from_le_bytes(input[pos..pos + 8].try_into().unwrap()) as usize;
            pos += 8;
            if orig_size > MAX_DECOMPRESS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "vmarkov frame size exceeds limit",
                ));
            }
            let frame_size = u64::from_le_bytes(input[pos..pos + 8].try_into().unwrap()) as usize;
            pos += 8;
            if pos + frame_size > input.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated vmarkov frame data",
                ));
            }
            self.reset_all();
            let frame = &input[pos..pos + frame_size];
            pos += frame_size;
            match mix {
                0 => decode_with(&mut self.v0, frame, orig_size, four_bit, output),
                1 => decode_with(&mut self.v1, frame, orig_size, four_bit, output),
                2 => decode_with(&mut self.v2, frame, orig_size, four_bit, output),
                3 => decode_with(&mut self.v3, frame, orig_size, four_bit, output),
                _ => decode_with(&mut self.v4, frame, orig_size, four_bit, output),
            }
        }
        Ok(())
    }
}

// --- Public codec ----------------------------------------------------------

pub struct MarkovCompressor {
    four_bit: bool,
    /// Sub-codec chosen from the FIRST block's size and kept for the whole
    /// stream (one genome -> one codec). Set once on the first compress call.
    chosen: Option<u8>,
    markov: Option<MarkovBlock>,
    bmarkov: Option<BmarkovBlock>,
    vmarkov: Option<VmarkovSet>,
    /// When set, per-base model probabilities are appended here in stream order.
    prob_out: Option<ProbSink>,
}

impl MarkovCompressor {
    pub fn new(four_bit: bool) -> Self {
        Self {
            four_bit,
            chosen: None,
            markov: None,
            bmarkov: None,
            vmarkov: None,
            prob_out: None,
        }
    }

    /// Construct with per-base probability capture into `sink` (stream order).
    /// Capture is only correct when the stream is compressed sequentially.
    pub fn with_prob(four_bit: bool, sink: ProbSink) -> Self {
        Self {
            four_bit,
            chosen: None,
            markov: None,
            bmarkov: None,
            vmarkov: None,
            prob_out: Some(sink),
        }
    }

    /// Conservative peak: any subset of the three model machines may be live.
    /// markov ~2.45 GiB, bmarkov ~2.3 GiB, vmarkov ~0.32 GiB.
    pub fn estimate_memory(_input_size: usize) -> usize {
        2_449_867_392 + 2_300_000_000 + 336 * 1024 * 1024
    }
}

impl Compressor for MarkovCompressor {
    fn compress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        // First block decides the codec for the whole stream; the tag is still
        // written per block so the decoder can route each block independently.
        let tag = *self.chosen.get_or_insert_with(|| dispatch(input.len()));
        output.push(tag);
        let fb = self.four_bit;
        let sink = self.prob_out.clone();
        let mut guard = sink.as_ref().map(|s| s.lock().unwrap());
        let probs = guard.as_deref_mut();
        match tag {
            TAG_VMARKOV => self
                .vmarkov
                .get_or_insert_with(VmarkovSet::new)
                .encode(input, fb, output, probs),
            TAG_BMARKOV => encode_nuc(
                self.bmarkov.get_or_insert_with(BmarkovBlock::new),
                input,
                fb,
                output,
                probs,
            ),
            _ => encode_nuc(
                self.markov.get_or_insert_with(MarkovBlock::new),
                input,
                fb,
                output,
                probs,
            ),
        }
        Ok(())
    }

    fn finish(&mut self, _output: &mut Vec<u8>) -> io::Result<()> {
        Ok(())
    }

    fn reset(&mut self) {
        self.chosen = None;
        if let Some(c) = &mut self.markov {
            c.reset();
        }
        if let Some(c) = &mut self.bmarkov {
            c.reset();
        }
        if let Some(c) = &mut self.vmarkov {
            c.reset_all();
        }
    }

    fn compress_memory(&self, input_size: usize) -> usize {
        Self::estimate_memory(input_size)
    }
}

pub struct MarkovDecompressor {
    four_bit: bool,
    markov: Option<MarkovBlock>,
    bmarkov: Option<BmarkovBlock>,
    vmarkov: Option<VmarkovSet>,
}

impl MarkovDecompressor {
    pub fn new(four_bit: bool) -> Self {
        Self {
            four_bit,
            markov: None,
            bmarkov: None,
            vmarkov: None,
        }
    }
}

impl Decompressor for MarkovDecompressor {
    fn decompress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        let tag = input[0];
        let rest = &input[1..];
        let fb = self.four_bit;
        match tag {
            TAG_VMARKOV => self
                .vmarkov
                .get_or_insert_with(VmarkovSet::new)
                .decode(rest, fb, output),
            TAG_BMARKOV => decode_nuc(
                self.bmarkov.get_or_insert_with(BmarkovBlock::new),
                rest,
                fb,
                output,
            ),
            TAG_MARKOV => decode_nuc(
                self.markov.get_or_insert_with(MarkovBlock::new),
                rest,
                fb,
                output,
            ),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("markov codec: unknown sub-codec tag {}", tag),
            )),
        }
    }

    fn reset(&mut self) {
        if let Some(c) = &mut self.markov {
            c.reset();
        }
        if let Some(c) = &mut self.bmarkov {
            c.reset();
        }
        if let Some(c) = &mut self.vmarkov {
            c.reset_all();
        }
    }

    fn decompress_memory(&self, _output_size: usize) -> usize {
        MarkovCompressor::estimate_memory(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(len: usize, four_bit: bool) {
        let _guard = crate::HEAVY_TEST_LOCK.lock().unwrap();
        let input: Vec<u8> = (0..len)
            .map(|i| {
                if four_bit {
                    [1u8, 2, 4, 8][(i * 7 + 3) % 4]
                } else {
                    ((i * 5 + 1) % 256) as u8
                }
            })
            .collect();
        let mut comp = MarkovCompressor::new(four_bit);
        let mut packed = Vec::new();
        comp.compress(&input, &mut packed).unwrap();
        comp.finish(&mut packed).unwrap();
        assert_eq!(packed[0], dispatch(input.len()));

        let mut dec = MarkovDecompressor::new(four_bit);
        let mut out = Vec::new();
        dec.decompress(&packed, &mut out).unwrap();
        assert_eq!(out, input, "len={} four_bit={}", len, four_bit);
    }

    #[test]
    fn dispatch_thresholds() {
        assert_eq!(dispatch(VMARKOV_MAX - 1), TAG_VMARKOV);
        assert_eq!(dispatch(VMARKOV_MAX), TAG_BMARKOV);
        assert_eq!(dispatch(bmarkov_max() - 1), TAG_BMARKOV);
        assert_eq!(dispatch(bmarkov_max()), TAG_MARKOV);
    }

    #[test]
    fn roundtrip_vmarkov_size() {
        roundtrip(50_000, false);
    }
    #[test]
    fn roundtrip_bmarkov_size() {
        roundtrip(400_000, false);
    }
    #[test]
    fn roundtrip_markov_size() {
        roundtrip(900_000, false);
    }
    #[test]
    fn roundtrip_markov_size_4bit() {
        roundtrip(900_000, true);
    }
}
