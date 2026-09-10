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

use super::LOG_TABLE;
use memmap2::MmapMut;

const MODE_16: u8 = 0;
const MODE_8: u8 = 1;
const MODE_4: u8 = 2;

/// Zero-initialized buffer backed by anonymous mmap.
/// Pages are page-aligned (>= 4 KiB), satisfying any SIMD alignment needs.
/// On Linux, requests transparent huge pages via MADV_HUGEPAGE.
struct AlignedVec {
    ptr: *mut u8,
    mmap: MmapMut,
}

impl AlignedVec {
    fn new(size: usize) -> Self {
        assert!(size > 0);
        let mut mmap = MmapMut::map_anon(size).expect("anonymous mmap failed");
        let ptr = mmap.as_mut_ptr();
        #[cfg(target_os = "linux")]
        unsafe {
            libc::madvise(ptr as *mut libc::c_void, size, libc::MADV_HUGEPAGE);
        }
        Self { ptr, mmap }
    }

    fn fill_zero(&mut self) {
        #[cfg(target_os = "linux")]
        unsafe {
            libc::madvise(
                self.ptr as *mut libc::c_void,
                self.mmap.len(),
                libc::MADV_DONTNEED,
            );
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.mmap.fill(0);
        }
    }
}

// Safety: the raw pointer is exclusively owned (like Box) and the
// underlying mmap is not shared.
unsafe impl Send for AlignedVec {}
unsafe impl Sync for AlignedVec {}

pub struct MarkovModel<
    const ORDER: usize,
    const ALPHA_DEN: u32,
    const UPDATE_REG: bool,
    const UPDATE_RC: bool,
    const MAX_COUNT: u32,
> {
    data: AlignedVec,
    pub ctx: u32,
    pub ctx_rc: u32,
}

impl<
        const ORDER: usize,
        const ALPHA_DEN: u32,
        const UPDATE_REG: bool,
        const UPDATE_RC: bool,
        const MAX_COUNT: u32,
    > Default for MarkovModel<ORDER, ALPHA_DEN, UPDATE_REG, UPDATE_RC, MAX_COUNT>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<
        const ORDER: usize,
        const ALPHA_DEN: u32,
        const UPDATE_REG: bool,
        const UPDATE_RC: bool,
        const MAX_COUNT: u32,
    > MarkovModel<ORDER, ALPHA_DEN, UPDATE_REG, UPDATE_RC, MAX_COUNT>
{
    const CTX_MAX: u64 = 4u64.pow(ORDER as u32);
    const STORAGE_MODE: u8 = if ORDER >= 14 {
        MODE_4
    } else if Self::CTX_MAX > 1_000_000 {
        MODE_8
    } else {
        MODE_16
    };
    const EFFECTIVE_MAX_COUNT: u32 = if ORDER >= 14 {
        if MAX_COUNT < 15 {
            MAX_COUNT
        } else {
            15
        }
    } else if Self::CTX_MAX > 1_000_000 {
        if MAX_COUNT < 255 {
            MAX_COUNT
        } else {
            255
        }
    } else if MAX_COUNT < 65535 {
        MAX_COUNT
    } else {
        65535
    };
    const CTX_SHIFT: usize = 2 * (ORDER - 1);
    const CTX_MASK: u32 = if Self::CTX_SHIFT < 32 {
        (1u32 << Self::CTX_SHIFT) - 1
    } else {
        u32::MAX
    };
    const ALPHA_MUL: u32 = 1 << ALPHA_DEN;
    const USE_COMPACT_LAST_SYMBOL: bool =
        Self::STORAGE_MODE == MODE_8 && Self::EFFECTIVE_MAX_COUNT == 0;

    /// Byte count for the allocation.
    const ALLOC_SIZE: usize = if Self::USE_COMPACT_LAST_SYMBOL {
        Self::CTX_MAX as usize // 0 = unseen, 1..4 = last symbol
    } else if Self::STORAGE_MODE == MODE_16 {
        Self::CTX_MAX as usize * 4 * 2 // 4 u16s per context
    } else if Self::STORAGE_MODE == MODE_8 {
        Self::CTX_MAX as usize * 4 // 4 u8s per context
    } else {
        Self::CTX_MAX as usize * 2 // 2 packed nibble bytes per context
    };

    pub fn new() -> Self {
        Self {
            data: AlignedVec::new(Self::ALLOC_SIZE),
            ctx: 0,
            ctx_rc: Self::CTX_MAX as u32 - 1,
        }
    }

    /// Shift in a symbol to the forward context.
    #[inline(always)]
    pub fn update_ctx(&mut self, symbol: u8) {
        self.ctx = ((self.ctx & Self::CTX_MASK) << 2) + symbol as u32;
    }

    /// Shift in a symbol to the reverse context.
    #[inline(always)]
    pub fn update_ctx_rc(&mut self, symbol: u8) {
        self.ctx_rc = (self.ctx_rc >> 2) + (((symbol ^ 1) as u32) << Self::CTX_SHIFT);
    }

    /// Fill cum[1..=4] using a caller-provided `alpha_mul` instead of the
    /// type's ALPHA_DEN.  Used by bmarkov where two logical models share
    /// the same count table but differ in alpha_den.
    #[inline(always)]
    pub fn predict_with_alpha_mul(&self, alpha_mul: u32, cum: &mut [u32]) {
        if Self::STORAGE_MODE == MODE_16 {
            let c = self.data.ptr as *const u16;
            let base = (self.ctx as usize) * 4;
            unsafe {
                Self::predict_inner_dyn(
                    alpha_mul,
                    *c.add(base) as u32,
                    *c.add(base + 1) as u32,
                    *c.add(base + 2) as u32,
                    *c.add(base + 3) as u32,
                    cum,
                );
            }
        } else if Self::STORAGE_MODE == MODE_8 {
            let c = self.data.ptr;
            let base = (self.ctx as usize) * 4;
            unsafe {
                Self::predict_inner_dyn(
                    alpha_mul,
                    *c.add(base) as u32,
                    *c.add(base + 1) as u32,
                    *c.add(base + 2) as u32,
                    *c.add(base + 3) as u32,
                    cum,
                );
            }
        } else {
            let c = self.data.ptr;
            let base = (self.ctx as usize) * 2;
            unsafe {
                let b0 = *c.add(base);
                let b1 = *c.add(base + 1);
                Self::predict_inner_dyn(
                    alpha_mul,
                    (b0 & 0x0F) as u32,
                    ((b0 >> 4) & 0x0F) as u32,
                    (b1 & 0x0F) as u32,
                    ((b1 >> 4) & 0x0F) as u32,
                    cum,
                );
            }
        }
    }

    #[inline(always)]
    unsafe fn predict_inner_dyn(
        alpha_mul: u32,
        c0: u32,
        c1: u32,
        c2: u32,
        c3: u32,
        cum: &mut [u32],
    ) {
        let f0 = 1 + alpha_mul * c0;
        let f1 = 1 + alpha_mul * c1;
        let f2 = 1 + alpha_mul * c2;
        let f3 = 1 + alpha_mul * c3;
        *cum.get_unchecked_mut(1) = f0;
        *cum.get_unchecked_mut(2) = f0 + f1;
        *cum.get_unchecked_mut(3) = f0 + f1 + f2;
        *cum.get_unchecked_mut(4) = f0 + f1 + f2 + f3;
    }

    /// Coding cost computed with a caller-provided `alpha_mul`.
    #[inline(always)]
    pub fn coding_cost_with_alpha_mul(&self, alpha_mul: u32, symbol: usize) -> u32 {
        let (f0, f1, f2, f3) = if Self::STORAGE_MODE == MODE_16 {
            let c = self.data.ptr as *const u16;
            let base = (self.ctx as usize) * 4;
            unsafe {
                (
                    1 + alpha_mul * (*c.add(base) as u32),
                    1 + alpha_mul * (*c.add(base + 1) as u32),
                    1 + alpha_mul * (*c.add(base + 2) as u32),
                    1 + alpha_mul * (*c.add(base + 3) as u32),
                )
            }
        } else if Self::STORAGE_MODE == MODE_8 {
            let c = self.data.ptr;
            let base = (self.ctx as usize) * 4;
            unsafe {
                (
                    1 + alpha_mul * (*c.add(base) as u32),
                    1 + alpha_mul * (*c.add(base + 1) as u32),
                    1 + alpha_mul * (*c.add(base + 2) as u32),
                    1 + alpha_mul * (*c.add(base + 3) as u32),
                )
            }
        } else {
            let c = self.data.ptr;
            let base = (self.ctx as usize) * 2;
            unsafe {
                let b0 = *c.add(base);
                let b1 = *c.add(base + 1);
                (
                    1 + alpha_mul * ((b0 & 0x0F) as u32),
                    1 + alpha_mul * (((b0 >> 4) & 0x0F) as u32),
                    1 + alpha_mul * ((b1 & 0x0F) as u32),
                    1 + alpha_mul * (((b1 >> 4) & 0x0F) as u32),
                )
            }
        };
        let own_freq = [f0, f1, f2, f3];
        let sum = f0 + f1 + f2 + f3;
        unsafe {
            (*LOG_TABLE.get_unchecked(sum as usize)
                - *LOG_TABLE.get_unchecked(*own_freq.get_unchecked(symbol) as usize))
                as u32
        }
    }

    /// Fill cum[1..=4] with cumulative frequencies for the 4 symbols
    /// based on the current forward context. cum[0] must already be 0.
    #[inline(always)]
    pub fn predict(&self, cum: &mut [u32]) {
        // All branches on STORAGE_MODE are compile-time eliminated since it's a const.
        if Self::USE_COMPACT_LAST_SYMBOL {
            let c = self.data.ptr;
            unsafe {
                let state = *c.add(self.ctx as usize);
                debug_assert!(state <= 4);
                Self::predict_inner(
                    (state == 1) as u32,
                    (state == 2) as u32,
                    (state == 3) as u32,
                    (state == 4) as u32,
                    cum,
                );
            }
        } else if Self::STORAGE_MODE == MODE_16 {
            let c = self.data.ptr as *const u16;
            let base = (self.ctx as usize) * 4;
            unsafe {
                Self::predict_inner(
                    *c.add(base) as u32,
                    *c.add(base + 1) as u32,
                    *c.add(base + 2) as u32,
                    *c.add(base + 3) as u32,
                    cum,
                );
            }
        } else if Self::STORAGE_MODE == MODE_8 {
            let c = self.data.ptr;
            let base = (self.ctx as usize) * 4;
            unsafe {
                Self::predict_inner(
                    *c.add(base) as u32,
                    *c.add(base + 1) as u32,
                    *c.add(base + 2) as u32,
                    *c.add(base + 3) as u32,
                    cum,
                );
            }
        } else {
            let c = self.data.ptr;
            let base = (self.ctx as usize) * 2;
            unsafe {
                let b0 = *c.add(base);
                let b1 = *c.add(base + 1);
                Self::predict_inner(
                    (b0 & 0x0F) as u32,
                    ((b0 >> 4) & 0x0F) as u32,
                    (b1 & 0x0F) as u32,
                    ((b1 >> 4) & 0x0F) as u32,
                    cum,
                );
            }
        }
    }

    /// Shared predict logic: given 4 raw counts, write cumulative frequencies
    /// into cum[1..=4]. Pure scalar -- the 4-wide prefix sum has too little
    /// parallelism to benefit from SSE (3-add dependency chain).
    #[inline(always)]
    unsafe fn predict_inner(c0: u32, c1: u32, c2: u32, c3: u32, cum: &mut [u32]) {
        let f0 = 1 + Self::ALPHA_MUL * c0;
        let f1 = 1 + Self::ALPHA_MUL * c1;
        let f2 = 1 + Self::ALPHA_MUL * c2;
        let f3 = 1 + Self::ALPHA_MUL * c3;
        *cum.get_unchecked_mut(1) = f0;
        *cum.get_unchecked_mut(2) = f0 + f1;
        *cum.get_unchecked_mut(3) = f0 + f1 + f2;
        *cum.get_unchecked_mut(4) = f0 + f1 + f2 + f3;
    }

    /// Compute the information content (in scaled nats) for encoding
    /// `symbol` in the current forward context.
    /// Returns ln(total_freq / symbol_freq) * 1_000_000.
    #[inline(always)]
    pub fn coding_cost(&self, symbol: usize) -> u32 {
        if Self::USE_COMPACT_LAST_SYMBOL {
            let c = self.data.ptr;
            unsafe {
                let state = *c.add(self.ctx as usize);
                debug_assert!(state <= 4);
                let own = if state == (symbol as u8 + 1) {
                    1 + Self::ALPHA_MUL
                } else {
                    1
                };
                let sum = if state == 0 { 4 } else { 4 + Self::ALPHA_MUL };
                return (*LOG_TABLE.get_unchecked(sum as usize)
                    - *LOG_TABLE.get_unchecked(own as usize)) as u32;
            }
        }

        let (f0, f1, f2, f3) = if Self::STORAGE_MODE == MODE_16 {
            let c = self.data.ptr as *const u16;
            let base = (self.ctx as usize) * 4;
            unsafe {
                (
                    1 + Self::ALPHA_MUL * (*c.add(base) as u32),
                    1 + Self::ALPHA_MUL * (*c.add(base + 1) as u32),
                    1 + Self::ALPHA_MUL * (*c.add(base + 2) as u32),
                    1 + Self::ALPHA_MUL * (*c.add(base + 3) as u32),
                )
            }
        } else if Self::STORAGE_MODE == MODE_8 {
            let c = self.data.ptr;
            let base = (self.ctx as usize) * 4;
            unsafe {
                (
                    1 + Self::ALPHA_MUL * (*c.add(base) as u32),
                    1 + Self::ALPHA_MUL * (*c.add(base + 1) as u32),
                    1 + Self::ALPHA_MUL * (*c.add(base + 2) as u32),
                    1 + Self::ALPHA_MUL * (*c.add(base + 3) as u32),
                )
            }
        } else {
            let c = self.data.ptr;
            let base = (self.ctx as usize) * 2;
            unsafe {
                let b0 = *c.add(base);
                let b1 = *c.add(base + 1);
                (
                    1 + Self::ALPHA_MUL * ((b0 & 0x0F) as u32),
                    1 + Self::ALPHA_MUL * (((b0 >> 4) & 0x0F) as u32),
                    1 + Self::ALPHA_MUL * ((b1 & 0x0F) as u32),
                    1 + Self::ALPHA_MUL * (((b1 >> 4) & 0x0F) as u32),
                )
            }
        };
        let own_freq = [f0, f1, f2, f3];
        let sum = f0 + f1 + f2 + f3;
        unsafe {
            (*LOG_TABLE.get_unchecked(sum as usize)
                - *LOG_TABLE.get_unchecked(*own_freq.get_unchecked(symbol) as usize))
                as u32
        }
    }

    /// Update the frequency count for `symbol` at context `ctx`.
    /// The context is passed explicitly because the caller may use
    /// either self.ctx (forward) or self.ctx_rc (reverse).
    #[inline(always)]
    pub fn update(&mut self, ctx: u32, symbol: usize) {
        if Self::USE_COMPACT_LAST_SYMBOL {
            let c = self.data.ptr;
            unsafe {
                *c.add(ctx as usize) = symbol as u8 + 1;
            }
        } else if Self::STORAGE_MODE == MODE_16 {
            let c = self.data.ptr as *mut u16;
            let base = (ctx as usize) * 4;
            unsafe {
                if Self::EFFECTIVE_MAX_COUNT == 0 {
                    *c.add(base) = 0;
                    *c.add(base + 1) = 0;
                    *c.add(base + 2) = 0;
                    *c.add(base + 3) = 0;
                } else if *c.add(base + symbol) == Self::EFFECTIVE_MAX_COUNT as u16 {
                    *c.add(base) >>= 1;
                    *c.add(base + 1) >>= 1;
                    *c.add(base + 2) >>= 1;
                    *c.add(base + 3) >>= 1;
                }
                *c.add(base + symbol) += 1;
            }
        } else if Self::STORAGE_MODE == MODE_8 {
            let c = self.data.ptr;
            let base = (ctx as usize) * 4;
            unsafe {
                if Self::EFFECTIVE_MAX_COUNT == 0 {
                    *c.add(base) = 0;
                    *c.add(base + 1) = 0;
                    *c.add(base + 2) = 0;
                    *c.add(base + 3) = 0;
                } else if *c.add(base + symbol) == Self::EFFECTIVE_MAX_COUNT as u8 {
                    *c.add(base) >>= 1;
                    *c.add(base + 1) >>= 1;
                    *c.add(base + 2) >>= 1;
                    *c.add(base + 3) >>= 1;
                }
                *c.add(base + symbol) += 1;
            }
        } else {
            let c = self.data.ptr;
            let base = (ctx as usize) * 2;
            let masks: [u8; 4] = [0x0F, 0xF0, 0x0F, 0xF0];
            unsafe {
                let byte_idx = base + (symbol >> 1);
                if (*c.add(byte_idx) & masks[symbol]) == masks[symbol] {
                    *c.add(base) = (*c.add(base) >> 1) & 0x77;
                    *c.add(base + 1) = (*c.add(base + 1) >> 1) & 0x77;
                }
                match symbol {
                    0 => *c.add(base) += 1,
                    1 => *c.add(base) += 16,
                    2 => *c.add(base + 1) += 1,
                    3 => *c.add(base + 1) += 16,
                    _ => core::hint::unreachable_unchecked(),
                }
            }
        }
    }

    /// Full update for when this model is chosen as best.
    /// Always updates counts + forward context; also updates RC if UPDATE_RC.
    #[inline(always)]
    pub fn update_as_best(&mut self, s: u8) {
        self.update(self.ctx, s as usize);
        self.update_ctx(s);
        if UPDATE_RC {
            let rcs = (self.ctx_rc & 3) as usize;
            self.update_ctx_rc(s);
            self.update(self.ctx_rc, rcs);
        }
    }

    /// Update for when this model is NOT chosen as best.
    /// UPDATE_REG=true: full count update + context.
    /// UPDATE_REG=false: context only.
    #[inline(always)]
    pub fn update_as_nonbest(&mut self, s: u8) {
        if UPDATE_REG {
            self.update(self.ctx, s as usize);
        }
        self.update_ctx(s);
        if UPDATE_RC {
            if UPDATE_REG {
                let rcs = (self.ctx_rc & 3) as usize;
                self.update_ctx_rc(s);
                self.update(self.ctx_rc, rcs);
            } else {
                self.update_ctx_rc(s);
            }
        }
    }

    pub fn reset(&mut self) {
        self.ctx = 0;
        self.ctx_rc = Self::CTX_MAX as u32 - 1;
        self.data.fill_zero();
    }

    // Accessors for testing / inspection

    pub const fn order() -> usize {
        ORDER
    }

    pub const fn alpha_den() -> u32 {
        ALPHA_DEN
    }

    pub const fn update_reg_enabled() -> bool {
        UPDATE_REG
    }

    pub const fn update_rc_enabled() -> bool {
        UPDATE_RC
    }

    pub const fn configured_max_count() -> u32 {
        MAX_COUNT
    }

    pub const fn alloc_size_bytes() -> usize {
        Self::ALLOC_SIZE
    }

    pub const fn ctx_max() -> u64 {
        Self::CTX_MAX
    }

    pub const fn storage_mode() -> u8 {
        Self::STORAGE_MODE
    }

    pub const fn effective_max_count() -> u32 {
        Self::EFFECTIVE_MAX_COUNT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markov::rangecoder::{RangeDecoder, RangeEncoder};

    type Model3 = MarkovModel<3, 0, false, false, 65536>;
    type Model5 = MarkovModel<5, 0, false, false, 4095>;
    type Model7 = MarkovModel<7, 0, false, false, 1023>;
    type Model9 = MarkovModel<9, 1, false, false, 511>;
    type Model11 = MarkovModel<11, 2, true, true, 255>;
    type Model15 = MarkovModel<15, 6, true, true, 15>;
    type Model13 = MarkovModel<13, 9, true, true, 0>;

    // ---- Storage mode selection ----

    #[test]
    fn storage_mode_16_for_small_order() {
        assert_eq!(Model3::storage_mode(), MODE_16);
        assert_eq!(Model5::storage_mode(), MODE_16);
        assert_eq!(Model7::storage_mode(), MODE_16);
        assert_eq!(Model9::storage_mode(), MODE_16);
    }

    #[test]
    fn storage_mode_8_for_medium_order() {
        // 4^11 = 4194304 > 1_000_000 but Order < 14
        assert_eq!(Model11::storage_mode(), MODE_8);
        assert_eq!(Model13::storage_mode(), MODE_8);
    }

    #[test]
    fn storage_mode_4_for_large_order() {
        // Order >= 14
        assert_eq!(Model15::storage_mode(), MODE_4);
    }

    // ---- Context max ----

    #[test]
    fn ctx_max_values() {
        assert_eq!(Model3::ctx_max(), 64); // 4^3
        assert_eq!(Model5::ctx_max(), 1024); // 4^5
        assert_eq!(Model7::ctx_max(), 16384); // 4^7
        assert_eq!(Model9::ctx_max(), 262144); // 4^9
    }

    // ---- Max count clamping ----

    #[test]
    fn max_count_clamping() {
        assert_eq!(Model3::effective_max_count(), 65535); // min(65536, 65535)
        assert_eq!(Model5::effective_max_count(), 4095);
        assert_eq!(Model7::effective_max_count(), 1023);
        assert_eq!(Model9::effective_max_count(), 511);
        assert_eq!(Model11::effective_max_count(), 255);
        assert_eq!(Model15::effective_max_count(), 15);
        assert_eq!(Model13::effective_max_count(), 0);
    }

    // ---- Predict (fresh model) ----

    #[test]
    fn predict_fresh_uniform() {
        let model = Model3::new();
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // All counts are 0, so freq = 1 + 0 = 1 for each symbol.
        // AlphaDen=0, so ALPHA_MUL=1.
        assert_eq!(cum, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn predict_fresh_alpha_den_1() {
        // AlphaDen=1 -> ALPHA_MUL=2. Fresh: freq = 1 + 2*0 = 1.
        let model = Model9::new();
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        assert_eq!(cum, [0, 1, 2, 3, 4]);
    }

    // ---- Update + Predict ----

    #[test]
    fn update_changes_prediction() {
        let mut model = Model3::new();
        // Update symbol 0 at context 0
        model.update(0, 0);
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // count[0]=1, others=0. AlphaDen=0 -> ALPHA_MUL=1.
        // freq[0] = 1 + 1*1 = 2, freq[1..3] = 1 + 1*0 = 1.
        assert_eq!(cum, [0, 2, 3, 4, 5]);
    }

    #[test]
    fn multiple_updates() {
        let mut model = Model3::new();
        for _ in 0..10 {
            model.update(0, 0);
        }
        for _ in 0..5 {
            model.update(0, 2);
        }
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // AlphaDen=0, ALPHA_MUL=1.
        // count[0]=10, count[1]=0, count[2]=5, count[3]=0.
        // freq: 11, 1, 6, 1. cum: 0, 11, 12, 18, 19.
        assert_eq!(cum, [0, 11, 12, 18, 19]);
    }

    // ---- Context update ----

    #[test]
    fn update_ctx_shifts_correctly() {
        let mut model = Model3::new();
        // Order=3, CTX_SHIFT=4, CTX_MASK=0xF.
        // Initially ctx=0.
        model.update_ctx(2); // ctx = ((0 & 0xF) << 2) + 2 = 2
        assert_eq!(model.ctx, 2);
        model.update_ctx(3); // ctx = ((2 & 0xF) << 2) + 3 = 11
        assert_eq!(model.ctx, 11);
        model.update_ctx(1); // ctx = ((11 & 0xF) << 2) + 1 = 45
        assert_eq!(model.ctx, 45);
        // After 3 symbols, we have a full order-3 context.
        model.update_ctx(0); // ctx = ((45 & 0xF) << 2) + 0 = (13 << 2) = 52
        assert_eq!(model.ctx, 52);
    }

    #[test]
    fn update_ctx_rc_shifts_correctly() {
        let mut model = Model3::new();
        // Order=3, CTX_SHIFT=4.
        // Initially ctx_rc = 4^3 - 1 = 63.
        model.update_ctx_rc(0); // ctx_rc = (63 >> 2) + ((0^1) << 4) = 15 + 16 = 31
        assert_eq!(model.ctx_rc, 31);
        model.update_ctx_rc(1); // ctx_rc = (31 >> 2) + ((1^1) << 4) = 7 + 0 = 7
        assert_eq!(model.ctx_rc, 7);
    }

    // ---- Coding cost ----

    #[test]
    fn coding_cost_uniform() {
        let model = Model3::new();
        // All counts 0, so freq = [1,1,1,1], sum = 4.
        // cost = logTable[4] - logTable[1] = 1386294 - 0 = 1386294.
        // This is ln(4) * 1M = 2 * ln(2) * 1M.
        let cost = model.coding_cost(0);
        assert_eq!(cost, 1386294);
        // All symbols have the same cost in a uniform distribution.
        assert_eq!(model.coding_cost(1), cost);
        assert_eq!(model.coding_cost(2), cost);
        assert_eq!(model.coding_cost(3), cost);
    }

    #[test]
    fn coding_cost_after_updates() {
        let mut model = Model3::new();
        model.update(0, 0);
        model.update(0, 0);
        model.update(0, 0);
        // counts at ctx 0: [3, 0, 0, 0]. AlphaDen=0.
        // freq: [4, 1, 1, 1], sum = 7.
        let cost_0 = model.coding_cost(0);
        let cost_1 = model.coding_cost(1);
        // cost_0 = logTable[7] - logTable[4] = ln(7/4) * 1M
        // cost_1 = logTable[7] - logTable[1] = ln(7) * 1M
        assert!(cost_0 < cost_1, "frequent symbol should have lower cost");
        let expected_0 = (LOG_TABLE[7] - LOG_TABLE[4]) as u32;
        assert_eq!(cost_0, expected_0);
    }

    // ---- Mode 16: count halving ----

    #[test]
    fn mode16_halving_on_max_count() {
        // MaxCount=1023, clamped to 1023.
        let mut model = Model7::new();
        // Fill symbol 0 to max count
        for _ in 0..1023 {
            model.update(0, 0);
        }
        // Now count[0] = 1023 = mMaxCount. Next update triggers halving.
        let mut cum_before = [0u32; 5];
        model.predict(&mut cum_before);
        // freq[0] = 1 + 1*1023 = 1024. Others = 1.
        assert_eq!(cum_before[1], 1024);

        model.update(0, 0); // triggers halve then increment
        let mut cum_after = [0u32; 5];
        model.predict(&mut cum_after);
        // After halving: count[0] = 1023>>1 = 511, then +1 = 512.
        // freq[0] = 1 + 512 = 513.
        assert_eq!(cum_after[1], 513);
    }

    // ---- Mode 8 ----

    #[test]
    fn mode8_construction_and_predict() {
        let model = Model11::new();
        assert_eq!(Model11::storage_mode(), MODE_8);
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // AlphaDen=2, ALPHA_MUL=4. Fresh: freq = 1 + 4*0 = 1.
        assert_eq!(cum, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn mode8_update_and_predict() {
        let mut model = Model11::new();
        model.update(0, 1);
        model.update(0, 1);
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // AlphaDen=2, ALPHA_MUL=4. count[1]=2, others=0.
        // freq: [1, 1+4*2, 1, 1] = [1, 9, 1, 1]. cum: [0, 1, 10, 11, 12].
        assert_eq!(cum, [0, 1, 10, 11, 12]);
    }

    // ---- Mode 4 ----

    #[test]
    fn mode4_construction_and_predict() {
        let model = Model15::new();
        assert_eq!(Model15::storage_mode(), MODE_4);
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // AlphaDen=6, ALPHA_MUL=64. Fresh: freq = 1 + 64*0 = 1.
        assert_eq!(cum, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn mode4_update_all_symbols() {
        let mut model = Model15::new();
        model.update(0, 0);
        model.update(0, 1);
        model.update(0, 2);
        model.update(0, 3);
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // AlphaDen=6, ALPHA_MUL=64. All counts = 1.
        // freq = 1 + 64*1 = 65 for each.
        assert_eq!(cum, [0, 65, 130, 195, 260]);
    }

    #[test]
    fn mode4_halving() {
        let mut model = Model15::new();
        // MaxCount = 15 for MODE_4. Fill symbol 0 to 15.
        for _ in 0..15 {
            model.update(0, 0);
        }
        // count[0] = 15 = max. Next update halves.
        model.update(0, 0);
        // After halving: 15 >> 1 = 7 (with & 0x77 masking), then +1 = 8.
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // freq[0] = 1 + 64 * 8 = 513.
        assert_eq!(cum[1], 513);
    }

    // ---- MaxCount = 0 (memset behavior) ----

    #[test]
    fn max_count_zero_clears_on_update() {
        let mut model = Model13::new();
        assert_eq!(Model13::effective_max_count(), 0);
        // Update symbol 0 multiple times
        model.update(0, 0);
        model.update(0, 0);
        model.update(0, 0);
        // With MaxCount=0, each update zeros all counts then increments.
        // So only the last update's symbol has count 1.
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        // AlphaDen=9, ALPHA_MUL=512. count[0]=1, others=0.
        // freq: [1+512, 1, 1, 1] = [513, 1, 1, 1].
        assert_eq!(cum, [0, 513, 514, 515, 516]);

        // Update a different symbol
        model.update(0, 2);
        model.predict(&mut cum);
        // zeros all, then count[2]=1.
        // freq: [1, 1, 513, 1].
        assert_eq!(cum, [0, 1, 2, 515, 516]);
    }

    // ---- Reset ----

    #[test]
    fn reset_clears_state() {
        let mut model = Model3::new();
        model.update(0, 0);
        model.update(0, 1);
        model.update_ctx(2);
        model.update_ctx_rc(1);

        model.reset();

        assert_eq!(model.ctx, 0);
        assert_eq!(model.ctx_rc, Model3::ctx_max() as u32 - 1);
        let mut cum = [0u32; 5];
        model.predict(&mut cum);
        assert_eq!(cum, [0, 1, 2, 3, 4]);
    }

    // ---- Integration: range coder + model ----

    #[test]
    fn rangecoder_model_roundtrip() {
        let input: Vec<u8> = vec![
            0, 1, 2, 3, 0, 0, 1, 3, 2, 0, 1, 1, 0, 3, 2, 1, 0, 0, 0, 1, 2, 3, 3, 2, 1, 0, 0, 2, 1,
            3, 0, 1,
        ];

        // Encode
        let mut enc = RangeEncoder::new();
        let mut model = Model3::new();
        for &s in &input {
            let mut cum = [0u32; 5];
            model.predict(&mut cum);
            enc.encode_symbol(4, &cum, s as usize);
            model.update(model.ctx, s as usize);
            model.update_ctx(s);
        }
        enc.finish();

        // Decode
        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        let mut model = Model3::new();
        let mut decoded = Vec::new();
        for _ in 0..input.len() {
            let mut cum = [0u32; 5];
            model.predict(&mut cum);
            let s = dec.decode_symbol(4, &cum);
            model.update(model.ctx, s);
            model.update_ctx(s as u8);
            decoded.push(s as u8);
        }
        assert_eq!(input, decoded);
    }

    #[test]
    fn rangecoder_model_roundtrip_with_rc_context() {
        // Test with UPDATE_RC=true (reverse context updates)
        type ModelRC = MarkovModel<3, 0, true, true, 65536>;

        let input: Vec<u8> = vec![0, 1, 2, 3, 3, 2, 1, 0, 0, 1, 0, 2, 3, 1, 2, 0];

        // Encode
        let mut enc = RangeEncoder::new();
        let mut model = ModelRC::new();
        for &s in &input {
            let mut cum = [0u32; 5];
            model.predict(&mut cum);
            enc.encode_symbol(4, &cum, s as usize);
            // Forward update
            model.update(model.ctx, s as usize);
            model.update_ctx(s);
            // Reverse update
            let rcs = (model.ctx_rc & 3) as usize;
            model.update_ctx_rc(s);
            model.update(model.ctx_rc, rcs);
        }
        enc.finish();

        // Decode
        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        let mut model = ModelRC::new();
        let mut decoded = Vec::new();
        for _ in 0..input.len() {
            let mut cum = [0u32; 5];
            model.predict(&mut cum);
            let s = dec.decode_symbol(4, &cum);
            model.update(model.ctx, s);
            model.update_ctx(s as u8);
            let rcs = (model.ctx_rc & 3) as usize;
            model.update_ctx_rc(s as u8);
            model.update(model.ctx_rc, rcs);
            decoded.push(s as u8);
        }
        assert_eq!(input, decoded);
    }

    #[test]
    fn rangecoder_model5_roundtrip() {
        // Higher-order model
        let input: Vec<u8> = (0..200).map(|i| ((i * 3 + 1) % 4) as u8).collect();

        let mut enc = RangeEncoder::new();
        let mut model = Model5::new();
        for &s in &input {
            let mut cum = [0u32; 5];
            model.predict(&mut cum);
            enc.encode_symbol(4, &cum, s as usize);
            model.update(model.ctx, s as usize);
            model.update_ctx(s);
        }
        enc.finish();

        let encoded = enc.output();
        let mut dec = RangeDecoder::new(encoded);
        let mut model = Model5::new();
        let mut decoded = Vec::new();
        for _ in 0..input.len() {
            let mut cum = [0u32; 5];
            model.predict(&mut cum);
            let s = dec.decode_symbol(4, &cum);
            model.update(model.ctx, s);
            model.update_ctx(s as u8);
            decoded.push(s as u8);
        }
        assert_eq!(input, decoded);
    }

    // ---- Log table sanity ----

    #[test]
    fn log_table_known_values() {
        assert_eq!(LOG_TABLE[0], 0);
        assert_eq!(LOG_TABLE[1], 0);
        assert_eq!(LOG_TABLE[2], 693147);
        assert_eq!(LOG_TABLE[3], 1098612);
        assert_eq!(LOG_TABLE[4], 1386294);
        assert_eq!(LOG_TABLE[10], 2302585);
        // ln(100) * 1M = 4605170
        assert_eq!(LOG_TABLE[100], 4605170);
    }
}
