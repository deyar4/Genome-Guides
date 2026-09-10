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

// Block-level encode/decode for the bmarkov compressor.
//
// Five-model mixture:
//   m0: order=11 alpha_den=4  update_reg=true  update_rc=true  max_count=127
//   m1: order=15 alpha_den=12 update_reg=true  update_rc=true  max_count=0    (shared)
//   m2: order=15 alpha_den=6  update_reg=true  update_rc=true  max_count=0    (shared)
//   m3: order=5  alpha_den=0  update_reg=true  update_rc=true  max_count=1023
//   m4: order=3  alpha_den=0  update_reg=false update_rc=false max_count=255
//
// m1 and m2 have identical storage semantics (same order, same update rules,
// max_count=0 forces zero-then-increment per update).  We keep one physical
// MarkovModel backing both and evaluate predictions with a runtime alpha_mul.

use crate::markov::aux_model::AuxModel;
use crate::markov::mix_aux_model::MixAuxModel;
use crate::markov::model::MarkovModel;
use crate::markov::rangecoder::{RangeDecoder, RangeEncoder};
use crate::markov::xlat::{backward_xlat_block, forward_xlat_block};

pub use crate::markov::xlat::BLOCK_SIZE;
const N_MODELS: usize = 5;

// m1 is the primary order-15 model; m2 is a virtual view over its storage
// using a different alpha_mul.  We use m1's type as the storage owner.
type M0 = MarkovModel<11, 4, true, true, 127>;
type M15Shared = MarkovModel<15, 12, true, true, 0>;
type M3 = MarkovModel<5, 0, true, true, 1023>;
type M4 = MarkovModel<3, 0, false, false, 255>;

// ALPHA_MUL for the m2 view: 1 << 6 = 64.
const M2_ALPHA_MUL: u32 = 1 << 6;

pub struct BlockCodec {
    m0: M0,
    m15: M15Shared,
    m3: M3,
    m4: M4,
    aux: MixAuxModel,
    btm: AuxModel<2>, // block type (hasNonACGT)
    ucm: AuxModel<2>, // unknown choice (known vs unknown)
    um: AuxModel<12>, // unknown symbol (12 non-ACGT IUPAC codes)
}


impl BlockCodec {
    pub fn new() -> Self {
        Self {
            m0: M0::new(),
            m15: M15Shared::new(),
            m3: M3::new(),
            m4: M4::new(),
            aux: MixAuxModel::new(),
            btm: AuxModel::new(),
            ucm: AuxModel::new(),
            um: AuxModel::new(),
        }
    }

    /// Encode a block of BLOCK_SIZE bytes.
    #[inline(never)]
    pub fn encode_block(
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
            let mut cum = [0u32; N_MODELS + 1];
            self.aux.predict(&mut cum);
            enc.encode_symbol(N_MODELS, &cum, best);
            self.aux.update(best);
        }

        let cap = enc.is_capturing();
        macro_rules! run {
            ($b:literal) => {
                if cap {
                    self.encode_inner::<$b, true>(block, four_bit, has_non_acgt, enc)
                } else {
                    self.encode_inner::<$b, false>(block, four_bit, has_non_acgt, enc)
                }
            };
        }
        match best {
            0 => run!(0),
            1 => run!(1),
            2 => run!(2),
            3 => run!(3),
            4 => run!(4),
            _ => unreachable!(),
        }
    }

    /// Decode a block of BLOCK_SIZE bytes.
    #[inline(never)]
    pub fn decode_block(
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
            let mut cum = [0u32; N_MODELS + 1];
            self.aux.predict(&mut cum);
            let b = dec.decode_symbol(N_MODELS, &cum);
            self.aux.update(b);
            b
        };

        match best {
            0 => self.decode_inner::<0>(four_bit, has_non_acgt, dec, out),
            1 => self.decode_inner::<1>(four_bit, has_non_acgt, dec, out),
            2 => self.decode_inner::<2>(four_bit, has_non_acgt, dec, out),
            3 => self.decode_inner::<3>(four_bit, has_non_acgt, dec, out),
            4 => self.decode_inner::<4>(four_bit, has_non_acgt, dec, out),
            _ => unreachable!(),
        }

        if four_bit {
            backward_xlat_block(out, has_non_acgt);
        }
    }

    pub fn reset(&mut self) {
        self.m0.reset();
        self.m15.reset();
        self.m3.reset();
        self.m4.reset();
        self.aux.reset();
        self.btm.reset();
        self.ucm.reset();
        self.um.reset();
    }

    // -- helpers --

    /// Evaluate each model's coding cost over the block and return the best.
    /// m1 and m2 share the same storage and context, only alpha_mul differs.
    fn select_best(&mut self, block: &[u8; BLOCK_SIZE], four_bit: bool) -> usize {
        let mut costs = [0u32; N_MODELS];

        macro_rules! compute_cost {
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

        compute_cost!(self.m0, 0);
        // Shared order-15 model: walk ctx once, accumulate into both costs.
        {
            let saved = self.m15.ctx;
            for &b in block.iter() {
                if four_bit && b >= 4 {
                    continue;
                }
                costs[1] += self.m15.coding_cost(b as usize);
                costs[2] += self
                    .m15
                    .coding_cost_with_alpha_mul(M2_ALPHA_MUL, b as usize);
                self.m15.update_ctx(b);
            }
            self.m15.ctx = saved;
        }
        compute_cost!(self.m3, 3);
        compute_cost!(self.m4, 4);

        let mut best = 0;
        for i in 1..N_MODELS {
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
                1 => self.m15.predict(&mut cum),
                2 => self.m15.predict_with_alpha_mul(M2_ALPHA_MUL, &mut cum),
                3 => self.m3.predict(&mut cum),
                4 => self.m4.predict(&mut cum),
                _ => unsafe { core::hint::unreachable_unchecked() },
            }
            enc.encode_symbol(4, &cum, s as usize);
            if CAP {
                enc.log_base_prob(&cum, 4, s as usize);
            }

            match BEST {
                0 => self.m0.update_as_best(s),
                1 | 2 => self.m15.update_as_best(s),
                3 => self.m3.update_as_best(s),
                4 => self.m4.update_as_best(s),
                _ => unsafe { core::hint::unreachable_unchecked() },
            }

            if BEST != 0 {
                self.m0.update_as_nonbest(s);
            }
            // The shared order-15 model is updated at most once per symbol;
            // BEST==1 and BEST==2 both route through update_as_best above.
            if BEST != 1 && BEST != 2 {
                self.m15.update_as_nonbest(s);
            }
            if BEST != 3 {
                self.m3.update_as_nonbest(s);
            }
            if BEST != 4 {
                self.m4.update_as_nonbest(s);
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
                1 => self.m15.predict(&mut cum),
                2 => self.m15.predict_with_alpha_mul(M2_ALPHA_MUL, &mut cum),
                3 => self.m3.predict(&mut cum),
                4 => self.m4.predict(&mut cum),
                _ => unsafe { core::hint::unreachable_unchecked() },
            }
            let s = dec.decode_symbol(4, &cum) as u8;
            out[n] = s;

            match BEST {
                0 => self.m0.update_as_best(s),
                1 | 2 => self.m15.update_as_best(s),
                3 => self.m3.update_as_best(s),
                4 => self.m4.update_as_best(s),
                _ => unsafe { core::hint::unreachable_unchecked() },
            }

            if BEST != 0 {
                self.m0.update_as_nonbest(s);
            }
            if BEST != 1 && BEST != 2 {
                self.m15.update_as_nonbest(s);
            }
            if BEST != 3 {
                self.m3.update_as_nonbest(s);
            }
            if BEST != 4 {
                self.m4.update_as_nonbest(s);
            }
        }
    }
}

impl Default for BlockCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_2bit_single_block() {
        let _guard = crate::HEAVY_TEST_LOCK.lock().unwrap();
        let mut codec_enc = BlockCodec::new();
        let mut codec_dec = BlockCodec::new();

        let original = {
            let mut b = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                b[i] = ((i * 3 + 1) % 4) as u8;
            }
            b
        };

        let mut block = original;
        let mut enc = RangeEncoder::new();
        codec_enc.encode_block(&mut block, false, &mut enc);
        enc.finish();

        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        let mut decoded = [0u8; BLOCK_SIZE];
        codec_dec.decode_block(false, &mut dec, &mut decoded);

        assert_eq!(original, decoded);
    }

    #[test]
    fn roundtrip_2bit_multiple_blocks() {
        let _guard = crate::HEAVY_TEST_LOCK.lock().unwrap();
        let mut codec_enc = BlockCodec::new();
        let mut codec_dec = BlockCodec::new();

        let blocks: Vec<[u8; BLOCK_SIZE]> = (0..4)
            .map(|blk| {
                let mut b = [0u8; BLOCK_SIZE];
                for i in 0..BLOCK_SIZE {
                    b[i] = ((i + blk * 7) % 4) as u8;
                }
                b
            })
            .collect();

        let mut enc = RangeEncoder::new();
        for blk in &blocks {
            let mut block = *blk;
            codec_enc.encode_block(&mut block, false, &mut enc);
        }
        enc.finish();

        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        for original in &blocks {
            let mut decoded = [0u8; BLOCK_SIZE];
            codec_dec.decode_block(false, &mut dec, &mut decoded);
            assert_eq!(original, &decoded);
        }
    }

    #[test]
    fn roundtrip_4bit_with_iupac() {
        let _guard = crate::HEAVY_TEST_LOCK.lock().unwrap();
        let mut codec_enc = BlockCodec::new();
        let mut codec_dec = BlockCodec::new();

        let original = {
            let mut b = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                b[i] = match i % 8 {
                    0 => 1,  // A
                    1 => 2,  // C
                    2 => 4,  // G
                    3 => 8,  // T
                    4 => 3,  // M
                    5 => 5,  // R
                    6 => 15, // N
                    7 => 1,  // A
                    _ => unreachable!(),
                };
            }
            b
        };

        let mut block = original;
        let mut enc = RangeEncoder::new();
        codec_enc.encode_block(&mut block, true, &mut enc);
        enc.finish();

        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        let mut decoded = [0u8; BLOCK_SIZE];
        codec_dec.decode_block(true, &mut dec, &mut decoded);

        assert_eq!(original, decoded);
    }

    #[test]
    fn roundtrip_2bit_after_reset() {
        let _guard = crate::HEAVY_TEST_LOCK.lock().unwrap();
        let mut codec_enc = BlockCodec::new();
        let mut codec_dec = BlockCodec::new();

        let block1 = {
            let mut b = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                b[i] = (i % 4) as u8;
            }
            b
        };
        let block2 = {
            let mut b = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                b[i] = ((i * 2 + 1) % 4) as u8;
            }
            b
        };

        let mut enc = RangeEncoder::new();
        let mut b = block1;
        codec_enc.encode_block(&mut b, false, &mut enc);
        enc.finish();

        let mut dec = RangeDecoder::new(enc.output());
        let mut decoded = [0u8; BLOCK_SIZE];
        codec_dec.decode_block(false, &mut dec, &mut decoded);
        assert_eq!(block1, decoded);

        codec_enc.reset();
        codec_dec.reset();

        let mut enc = RangeEncoder::new();
        let mut b = block2;
        codec_enc.encode_block(&mut b, false, &mut enc);
        enc.finish();

        let mut dec = RangeDecoder::new(enc.output());
        let mut decoded = [0u8; BLOCK_SIZE];
        codec_dec.decode_block(false, &mut dec, &mut decoded);
        assert_eq!(block2, decoded);
    }
}
