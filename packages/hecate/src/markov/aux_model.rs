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

// AuxModel -- order-5 context model with variable alphabet size.
// Used for model selection, block type classification, and unknown symbol handling.

pub struct AuxModel<const N_MODELS: usize> {
    c: Box<[u8]>,
    ctx: u32,
}

impl<const N_MODELS: usize> AuxModel<N_MODELS> {
    const ORDER: u32 = 5;
    // Round N_MODELS up to the next power of 2 (2, 4, 8, or 16).
    const N_MODELS_UP: usize = if N_MODELS <= 2 {
        2
    } else if N_MODELS <= 4 {
        4
    } else if N_MODELS <= 8 {
        8
    } else {
        16
    };
    const N_MODELS_UP_LOG: usize = if N_MODELS <= 2 {
        1
    } else if N_MODELS <= 4 {
        2
    } else if N_MODELS <= 8 {
        3
    } else {
        4
    };
    const MULTIPLIER: usize = Self::N_MODELS_UP.pow(Self::ORDER - 1);
    const CTX_MAX: usize = Self::N_MODELS_UP.pow(Self::ORDER);
    const ALLOC_SIZE: usize = Self::CTX_MAX << Self::N_MODELS_UP_LOG;

    pub fn new() -> Self {
        Self {
            c: vec![0u8; Self::ALLOC_SIZE].into_boxed_slice(),
            ctx: 0,
        }
    }

    pub const fn alloc_size_bytes() -> usize {
        Self::ALLOC_SIZE
    }

    /// Fill cum[1..=N_MODELS] given cum[0] = 0.
    #[inline(always)]
    pub fn predict(&self, cum: &mut [u32]) {
        let base = (self.ctx as usize) << Self::N_MODELS_UP_LOG;
        unsafe {
            for symbol in 0..N_MODELS {
                *cum.get_unchecked_mut(symbol + 1) =
                    *cum.get_unchecked(symbol) + 1 + *self.c.get_unchecked(base + symbol) as u32;
            }
        }
    }

    #[inline(always)]
    pub fn update(&mut self, symbol: usize) {
        let base = (self.ctx as usize) << Self::N_MODELS_UP_LOG;
        unsafe {
            if *self.c.get_unchecked(base + symbol) == 255 {
                for n in 0..Self::N_MODELS_UP {
                    *self.c.get_unchecked_mut(base + n) >>= 1;
                }
            }
            *self.c.get_unchecked_mut(base + symbol) += 1;
        }
        self.ctx = (((self.ctx as usize) & (Self::MULTIPLIER - 1)) << Self::N_MODELS_UP_LOG) as u32
            + symbol as u32;
    }

    pub fn reset(&mut self) {
        self.ctx = 0;
        self.c.fill(0);
    }
}

impl<const N_MODELS: usize> Default for AuxModel<N_MODELS> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predict_fresh_uniform() {
        let m = AuxModel::<4>::new();
        let mut cum = [0u32; 5];
        m.predict(&mut cum);
        // All counts 0, freq = 1 each.
        assert_eq!(cum, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn update_changes_prediction() {
        let mut m = AuxModel::<4>::new();
        m.update(0);
        m.update(0);
        let mut cum = [0u32; 5];
        // Context has shifted, but ctx=0 was updated before the shift.
        // Reset ctx to 0 to see the update at the original context.
        let saved = m.ctx;
        m.ctx = 0;
        m.predict(&mut cum);
        m.ctx = saved;
        // count[0]=2, others=0. freq: [3, 1, 1, 1].
        assert_eq!(cum, [0, 3, 4, 5, 6]);
    }

    #[test]
    fn reset_clears() {
        let mut m = AuxModel::<4>::new();
        m.update(2);
        m.update(3);
        m.reset();
        assert_eq!(m.ctx, 0);
        let mut cum = [0u32; 5];
        m.predict(&mut cum);
        assert_eq!(cum, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn n_models_up_rounding() {
        // NModels=2 -> NModelsUp=2
        assert_eq!(AuxModel::<2>::N_MODELS_UP, 2);
        // NModels=5 -> NModelsUp=8
        assert_eq!(AuxModel::<5>::N_MODELS_UP, 8);
        // NModels=12 -> NModelsUp=16
        assert_eq!(AuxModel::<12>::N_MODELS_UP, 16);
    }
}
