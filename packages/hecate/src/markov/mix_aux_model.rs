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

// Blended order-2/3/4 predictor for model-selection symbols in the
// markov-mix block codec.
//
// Symbols are packed 3 bits per symbol (N_MODELS=5 rounded up to 8).
// Each order holds u8 count cells; on saturation at MAX_COUNT=127 the
// entire row halves.  Predictions are a fixed 1:2:1 integer blend so
// encoder/decoder produce bit-exact CDFs without floating point.

const N: usize = 5;
const BITS: u32 = 3;
const MAX_COUNT: u8 = 127;
const N_CTX2: usize = 1 << (2 * BITS as usize); // 64
const N_CTX3: usize = 1 << (3 * BITS as usize); // 512
const N_CTX4: usize = 1 << (4 * BITS as usize); // 4096
const M2: u32 = (N_CTX2 - 1) as u32;
const M3: u32 = (N_CTX3 - 1) as u32;
const M4: u32 = (N_CTX4 - 1) as u32;

pub struct MixAuxModel {
    c2: Box<[u8]>,
    c3: Box<[u8]>,
    c4: Box<[u8]>,
    ctx2: u32,
    ctx3: u32,
    ctx4: u32,
}

impl MixAuxModel {
    pub fn new() -> Self {
        Self {
            c2: vec![0u8; N_CTX2 * N].into_boxed_slice(),
            c3: vec![0u8; N_CTX3 * N].into_boxed_slice(),
            c4: vec![0u8; N_CTX4 * N].into_boxed_slice(),
            ctx2: 0,
            ctx3: 0,
            ctx4: 0,
        }
    }

    /// Fill `cum[1..=N]` with cumulative frequencies; `cum[0]` must be 0.
    #[inline(always)]
    pub fn predict(&self, cum: &mut [u32]) {
        let b2 = (self.ctx2 as usize) * N;
        let b3 = (self.ctx3 as usize) * N;
        let b4 = (self.ctx4 as usize) * N;
        let mut f2 = [0u32; N];
        let mut f3 = [0u32; N];
        let mut f4 = [0u32; N];
        let (mut t2, mut t3, mut t4) = (0u32, 0u32, 0u32);
        unsafe {
            for j in 0..N {
                f2[j] = 1 + *self.c2.get_unchecked(b2 + j) as u32;
                t2 += f2[j];
                f3[j] = 1 + *self.c3.get_unchecked(b3 + j) as u32;
                t3 += f3[j];
                f4[j] = 1 + *self.c4.get_unchecked(b4 + j) as u32;
                t4 += f4[j];
            }
        }
        // Blend 1:2:1 by cross-multiplying with the other orders' totals,
        // yielding a common denominator of 4*t2*t3*t4. Cells fit in u32:
        // worst-case per-symbol v <= 4*128*(5*128)^2 ~= 2^28, summed over
        // N=5 <= 2^30.6 -- well under 2^31 for the range coder's muldiv.
        let t34 = t3 * t4;
        let t24 = t2 * t4;
        let t23 = t2 * t3;
        cum[0] = 0;
        let mut acc = 0u32;
        for j in 0..N {
            let v = f2[j] * t34 + 2 * f3[j] * t24 + f4[j] * t23;
            acc += v;
            cum[j + 1] = acc;
        }
    }

    #[inline(always)]
    pub fn update(&mut self, symbol: usize) {
        let b2 = (self.ctx2 as usize) * N;
        let b3 = (self.ctx3 as usize) * N;
        let b4 = (self.ctx4 as usize) * N;
        Self::bump(&mut self.c2, b2, symbol);
        Self::bump(&mut self.c3, b3, symbol);
        Self::bump(&mut self.c4, b4, symbol);
        let s = symbol as u32;
        self.ctx2 = ((self.ctx2 << BITS) | s) & M2;
        self.ctx3 = ((self.ctx3 << BITS) | s) & M3;
        self.ctx4 = ((self.ctx4 << BITS) | s) & M4;
    }

    #[inline(always)]
    fn bump(tab: &mut [u8], base: usize, s: usize) {
        unsafe {
            if *tab.get_unchecked(base + s) == MAX_COUNT {
                for j in 0..N {
                    *tab.get_unchecked_mut(base + j) >>= 1;
                }
            }
            *tab.get_unchecked_mut(base + s) += 1;
        }
    }

    pub fn reset(&mut self) {
        self.c2.iter_mut().for_each(|c| *c = 0);
        self.c3.iter_mut().for_each(|c| *c = 0);
        self.c4.iter_mut().for_each(|c| *c = 0);
        self.ctx2 = 0;
        self.ctx3 = 0;
        self.ctx4 = 0;
    }
}

impl Default for MixAuxModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov::rangecoder::{RangeDecoder, RangeEncoder};

    #[test]
    fn fresh_predict_is_uniform() {
        let m = MixAuxModel::new();
        let mut cum = [0u32; N + 1];
        m.predict(&mut cum);
        assert_eq!(cum[0], 0);
        // All five symbols equally likely -> equal gaps.
        let gap = cum[1] - cum[0];
        for j in 1..N {
            assert_eq!(cum[j + 1] - cum[j], gap);
        }
    }

    #[test]
    fn update_shifts_distribution() {
        let mut m = MixAuxModel::new();
        for _ in 0..16 {
            m.update(0);
        }
        let mut cum = [0u32; N + 1];
        m.predict(&mut cum);
        let g0 = cum[1] - cum[0];
        let g1 = cum[2] - cum[1];
        assert!(g0 > g1, "symbol 0 should dominate after repeated updates");
    }

    #[test]
    fn roundtrip_random() {
        let syms: Vec<u8> = (0..2000).map(|i| ((i * 131 + 7) % 5) as u8).collect();

        let mut enc = RangeEncoder::new();
        let mut me = MixAuxModel::new();
        for &s in &syms {
            let mut cum = [0u32; N + 1];
            me.predict(&mut cum);
            enc.encode_symbol(N, &cum, s as usize);
            me.update(s as usize);
        }
        enc.finish();
        let encoded = enc.into_output();

        let mut dec = RangeDecoder::new(&encoded);
        let mut md = MixAuxModel::new();
        let mut out = Vec::with_capacity(syms.len());
        for _ in 0..syms.len() {
            let mut cum = [0u32; N + 1];
            md.predict(&mut cum);
            let s = dec.decode_symbol(N, &cum);
            md.update(s);
            out.push(s as u8);
        }
        assert_eq!(out, syms);
    }

    #[test]
    fn reset_clears_state() {
        let mut m = MixAuxModel::new();
        for _ in 0..8 {
            m.update(2);
        }
        m.reset();
        assert_eq!(m.ctx2, 0);
        assert_eq!(m.ctx3, 0);
        assert_eq!(m.ctx4, 0);
        let mut cum = [0u32; N + 1];
        m.predict(&mut cum);
        let gap = cum[1];
        for j in 1..N {
            assert_eq!(cum[j + 1] - cum[j], gap);
        }
    }
}
