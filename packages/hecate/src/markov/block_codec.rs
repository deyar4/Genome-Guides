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

// Block-level encode/decode for the markov-mix compressor (level 7).

use super::aux_model::AuxModel;
use super::mix_aux_model::MixAuxModel;
use super::model::MarkovModel;
use super::rangecoder::{RangeDecoder, RangeEncoder};
use super::xlat::{backward_xlat_block, forward_xlat_block};
pub use super::xlat::BLOCK_SIZE;

const N_MODELS: usize = 5;

type M0 = MarkovModel<3, 0, false, false, 65536>;
type M1 = MarkovModel<7, 0, false, false, 1023>;
type M2 = MarkovModel<11, 2, false, true, 255>;
type M3 = MarkovModel<15, 6, true, true, 15>;
type M4 = MarkovModel<13, 9, true, false, 0>;

pub struct BlockCodec {
    m0: M0,
    m1: M1,
    m2: M2,
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
            m1: M1::new(),
            m2: M2::new(),
            m3: M3::new(),
            m4: M4::new(),
            aux: MixAuxModel::new(),
            btm: AuxModel::new(),
            ucm: AuxModel::new(),
            um: AuxModel::new(),
        }
    }

    /// Encode a block of BLOCK_SIZE bytes.
    /// For `four_bit=true`: bytes are 0-15 (IUPAC indices).
    /// For `four_bit=false`: bytes are 0-3 (ACGT only).
    #[inline(never)]
    pub fn encode_block(
        &mut self,
        block: &mut [u8; BLOCK_SIZE],
        four_bit: bool,
        enc: &mut RangeEncoder,
    ) {
        // 1. Four-bit: translate and encode block type flag.
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

        // 2. Model selection: compute coding cost for each model, pick best.
        let best = self.select_best(block, four_bit);
        {
            let mut cum = [0u32; N_MODELS + 1];
            self.aux.predict(&mut cum);
            enc.encode_symbol(N_MODELS, &cum, best);
            self.aux.update(best);
        }

        // 3. Encode each byte -- dispatch to const-generic specialization.
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
        // 1. Four-bit: decode block type flag.
        let has_non_acgt = if four_bit {
            let mut cum = [0u32; 3];
            self.btm.predict(&mut cum);
            let bad = dec.decode_symbol(2, &cum) != 0;
            self.btm.update(bad as usize);
            bad
        } else {
            false
        };

        // 2. Decode which model was selected.
        let best = {
            let mut cum = [0u32; N_MODELS + 1];
            self.aux.predict(&mut cum);
            let b = dec.decode_symbol(N_MODELS, &cum);
            self.aux.update(b);
            b
        };

        // 3. Decode each byte -- dispatch to const-generic specialization.
        match best {
            0 => self.decode_inner::<0>(four_bit, has_non_acgt, dec, out),
            1 => self.decode_inner::<1>(four_bit, has_non_acgt, dec, out),
            2 => self.decode_inner::<2>(four_bit, has_non_acgt, dec, out),
            3 => self.decode_inner::<3>(four_bit, has_non_acgt, dec, out),
            4 => self.decode_inner::<4>(four_bit, has_non_acgt, dec, out),
            _ => unreachable!(),
        }

        // 4. Four-bit: inverse translation.
        if four_bit {
            backward_xlat_block(out, has_non_acgt);
        }
    }

    pub fn reset(&mut self) {
        self.m0.reset();
        self.m1.reset();
        self.m2.reset();
        self.m3.reset();
        self.m4.reset();
        self.aux.reset();
        self.btm.reset();
        self.ucm.reset();
        self.um.reset();
    }

    // -- helpers --

    /// Evaluate each model's coding cost over the block and return the best.
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
        compute_cost!(self.m1, 1);
        compute_cost!(self.m2, 2);
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

    /// Const-generic inner encode loop. The compiler generates 5 specialized
    /// versions (one per BEST value), eliminating all model dispatch overhead.
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

            // Handle non-ACGT in 4-bit mode.
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

            // Normal ACGT encoding using the best model.
            let mut cum = [0u32; 5];
            match BEST {
                0 => self.m0.predict(&mut cum),
                1 => self.m1.predict(&mut cum),
                2 => self.m2.predict(&mut cum),
                3 => self.m3.predict(&mut cum),
                4 => self.m4.predict(&mut cum),
                _ => unsafe { core::hint::unreachable_unchecked() },
            }
            enc.encode_symbol(4, &cum, s as usize);
            if CAP {
                enc.log_base_prob(&cum, 4, s as usize);
            }

            // Update best model.
            match BEST {
                0 => self.m0.update_as_best(s),
                1 => self.m1.update_as_best(s),
                2 => self.m2.update_as_best(s),
                3 => self.m3.update_as_best(s),
                4 => self.m4.update_as_best(s),
                _ => unsafe { core::hint::unreachable_unchecked() },
            }

            // Update non-best models.
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
            if BEST != 4 {
                self.m4.update_as_nonbest(s);
            }
        }
    }

    /// Const-generic inner decode loop.
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
                4 => self.m4.predict(&mut cum),
                _ => unsafe { core::hint::unreachable_unchecked() },
            }
            let s = dec.decode_symbol(4, &cum) as u8;
            out[n] = s;

            match BEST {
                0 => self.m0.update_as_best(s),
                1 => self.m1.update_as_best(s),
                2 => self.m2.update_as_best(s),
                3 => self.m3.update_as_best(s),
                4 => self.m4.update_as_best(s),
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

    // -- xlat roundtrip tests (no models needed) --

    #[test]
    fn xlat_acgt_roundtrip() {
        // A(1), C(2), G(4), T(8) pattern repeated
        let original = {
            let mut b = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                b[i] = [1, 2, 4, 8][i % 4];
            }
            b
        };
        let mut block = original;
        let has_bad = forward_xlat_block(&mut block);
        assert!(!has_bad);
        // After forward xlat, values should be 0-3
        for &b in &block {
            assert!(b < 4, "expected 0-3, got {}", b);
        }
        backward_xlat_block(&mut block, false);
        assert_eq!(block, original);
    }

    #[test]
    fn xlat_iupac_roundtrip() {
        // Include some non-ACGT codes
        let original = {
            let mut b = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                b[i] = (i % 16) as u8; // 0-15 all IUPAC values
            }
            b
        };
        let mut block = original;
        let has_bad = forward_xlat_block(&mut block);
        assert!(has_bad);
        backward_xlat_block(&mut block, true);
        assert_eq!(block, original);
    }

    #[test]
    fn xlat_forward_maps_acgt_to_0_3() {
        // Fill entire block with valid ACGT values
        let mut block = [0u8; BLOCK_SIZE];
        for i in 0..BLOCK_SIZE {
            block[i] = [4, 2, 8, 1][i % 4]; // G, C, T, A
        }
        let has_bad = forward_xlat_block(&mut block);
        assert!(!has_bad);
        // G->0, C->1, T->2, A->3
        assert_eq!(block[0], 0);
        assert_eq!(block[1], 1);
        assert_eq!(block[2], 2);
        assert_eq!(block[3], 3);
    }

    // -- Block codec roundtrip tests --
    // These allocate all models including Model15 (~2 GiB).
    // Serialized via HEAVY_TEST_LOCK to prevent OOM from parallel runs.

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

        // Encode
        let mut block = original;
        let mut enc = RangeEncoder::new();
        codec_enc.encode_block(&mut block, false, &mut enc);
        enc.finish();

        // Decode
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

        // Encode all blocks
        let mut enc = RangeEncoder::new();
        for blk in &blocks {
            let mut block = *blk;
            codec_enc.encode_block(&mut block, false, &mut enc);
        }
        enc.finish();

        // Decode all blocks
        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        for original in &blocks {
            let mut decoded = [0u8; BLOCK_SIZE];
            codec_dec.decode_block(false, &mut dec, &mut decoded);
            assert_eq!(original, &decoded);
        }
    }

    #[test]
    fn roundtrip_4bit_acgt_only() {
        let _guard = crate::HEAVY_TEST_LOCK.lock().unwrap();
        let mut codec_enc = BlockCodec::new();
        let mut codec_dec = BlockCodec::new();

        // Block of pure ACGT values (IUPAC: A=1, C=2, G=4, T=8)
        let original = {
            let mut b = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                b[i] = [1, 2, 4, 8][i % 4];
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
    fn roundtrip_4bit_with_iupac() {
        let _guard = crate::HEAVY_TEST_LOCK.lock().unwrap();
        let mut codec_enc = BlockCodec::new();
        let mut codec_dec = BlockCodec::new();

        // Block with mixed ACGT and IUPAC ambiguity codes
        let original = {
            let mut b = [0u8; BLOCK_SIZE];
            for i in 0..BLOCK_SIZE {
                b[i] = match i % 8 {
                    0 => 1,  // A
                    1 => 2,  // C
                    2 => 4,  // G
                    3 => 8,  // T
                    4 => 3,  // M (A or C)
                    5 => 5,  // R (A or G)
                    6 => 15, // N (any)
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

        // First session
        let mut enc = RangeEncoder::new();
        let mut b = block1;
        codec_enc.encode_block(&mut b, false, &mut enc);
        enc.finish();

        let mut dec = RangeDecoder::new(enc.output());
        let mut decoded = [0u8; BLOCK_SIZE];
        codec_dec.decode_block(false, &mut dec, &mut decoded);
        assert_eq!(block1, decoded);

        // Reset both codecs
        codec_enc.reset();
        codec_dec.reset();

        // Second session with different data
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
