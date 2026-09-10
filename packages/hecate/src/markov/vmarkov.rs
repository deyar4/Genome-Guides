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

// Per-virus-tuned block codecs for vmarkov.
//
// Five mixtures, each with 4 Markov models, discovered by a per-genome
// sweep of the markov optimiser on small viral genomes.  The vmarkov
// compressor tries all five on every mux block and writes the index of
// the winner, so each block picks whichever mixture fits its local
// statistics best.
//
// Mixtures (order, alpha_den, update_reg, update_rc, max_count):
//   V0 (hbv,        2.002 b/sym): (13,0,F,F,0)   (3,2,T,F,255)  (3,0,T,T,255)   (7,0,T,T,0)
//   V1 (phi_x_174,  1.976 b/sym): (5,0,T,F,0)    (3,0,T,F,255)  (9,0,F,F,0)     (3,1,T,T,255)
//   V2 (hiv_1,      1.918 b/sym): (7,2,T,T,0)    (9,12,T,F,0)   (3,0,T,F,255)   (11,9,T,F,0)
//   V3 (zika,       1.958 b/sym): (3,0,F,F,255)  (3,0,T,F,255)  (7,2,T,F,63)    (7,1,T,T,63)
//   V4 (sars_cov_2, 1.930 b/sym): (3,0,T,T,255)  (7,2,T,F,0)    (7,3,T,T,0)     (9,9,T,T,0)

use crate::markov::aux_model::AuxModel;
use crate::markov::model::MarkovModel;
use crate::markov::rangecoder::{RangeDecoder, RangeEncoder};
use crate::markov::xlat::{backward_xlat_block, forward_xlat_block};

pub use crate::markov::xlat::BLOCK_SIZE;
pub const N_MIXTURES: usize = 5;


// -- BlockCodec trait: each mixture-specific codec implements this --

pub trait MixtureBlockCodec {
    fn encode_block(
        &mut self,
        block: &mut [u8; BLOCK_SIZE],
        four_bit: bool,
        enc: &mut RangeEncoder,
    );
    fn decode_block(&mut self, four_bit: bool, dec: &mut RangeDecoder, out: &mut [u8; BLOCK_SIZE]);
    fn reset(&mut self);
}

// -- Macro generating one mixture's BlockCodec --
//
// 4-model mixture, N_MODELS=4, model selection via AuxModel<4>.

macro_rules! make_mixture {
    ($name:ident, $m0:ty, $m1:ty, $m2:ty, $m3:ty) => {
        pub struct $name {
            m0: $m0,
            m1: $m1,
            m2: $m2,
            m3: $m3,
            aux: AuxModel<4>,
            btm: AuxModel<2>,
            ucm: AuxModel<2>,
            um: AuxModel<12>,
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    m0: <$m0>::new(),
                    m1: <$m1>::new(),
                    m2: <$m2>::new(),
                    m3: <$m3>::new(),
                    aux: AuxModel::new(),
                    btm: AuxModel::new(),
                    ucm: AuxModel::new(),
                    um: AuxModel::new(),
                }
            }

            fn select_best(&mut self, block: &[u8; BLOCK_SIZE], four_bit: bool) -> usize {
                let mut costs = [0u32; 4];
                macro_rules! walk {
                    ($model:expr, $idx:expr) => {{
                        let saved = $model.ctx;
                        for &b in block.iter() {
                            if four_bit && b >= 4 {
                                continue;
                            }
                            costs[$idx] += $model.coding_cost(b as usize);
                            $model.update_ctx(b);
                        }
                        $model.ctx = saved;
                    }};
                }
                walk!(self.m0, 0);
                walk!(self.m1, 1);
                walk!(self.m2, 2);
                walk!(self.m3, 3);
                let mut best = 0;
                for i in 1..4 {
                    if costs[i] < costs[best] {
                        best = i;
                    }
                }
                best
            }

            #[inline(always)]
            fn encode_inner<const BEST: usize, const CAP: bool>(
                &mut self,
                block: &[u8; BLOCK_SIZE],
                four_bit: bool,
                has_non_acgt: bool,
                enc: &mut RangeEncoder,
            ) {
                for n in 0..BLOCK_SIZE {
                    let s = block[n];
                    if four_bit && has_non_acgt {
                        let uc = (s >= 4) as usize;
                        let mut cum = [0u32; 3];
                        self.ucm.predict(&mut cum);
                        enc.encode_symbol(2, &cum, uc);
                        self.ucm.update(uc);
                        if uc != 0 {
                            let mut cum = [0u32; 13];
                            self.um.predict(&mut cum);
                            enc.encode_symbol(12, &cum, (s - 4) as usize);
                            if CAP {
                                enc.log_base_prob(&cum, 12, (s - 4) as usize);
                            }
                            self.um.update((s - 4) as usize);
                            continue;
                        }
                    }
                    let mut cum = [0u32; 5];
                    match BEST {
                        0 => self.m0.predict(&mut cum),
                        1 => self.m1.predict(&mut cum),
                        2 => self.m2.predict(&mut cum),
                        3 => self.m3.predict(&mut cum),
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    }
                    enc.encode_symbol(4, &cum, s as usize);
                    if CAP {
                        enc.log_base_prob(&cum, 4, s as usize);
                    }
                    match BEST {
                        0 => self.m0.update_as_best(s),
                        1 => self.m1.update_as_best(s),
                        2 => self.m2.update_as_best(s),
                        3 => self.m3.update_as_best(s),
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    }
                    if BEST != 0 {
                        self.m0.update_as_nonbest(s);
                    }
                    if BEST != 1 {
                        self.m1.update_as_nonbest(s);
                    }
                    if BEST != 2 {
                        self.m2.update_as_nonbest(s);
                    }
                    if BEST != 3 {
                        self.m3.update_as_nonbest(s);
                    }
                }
            }

            #[inline(always)]
            fn decode_inner<const BEST: usize>(
                &mut self,
                four_bit: bool,
                has_non_acgt: bool,
                dec: &mut RangeDecoder,
                out: &mut [u8; BLOCK_SIZE],
            ) {
                for n in 0..BLOCK_SIZE {
                    if four_bit && has_non_acgt {
                        let mut cum = [0u32; 3];
                        self.ucm.predict(&mut cum);
                        let uc = dec.decode_symbol(2, &cum) != 0;
                        self.ucm.update(uc as usize);
                        if uc {
                            let mut cum = [0u32; 13];
                            self.um.predict(&mut cum);
                            let s = dec.decode_symbol(12, &cum);
                            self.um.update(s);
                            out[n] = s as u8 + 4;
                            continue;
                        }
                    }
                    let mut cum = [0u32; 5];
                    match BEST {
                        0 => self.m0.predict(&mut cum),
                        1 => self.m1.predict(&mut cum),
                        2 => self.m2.predict(&mut cum),
                        3 => self.m3.predict(&mut cum),
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    }
                    let s = dec.decode_symbol(4, &cum) as u8;
                    out[n] = s;
                    match BEST {
                        0 => self.m0.update_as_best(s),
                        1 => self.m1.update_as_best(s),
                        2 => self.m2.update_as_best(s),
                        3 => self.m3.update_as_best(s),
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    }
                    if BEST != 0 {
                        self.m0.update_as_nonbest(s);
                    }
                    if BEST != 1 {
                        self.m1.update_as_nonbest(s);
                    }
                    if BEST != 2 {
                        self.m2.update_as_nonbest(s);
                    }
                    if BEST != 3 {
                        self.m3.update_as_nonbest(s);
                    }
                }
            }
        }

        impl MixtureBlockCodec for $name {
            #[inline(never)]
            fn encode_block(
                &mut self,
                block: &mut [u8; BLOCK_SIZE],
                four_bit: bool,
                enc: &mut RangeEncoder,
            ) {
                let has_non_acgt = if four_bit {
                    let bad = forward_xlat_block(block);
                    let mut cum = [0u32; 3];
                    self.btm.predict(&mut cum);
                    enc.encode_symbol(2, &cum, bad as usize);
                    self.btm.update(bad as usize);
                    bad
                } else {
                    false
                };
                let best = self.select_best(block, four_bit);
                {
                    let mut cum = [0u32; 5];
                    self.aux.predict(&mut cum);
                    enc.encode_symbol(4, &cum, best);
                    self.aux.update(best);
                }
                match (best, enc.is_capturing()) {
                    (0, false) => self.encode_inner::<0, false>(block, four_bit, has_non_acgt, enc),
                    (0, true) => self.encode_inner::<0, true>(block, four_bit, has_non_acgt, enc),
                    (1, false) => self.encode_inner::<1, false>(block, four_bit, has_non_acgt, enc),
                    (1, true) => self.encode_inner::<1, true>(block, four_bit, has_non_acgt, enc),
                    (2, false) => self.encode_inner::<2, false>(block, four_bit, has_non_acgt, enc),
                    (2, true) => self.encode_inner::<2, true>(block, four_bit, has_non_acgt, enc),
                    (3, false) => self.encode_inner::<3, false>(block, four_bit, has_non_acgt, enc),
                    (3, true) => self.encode_inner::<3, true>(block, four_bit, has_non_acgt, enc),
                    _ => unreachable!(),
                }
            }

            #[inline(never)]
            fn decode_block(
                &mut self,
                four_bit: bool,
                dec: &mut RangeDecoder,
                out: &mut [u8; BLOCK_SIZE],
            ) {
                let has_non_acgt = if four_bit {
                    let mut cum = [0u32; 3];
                    self.btm.predict(&mut cum);
                    let bad = dec.decode_symbol(2, &cum) != 0;
                    self.btm.update(bad as usize);
                    bad
                } else {
                    false
                };
                let best = {
                    let mut cum = [0u32; 5];
                    self.aux.predict(&mut cum);
                    let b = dec.decode_symbol(4, &cum);
                    self.aux.update(b);
                    b
                };
                match best {
                    0 => self.decode_inner::<0>(four_bit, has_non_acgt, dec, out),
                    1 => self.decode_inner::<1>(four_bit, has_non_acgt, dec, out),
                    2 => self.decode_inner::<2>(four_bit, has_non_acgt, dec, out),
                    3 => self.decode_inner::<3>(four_bit, has_non_acgt, dec, out),
                    _ => unreachable!(),
                }
                if four_bit {
                    backward_xlat_block(out, has_non_acgt);
                }
            }

            fn reset(&mut self) {
                self.m0.reset();
                self.m1.reset();
                self.m2.reset();
                self.m3.reset();
                self.aux.reset();
                self.btm.reset();
                self.ucm.reset();
                self.um.reset();
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

// V0: hbv
make_mixture!(V0BlockCodec,
    MarkovModel<13, 0, false, false, 0>,
    MarkovModel<3, 2, true, false, 255>,
    MarkovModel<3, 0, true, true, 255>,
    MarkovModel<7, 0, true, true, 0>);

// V1: phi_x_174
make_mixture!(V1BlockCodec,
    MarkovModel<5, 0, true, false, 0>,
    MarkovModel<3, 0, true, false, 255>,
    MarkovModel<9, 0, false, false, 0>,
    MarkovModel<3, 1, true, true, 255>);

// V2: hiv_1
make_mixture!(V2BlockCodec,
    MarkovModel<7, 2, true, true, 0>,
    MarkovModel<9, 12, true, false, 0>,
    MarkovModel<3, 0, true, false, 255>,
    MarkovModel<11, 9, true, false, 0>);

// V3: zika
make_mixture!(V3BlockCodec,
    MarkovModel<3, 0, false, false, 255>,
    MarkovModel<3, 0, true, false, 255>,
    MarkovModel<7, 2, true, false, 63>,
    MarkovModel<7, 1, true, true, 63>);

// V4: sars_cov_2
make_mixture!(V4BlockCodec,
    MarkovModel<3, 0, true, true, 255>,
    MarkovModel<7, 2, true, false, 0>,
    MarkovModel<7, 3, true, true, 0>,
    MarkovModel<9, 9, true, true, 0>);

/// Encode one mux block with a single mixture codec and return the compressed frame.
pub fn encode_with<C: MixtureBlockCodec>(
    codec: &mut C,
    input: &[u8],
    four_bit: bool,
    capture: bool,
) -> (Vec<u8>, Vec<u32>) {
    let mut enc = RangeEncoder::with_capacity(input.len());
    if capture {
        enc.enable_prob_log();
    }
    if four_bit {
        let packed_per_block = BLOCK_SIZE / 2;
        let mut offset = 0;
        while offset < input.len() {
            let mut block = [0u8; BLOCK_SIZE];
            let chunk = &input[offset..(offset + packed_per_block).min(input.len())];
            for (j, &b) in chunk.iter().enumerate() {
                block[j * 2] = b & 0x0F;
                block[j * 2 + 1] = (b >> 4) & 0x0F;
            }
            codec.encode_block(&mut block, true, &mut enc);
            offset += packed_per_block;
        }
    } else {
        let packed_per_block = BLOCK_SIZE / 4;
        let mut offset = 0;
        while offset < input.len() {
            let mut block = [0u8; BLOCK_SIZE];
            let chunk = &input[offset..(offset + packed_per_block).min(input.len())];
            for (j, &b) in chunk.iter().enumerate() {
                block[j * 4] = b & 0x03;
                block[j * 4 + 1] = (b >> 2) & 0x03;
                block[j * 4 + 2] = (b >> 4) & 0x03;
                block[j * 4 + 3] = (b >> 6) & 0x03;
            }
            codec.encode_block(&mut block, false, &mut enc);
            offset += packed_per_block;
        }
    }
    enc.finish();
    let probs = enc.take_prob_log().unwrap_or_default();
    (enc.into_output(), probs)
}

/// Decode all blocks of a frame with a single mixture codec.
pub fn decode_with<C: MixtureBlockCodec>(
    codec: &mut C,
    frame: &[u8],
    orig_size: usize,
    four_bit: bool,
    output: &mut Vec<u8>,
) {
    let mut dec = RangeDecoder::new(frame);
    let mut decoded = 0usize;
    if four_bit {
        let ppb = BLOCK_SIZE / 2;
        while decoded < orig_size {
            let mut block = [0u8; BLOCK_SIZE];
            codec.decode_block(true, &mut dec, &mut block);
            let pack_count = ppb.min(orig_size - decoded);
            for j in 0..pack_count {
                output.push((block[j * 2] & 0x0F) | ((block[j * 2 + 1] & 0x0F) << 4));
            }
            decoded += ppb;
        }
    } else {
        let ppb = BLOCK_SIZE / 4;
        while decoded < orig_size {
            let mut block = [0u8; BLOCK_SIZE];
            codec.decode_block(false, &mut dec, &mut block);
            let pack_count = ppb.min(orig_size - decoded);
            for j in 0..pack_count {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_each_mixture_2bit() {
        let input: Vec<u8> = (0..200).map(|i| ((i * 3 + 1) % 256) as u8).collect();
        macro_rules! test_one {
            ($codec:ty) => {{
                let mut c_enc = <$codec>::new();
                let (comp, _) = encode_with(&mut c_enc, &input, false, false);
                let mut c_dec = <$codec>::new();
                let mut out = Vec::new();
                decode_with(&mut c_dec, &comp, input.len(), false, &mut out);
                assert_eq!(out, input);
            }};
        }
        test_one!(V0BlockCodec);
        test_one!(V1BlockCodec);
        test_one!(V2BlockCodec);
        test_one!(V3BlockCodec);
        test_one!(V4BlockCodec);
    }

    #[test]
    fn roundtrip_each_mixture_4bit() {
        let input: Vec<u8> = (0..200)
            .map(|i| {
                let lo = ((i * 5 + 2) % 16) as u8;
                let hi = ((i * 7 + 3) % 16) as u8;
                lo | (hi << 4)
            })
            .collect();
        macro_rules! test_one {
            ($codec:ty) => {{
                let mut c_enc = <$codec>::new();
                let (comp, _) = encode_with(&mut c_enc, &input, true, false);
                let mut c_dec = <$codec>::new();
                let mut out = Vec::new();
                decode_with(&mut c_dec, &comp, input.len(), true, &mut out);
                assert_eq!(out, input);
            }};
        }
        test_one!(V0BlockCodec);
        test_one!(V1BlockCodec);
        test_one!(V2BlockCodec);
        test_one!(V3BlockCodec);
        test_one!(V4BlockCodec);
    }
}
