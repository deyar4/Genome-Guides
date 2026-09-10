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

const S_TOP: u32 = 0x01000000;
const G_TOP: u32 = 0x00010000;
const THRES: u32 = 0xFF000000;
const THREG: u32 = 0x00FF0000;

// ---------- Encoder ----------

pub struct RangeEncoder {
    lowc: u64,
    ff_num: u32,
    cache: u32,
    range: u32,
    buf: Vec<u8>,
    pos: usize,
    /// When `Some`, records the model probability assigned to each coded base,
    /// quantised so that stored value `q` means probability `q / 100000`.
    prob_log: Option<Vec<u32>>,
}

impl Default for RangeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl RangeEncoder {
    pub fn new() -> Self {
        Self {
            lowc: 0,
            ff_num: 0,
            cache: 0,
            range: 0xFFFFFFFF,
            buf: Vec::new(),
            pos: 0,
            prob_log: None,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        let buf = vec![0u8; cap];
        Self {
            lowc: 0,
            ff_num: 0,
            cache: 0,
            range: 0xFFFFFFFF,
            buf,
            pos: 0,
            prob_log: None,
        }
    }

    /// Start recording the per-base model probability at each coded base.
    pub fn enable_prob_log(&mut self) {
        self.prob_log = Some(Vec::new());
    }

    /// Whether per-base probability recording is enabled.
    #[inline(always)]
    pub fn is_capturing(&self) -> bool {
        self.prob_log.is_some()
    }

    /// Take the recorded per-base probabilities, if recording was enabled.
    /// Each stored `q` encodes probability `q / 100000`.
    pub fn take_prob_log(&mut self) -> Option<Vec<u32>> {
        self.prob_log.take()
    }

    /// If recording, log the probability `freq(sym)/total` that a cumulative
    /// table over `n` symbols assigns to `sym`, quantised to five decimals.
    #[inline(always)]
    pub fn log_base_prob(&mut self, cum: &[u32], n: usize, sym: usize) {
        if let Some(log) = &mut self.prob_log {
            let freq = (cum[sym + 1] - cum[sym]) as f64;
            let p = freq / cum[n] as f64;
            log.push((p * 100_000.0).round() as u32);
        }
    }

    /// Encode a symbol given cumulative frequency table.
    /// cum[0] must be 0. cum[n_symbols] is the total frequency.
    #[inline(always)]
    pub fn encode_symbol(&mut self, n_symbols: usize, cum: &[u32], symbol: usize) {
        unsafe {
            let cf = *cum.get_unchecked(symbol);
            let cf1 = *cum.get_unchecked(symbol + 1);
            let tot = *cum.get_unchecked(n_symbols);
            self.process(cf, cf1 - cf, tot);
        }
    }

    /// Muldiv encode: `a * range / totFreq` in 64-bit intermediate,
    /// non-overlapping intervals via +1.
    #[inline(always)]
    pub fn process(&mut self, cum_freq: u32, freq: u32, tot_freq: u32) {
        let r = self.range as u64;
        let t = tot_freq as u64;
        let tmp = (cum_freq as u64 * r / t) as u32 + 1;
        self.lowc = self.lowc.wrapping_add(tmp as u64);
        self.range = ((cum_freq + freq) as u64 * r / t) as u32 - tmp;
        self.renorm();
    }

    pub fn finish(&mut self) {
        for _ in 0..5 {
            self.shift_low();
        }
    }

    pub fn output(&self) -> &[u8] {
        &self.buf[..self.pos]
    }

    pub fn into_output(mut self) -> Vec<u8> {
        self.buf.truncate(self.pos);
        self.buf
    }

    pub fn reset(&mut self) {
        self.lowc = 0;
        self.ff_num = 0;
        self.cache = 0;
        self.range = 0xFFFFFFFF;
        self.pos = 0;
    }

    // -- internals --

    #[inline(always)]
    fn low(&self) -> u32 {
        self.lowc as u32
    }
    #[inline(always)]
    fn carry(&self) -> u32 {
        (self.lowc >> 32) as u32
    }
    #[inline(always)]
    fn set_low(&mut self, v: u32) {
        self.lowc = (self.lowc & 0xFFFFFFFF_00000000) | v as u64;
    }
    #[inline(always)]
    fn clear_carry(&mut self) {
        self.lowc &= 0x00000000_FFFFFFFF;
    }

    #[inline(always)]
    fn renorm(&mut self) {
        if self.range < G_TOP {
            self.range <<= 16;
            self.shift_low2();
        } else if self.range < S_TOP {
            self.range <<= 8;
            self.shift_low();
        }
    }

    #[inline(always)]
    fn push_byte(&mut self, b: u8) {
        if self.pos >= self.buf.len() {
            self.grow();
        }
        unsafe {
            *self.buf.get_unchecked_mut(self.pos) = b;
        }
        self.pos += 1;
    }

    #[cold]
    #[inline(never)]
    fn grow(&mut self) {
        let new_len = (self.buf.len() * 2).max(1024);
        self.buf.resize(new_len, 0);
    }

    fn shift_low(&mut self) {
        let low = self.low();
        let carry = self.carry();
        if low < THRES || carry != 0 {
            self.push_byte(self.cache.wrapping_add(carry) as u8);
            let fill = carry.wrapping_sub(1) as u8;
            for _ in 0..self.ff_num {
                self.push_byte(fill);
            }
            self.ff_num = 0;
            self.cache = low >> 24;
            self.clear_carry();
        } else {
            self.ff_num += 1;
        }
        self.set_low(low << 8);
    }

    fn shift_low2(&mut self) {
        let low = self.low();
        let carry = self.carry();
        if low < THRES || carry != 0 {
            self.push_byte(self.cache.wrapping_add(carry) as u8);
            let fill = carry.wrapping_sub(1) as u8;
            for _ in 0..self.ff_num {
                self.push_byte(fill);
            }
            self.ff_num = 0;
            self.cache = low >> 24;
            self.clear_carry();
        } else {
            self.ff_num += 1;
        }
        let low2 = low & (S_TOP - 1);
        if low2 < THREG {
            self.push_byte(self.cache as u8);
            for _ in 0..self.ff_num {
                self.push_byte(0xFF);
            }
            self.ff_num = 0;
            self.cache = low2 >> 16;
        } else {
            self.ff_num += 1;
        }
        self.set_low(low2 << 16);
    }
}

// ---------- Decoder ----------

pub struct RangeDecoder<'a> {
    code: u32,
    range: u32,
    data: &'a [u8],
    pos: usize,
}

impl<'a> RangeDecoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let mut dec = Self {
            code: 0,
            range: 0xFFFFFFFF,
            data,
            pos: 0,
        };
        for _ in 0..5 {
            dec.code = (dec.code << 8).wrapping_add(dec.read_byte() as u32);
        }
        dec
    }

    /// Decode a symbol given cumulative frequency table.
    /// cum[0] must be 0. cum[n_symbols] is the total frequency.
    #[inline(always)]
    pub fn decode_symbol(&mut self, n_symbols: usize, cum: &[u32]) -> usize {
        unsafe {
            let tot = *cum.get_unchecked(n_symbols);
            let r = self.range as u64;
            let t = tot as u64;
            let f = ((self.code as u64 * t / r) as u32).min(tot - 1);

            if n_symbols == 2 {
                let c1 = *cum.get_unchecked(1);
                if f < c1 {
                    let cf = *cum.get_unchecked(0);
                    self.decode_update(cf, c1 - cf, r, t);
                    return 0;
                } else {
                    let c2 = *cum.get_unchecked(2);
                    self.decode_update(c1, c2 - c1, r, t);
                    return 1;
                }
            }

            for i in 0..n_symbols {
                let ci = *cum.get_unchecked(i);
                let ci1 = *cum.get_unchecked(i + 1);
                if ci <= f && f < ci1 {
                    self.decode_update(ci, ci1 - ci, r, t);
                    return i;
                }
            }
            0
        }
    }

    /// Muldiv decode matching the encoder's interval arithmetic.
    #[inline(always)]
    fn decode_update(&mut self, cum_freq: u32, freq: u32, r: u64, t: u64) {
        let tmp = (cum_freq as u64 * r / t) as u32 + 1;
        self.code -= tmp;
        self.range = ((cum_freq + freq) as u64 * r / t) as u32 - tmp;
        self.renorm();
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    // -- internals --

    #[inline(always)]
    fn read_byte(&mut self) -> u8 {
        if self.pos < self.data.len() {
            let b = unsafe { *self.data.get_unchecked(self.pos) };
            self.pos += 1;
            b
        } else {
            self.pos += 1;
            0
        }
    }

    #[inline(always)]
    fn renorm(&mut self) {
        if self.range < G_TOP {
            self.range <<= 16;
            let b1 = self.read_byte();
            let b2 = self.read_byte();
            self.code = (self.code << 16).wrapping_add(((b1 as u32) << 8) + b2 as u32);
        } else if self.range < S_TOP {
            self.range <<= 8;
            let b = self.read_byte();
            self.code = (self.code << 8).wrapping_add(b as u32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_roundtrip(cum: &[u32], symbols: &[usize]) {
        let n = cum.len() - 1;
        let mut enc = RangeEncoder::new();
        for &s in symbols {
            enc.encode_symbol(n, cum, s);
        }
        enc.finish();
        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        let decoded: Vec<usize> = (0..symbols.len())
            .map(|_| dec.decode_symbol(n, cum))
            .collect();
        assert_eq!(symbols, &decoded[..]);
    }

    #[test]
    fn roundtrip_uniform_4() {
        test_roundtrip(&[0, 1, 2, 3, 4], &[0, 1, 2, 3, 0, 1, 2, 3, 3, 2, 1, 0]);
    }

    #[test]
    fn roundtrip_skewed_4() {
        test_roundtrip(
            &[0, 100, 110, 115, 120],
            &[0, 0, 0, 1, 0, 0, 2, 0, 3, 0, 0, 0, 0, 1, 0, 0],
        );
    }

    #[test]
    fn roundtrip_binary() {
        test_roundtrip(&[0, 3, 4], &[0, 0, 0, 1, 0, 1, 1, 0, 0, 0]);
    }

    #[test]
    fn roundtrip_12sym() {
        let mut cum = [0u32; 13];
        for i in 1..=12 {
            cum[i] = cum[i - 1] + 1;
        }
        let s: Vec<usize> = (0..200).map(|i| i % 12).collect();
        test_roundtrip(&cum, &s);
    }

    #[test]
    fn roundtrip_long() {
        let cum = [0u32, 5, 8, 11, 16];
        let s: Vec<usize> = (0..10000).map(|i| ((i * 7 + 3) % 4) as usize).collect();
        test_roundtrip(&cum, &s);
    }

    #[test]
    fn roundtrip_adaptive() {
        let symbols = [0usize, 1, 2, 3, 0, 0, 0, 1, 2, 0, 3, 3, 2, 1, 0, 0];
        let mut counts = [1u32; 4];
        let mut enc = RangeEncoder::new();
        for &s in &symbols {
            let mut cum = [0u32; 5];
            for i in 0..4 {
                cum[i + 1] = cum[i] + counts[i];
            }
            enc.encode_symbol(4, &cum, s);
            counts[s] += 1;
        }
        enc.finish();
        counts = [1u32; 4];
        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        for (j, &expected) in symbols.iter().enumerate() {
            let mut cum = [0u32; 5];
            for i in 0..4 {
                cum[i + 1] = cum[i] + counts[i];
            }
            let s = dec.decode_symbol(4, &cum);
            assert_eq!(s, expected, "j={}", j);
            counts[s] += 1;
        }
    }

    #[test]
    fn skewed_compresses() {
        let uniform_cum = [0u32, 1, 2, 3, 4];
        let skewed_cum = [0u32, 1000, 1001, 1002, 1003];
        let symbols: Vec<usize> = vec![0; 1000];

        let mut enc = RangeEncoder::new();
        for &s in &symbols {
            enc.encode_symbol(4, &uniform_cum, s);
        }
        enc.finish();
        let uniform_size = enc.output().len();

        enc.reset();
        for &s in &symbols {
            enc.encode_symbol(4, &skewed_cum, s);
        }
        enc.finish();
        let skewed_size = enc.output().len();

        assert!(
            skewed_size < uniform_size,
            "skewed ({}) should be smaller than uniform ({})",
            skewed_size,
            uniform_size
        );
    }

    #[test]
    fn reset_reuse() {
        let cum = [0u32, 1, 2, 3, 4];
        let s1 = [0, 1, 2, 3];
        let s2 = [3, 2, 1, 0, 0, 1];

        let mut enc = RangeEncoder::new();
        for &s in &s1 {
            enc.encode_symbol(4, &cum, s);
        }
        enc.finish();
        let e1 = enc.output().to_vec();
        enc.reset();
        for &s in &s2 {
            enc.encode_symbol(4, &cum, s);
        }
        enc.finish();
        let e2 = enc.output().to_vec();

        let mut dec = RangeDecoder::new(&e1);
        let d1: Vec<_> = (0..4).map(|_| dec.decode_symbol(4, &cum)).collect();
        assert_eq!(&s1[..], &d1[..]);

        let mut dec = RangeDecoder::new(&e2);
        let d2: Vec<_> = (0..6).map(|_| dec.decode_symbol(4, &cum)).collect();
        assert_eq!(&s2[..], &d2[..]);
    }
}
