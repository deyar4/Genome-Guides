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

// IUPAC nucleotide reorder/validate translation shared by the block codecs.

/// Number of nucleotide symbols per coding block.
pub const BLOCK_SIZE: usize = 80;

/// SSE PSHUFB-based forward translation (x86_64 only).
/// Translates and validates 80 bytes in 5x16-byte SIMD passes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn forward_xlat_block_sse(block: &mut [u8; BLOCK_SIZE]) -> bool {
    use std::arch::x86_64::*;
    let xlat = _mm_setr_epi8(
        0x0F, 0x03, 0x01, 0x08, 0x00, 0x05, 0x06, 0x0A, 0x02, 0x07, 0x04, 0x0B, 0x09, 0x0C, 0x0D,
        0x0E,
    );
    let valid = _mm_setr_epi8(
        0x00, -1, -1, 0x00, -1, 0x00, 0x00, 0x00, -1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    );
    let z = _mm_setzero_si128();
    let p = block.as_mut_ptr();

    let v0 = _mm_loadu_si128(p.add(0) as *const __m128i);
    let v1 = _mm_loadu_si128(p.add(16) as *const __m128i);
    let v2 = _mm_loadu_si128(p.add(32) as *const __m128i);
    let v3 = _mm_loadu_si128(p.add(48) as *const __m128i);
    let v4 = _mm_loadu_si128(p.add(64) as *const __m128i);

    let mut badv = _mm_cmpeq_epi8(_mm_shuffle_epi8(valid, v0), z);
    let r0 = _mm_shuffle_epi8(xlat, v0);
    badv = _mm_or_si128(badv, _mm_cmpeq_epi8(_mm_shuffle_epi8(valid, v1), z));
    let r1 = _mm_shuffle_epi8(xlat, v1);
    badv = _mm_or_si128(badv, _mm_cmpeq_epi8(_mm_shuffle_epi8(valid, v2), z));
    let r2 = _mm_shuffle_epi8(xlat, v2);
    badv = _mm_or_si128(badv, _mm_cmpeq_epi8(_mm_shuffle_epi8(valid, v3), z));
    let r3 = _mm_shuffle_epi8(xlat, v3);
    badv = _mm_or_si128(badv, _mm_cmpeq_epi8(_mm_shuffle_epi8(valid, v4), z));
    let r4 = _mm_shuffle_epi8(xlat, v4);

    _mm_storeu_si128(p.add(0) as *mut __m128i, r0);
    _mm_storeu_si128(p.add(16) as *mut __m128i, r1);
    _mm_storeu_si128(p.add(32) as *mut __m128i, r2);
    _mm_storeu_si128(p.add(48) as *mut __m128i, r3);
    _mm_storeu_si128(p.add(64) as *mut __m128i, r4);

    _mm_movemask_epi8(badv) != 0
}

/// SSE PSHUFB-based backward translation (x86_64 only).
/// Uses a single full inverse table (works for both ACGT-only and IUPAC).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn backward_xlat_block_sse(block: &mut [u8; BLOCK_SIZE]) {
    use std::arch::x86_64::*;
    let inv = _mm_setr_epi8(
        0x04, 0x02, 0x08, 0x01, 0x0A, 0x05, 0x06, 0x09, 0x03, 0x0C, 0x07, 0x0B, 0x0D, 0x0E, 0x0F,
        0x00,
    );
    let p = block.as_mut_ptr();

    let v0 = _mm_shuffle_epi8(inv, _mm_loadu_si128(p.add(0) as *const __m128i));
    let v1 = _mm_shuffle_epi8(inv, _mm_loadu_si128(p.add(16) as *const __m128i));
    let v2 = _mm_shuffle_epi8(inv, _mm_loadu_si128(p.add(32) as *const __m128i));
    let v3 = _mm_shuffle_epi8(inv, _mm_loadu_si128(p.add(48) as *const __m128i));
    let v4 = _mm_shuffle_epi8(inv, _mm_loadu_si128(p.add(64) as *const __m128i));

    _mm_storeu_si128(p.add(0) as *mut __m128i, v0);
    _mm_storeu_si128(p.add(16) as *mut __m128i, v1);
    _mm_storeu_si128(p.add(32) as *mut __m128i, v2);
    _mm_storeu_si128(p.add(48) as *mut __m128i, v3);
    _mm_storeu_si128(p.add(64) as *mut __m128i, v4);
}

/// Scalar forward translation fallback.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
#[inline(always)]
fn forward_xlat_block_scalar(block: &mut [u8; BLOCK_SIZE]) -> bool {
    const XLAT: u64 = 0xedc9b472a650813f;
    const VALID: u16 = 0x0116;
    let mut good = true;
    for b in block.iter_mut() {
        let x = (*b & 0xF) as u32;
        good &= ((VALID >> x) & 1) != 0;
        *b = ((XLAT >> (x << 2)) & 0xF) as u8;
    }
    !good
}

/// Scalar backward translation fallback.
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
#[inline(always)]
fn backward_xlat_block_scalar(block: &mut [u8; BLOCK_SIZE], has_non_acgt: bool) {
    if has_non_acgt {
        const INV: u64 = 0x0fedb7c3965a1824;
        for b in block.iter_mut() {
            *b = ((INV >> ((*b & 0xF) as u32 * 4)) & 0xF) as u8;
        }
    } else {
        const INV_ACGT: u64 = 0x1824;
        for b in block.iter_mut() {
            *b = ((INV_ACGT >> ((*b & 0x3) as u32 * 4)) & 0xF) as u8;
        }
    }
}

/// Reorder IUPAC nucleotide indices for compression and detect non-ACGT.
/// Modifies `block` in-place. Returns true if any non-ACGT byte is present.
#[inline(always)]
pub fn forward_xlat_block(block: &mut [u8; BLOCK_SIZE]) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        unsafe { forward_xlat_block_sse(block) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        forward_xlat_block_scalar(block)
    }
}

/// Inverse of `forward_xlat_block`.
#[inline(always)]
pub fn backward_xlat_block(block: &mut [u8; BLOCK_SIZE], has_non_acgt: bool) {
    #[cfg(target_arch = "x86_64")]
    {
        let _ = has_non_acgt;
        unsafe { backward_xlat_block_sse(block) };
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        backward_xlat_block_scalar(block, has_non_acgt)
    }
}
