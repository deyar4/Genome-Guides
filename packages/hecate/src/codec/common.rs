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

/// Map ASCII byte -> 2-bit code. 0xFF = invalid.
/// C/c -> 0, G/g -> 1, A/a -> 2, T/t -> 3
pub const TWO_BIT_ENCODE: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    t[b'C' as usize] = 0;
    t[b'c' as usize] = 0;
    t[b'G' as usize] = 1;
    t[b'g' as usize] = 1;
    t[b'A' as usize] = 2;
    t[b'a' as usize] = 2;
    t[b'T' as usize] = 3;
    t[b't' as usize] = 3;
    t
};

/// 2-bit code -> uppercase ASCII.
pub const TWO_BIT_DECODE: [u8; 4] = [b'C', b'G', b'A', b'T'];

/// Packed byte -> 4 uppercase ASCII bases (2-bit).
/// Index by packed byte, get [base0, base1, base2, base3].
pub const TWO_BIT_UNPACK_4: [[u8; 4]; 256] = {
    let mut t = [[0u8; 4]; 256];
    let decode = [b'C', b'G', b'A', b'T'];
    let mut i = 0u16;
    while i < 256 {
        t[i as usize][0] = decode[(i & 0x03) as usize];
        t[i as usize][1] = decode[((i >> 2) & 0x03) as usize];
        t[i as usize][2] = decode[((i >> 4) & 0x03) as usize];
        t[i as usize][3] = decode[((i >> 6) & 0x03) as usize];
        i += 1;
    }
    t
};

/// Map ASCII byte -> 4-bit IUPAC code. 0xFF = invalid.
/// Gap(-)=0, A=1, C=2, M=3, G=4, R=5, S=6, V=7,
/// T=8, W=9, Y=10, H=11, K=12, D=13, B=14, N=15
pub const FOUR_BIT_ENCODE: [u8; 256] = {
    let mut t = [0xFFu8; 256];
    t[b'-' as usize] = 0;
    t[b'.' as usize] = 0;
    t[b'A' as usize] = 1;
    t[b'a' as usize] = 1;
    t[b'C' as usize] = 2;
    t[b'c' as usize] = 2;
    t[b'M' as usize] = 3;
    t[b'm' as usize] = 3;
    t[b'G' as usize] = 4;
    t[b'g' as usize] = 4;
    t[b'R' as usize] = 5;
    t[b'r' as usize] = 5;
    t[b'S' as usize] = 6;
    t[b's' as usize] = 6;
    t[b'V' as usize] = 7;
    t[b'v' as usize] = 7;
    t[b'T' as usize] = 8;
    t[b't' as usize] = 8;
    t[b'U' as usize] = 8;
    t[b'u' as usize] = 8;
    t[b'W' as usize] = 9;
    t[b'w' as usize] = 9;
    t[b'Y' as usize] = 10;
    t[b'y' as usize] = 10;
    t[b'H' as usize] = 11;
    t[b'h' as usize] = 11;
    t[b'K' as usize] = 12;
    t[b'k' as usize] = 12;
    t[b'D' as usize] = 13;
    t[b'd' as usize] = 13;
    t[b'B' as usize] = 14;
    t[b'b' as usize] = 14;
    t[b'N' as usize] = 15;
    t[b'n' as usize] = 15;
    t
};

/// 4-bit code -> uppercase ASCII.
pub const FOUR_BIT_DECODE: [u8; 16] = [
    b'-', b'A', b'C', b'M', b'G', b'R', b'S', b'V', b'T', b'W', b'Y', b'H', b'K', b'D', b'B', b'N',
];

/// Packed byte -> 2 uppercase ASCII bases (4-bit).
/// Index by packed byte, get [base0, base1].
pub const FOUR_BIT_UNPACK_2: [[u8; 2]; 256] = {
    let decode = [
        b'-', b'A', b'C', b'M', b'G', b'R', b'S', b'V', b'T', b'W', b'Y', b'H', b'K', b'D', b'B',
        b'N',
    ];
    let mut t = [[0u8; 2]; 256];
    let mut i = 0u16;
    while i < 256 {
        t[i as usize][0] = decode[(i & 0x0F) as usize];
        t[i as usize][1] = decode[((i >> 4) & 0x0F) as usize];
        i += 1;
    }
    t
};

/// ASCII byte -> case bit. 1 = uppercase or non-alpha, 0 = lowercase.
pub const CASE_BIT: [u8; 256] = {
    let mut t = [1u8; 256];
    let mut i = b'a';
    while i <= b'z' {
        t[i as usize] = 0;
        i += 1;
    }
    t
};

/// Pack a single code into the nucleotide buffer.
#[inline(always)]
pub fn pack_code(alphabet: u8, buf: &mut Vec<u8>, bit_pos: &mut u8, code: u8) {
    match alphabet {
        2 => {
            if *bit_pos == 0 {
                buf.push(code);
            } else {
                *buf.last_mut().unwrap() |= code << *bit_pos;
            }
            *bit_pos = (*bit_pos + 2) & 7;
        }
        4 => {
            if *bit_pos == 0 {
                buf.push(code);
            } else {
                *buf.last_mut().unwrap() |= code << 4;
            }
            *bit_pos = (*bit_pos + 4) & 7;
        }
        _ => {} // 8-bit handled separately
    }
}

// --- Case Encoder (Bitmap -> RLE) ---
//
// Collects case bits into a compact bitmap during encoding (fast, nearly
// branchless inner loop), then converts to RLE format on finish(). This
// avoids the serial dependency of incremental RLE encoding in the hot path.

pub struct CaseEncoder {
    words: Vec<u64>,
    current_word: u64,
    bits_in_word: u8,
    rle_buf: Vec<u8>,
}

impl Default for CaseEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl CaseEncoder {
    pub fn new() -> Self {
        Self {
            words: Vec::new(),
            current_word: 0,
            bits_in_word: 0,
            rle_buf: Vec::new(),
        }
    }

    /// Push a single case bit. is_upper: 1 = uppercase, 0 = lowercase.
    #[inline(always)]
    pub fn push_bit(&mut self, is_upper: u8) {
        self.current_word |= (is_upper as u64) << self.bits_in_word;
        self.bits_in_word += 1;
        if self.bits_in_word == 64 {
            self.words.push(self.current_word);
            self.current_word = 0;
            self.bits_in_word = 0;
        }
    }

    /// Push multiple case bits from the low bits of `mask`.
    /// Bit 0 = first base, bit 1 = second, etc. `count` must be 1..=48.
    #[inline(always)]
    pub fn push_bits(&mut self, count: u8, mask: u64) {
        debug_assert!(count > 0 && count <= 48);
        let shift = self.bits_in_word;
        self.current_word |= mask << shift;
        let new_pos = shift + count;
        if new_pos >= 64 {
            self.words.push(self.current_word);
            if new_pos > 64 {
                self.current_word = mask >> (64 - shift);
            } else {
                self.current_word = 0;
            }
            self.bits_in_word = new_pos - 64;
        } else {
            self.bits_in_word = new_pos;
        }
    }

    /// Access the RLE buffer (valid after `finish()` has been called).
    pub fn rle_data(&self) -> &[u8] {
        &self.rle_buf
    }

    /// Convert the collected bitmap to RLE and return the result.
    pub fn finish(&mut self) -> &[u8] {
        self.rle_buf.clear();
        let partial_bits = self.bits_in_word as u32;
        let total_bits = self.words.len() as u64 * 64 + partial_bits as u64;
        if total_bits == 0 {
            return &self.rle_buf;
        }

        let mut is_upper = true;
        let mut run_len: usize = 0;

        for &word in &self.words {
            Self::process_word(word, 64, &mut is_upper, &mut run_len, &mut self.rle_buf);
        }
        if partial_bits > 0 {
            Self::process_word(
                self.current_word,
                partial_bits,
                &mut is_upper,
                &mut run_len,
                &mut self.rle_buf,
            );
        }
        if run_len > 0 {
            Self::emit_run(&mut self.rle_buf, run_len);
        }

        &self.rle_buf
    }

    /// Scan a word for runs of 1s (uppercase) and 0s (lowercase) using
    /// trailing_zeros/trailing_ones, accumulating into the current run.
    fn process_word(
        word: u64,
        num_bits: u32,
        is_upper: &mut bool,
        run_len: &mut usize,
        rle: &mut Vec<u8>,
    ) {
        let mut remaining = num_bits;
        let mut pos = 0u32;
        while remaining > 0 {
            let shifted = word >> pos;
            let run = if *is_upper {
                (!shifted).trailing_zeros().min(remaining)
            } else {
                shifted.trailing_zeros().min(remaining)
            };
            *run_len += run as usize;
            pos += run;
            remaining -= run;
            if remaining > 0 {
                Self::emit_run(rle, *run_len);
                *is_upper = !*is_upper;
                *run_len = 0;
            }
        }
    }

    fn emit_run(rle: &mut Vec<u8>, mut len: usize) {
        while len >= 255 {
            rle.push(255);
            rle.push(0);
            len -= 255;
        }
        rle.push(len as u8);
    }

    pub fn reset(&mut self) {
        self.words.clear();
        self.current_word = 0;
        self.bits_in_word = 0;
        self.rle_buf.clear();
    }
}

// --- Case Decoder (RLE) ---

pub struct CaseDecoder<'a> {
    data: &'a [u8],
    pos: usize,
    is_upper: bool,
    remaining: u8,
}

impl<'a> CaseDecoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            is_upper: false,
            remaining: 0,
        }
    }

    #[inline(always)]
    pub fn next_is_upper(&mut self) -> bool {
        loop {
            if self.remaining > 0 {
                self.remaining -= 1;
                return self.is_upper;
            }
            if self.pos >= self.data.len() {
                return self.is_upper;
            }
            self.remaining = self.data[self.pos];
            self.pos += 1;
            self.is_upper = !self.is_upper;
        }
    }

    /// Get the next run as (is_upper, length).
    /// Returns usize::MAX length when the data is exhausted (infinite trailing run).
    #[inline(always)]
    pub fn next_run(&mut self) -> (bool, usize) {
        if self.remaining > 0 {
            let run = self.remaining as usize;
            self.remaining = 0;
            return (self.is_upper, run);
        }
        if self.pos >= self.data.len() {
            return (self.is_upper, usize::MAX);
        }
        self.is_upper = !self.is_upper;
        let run = self.data[self.pos] as usize;
        self.pos += 1;
        self.remaining = 0;
        (self.is_upper, run)
    }
}

// --- Scalar Batch Encoding ---

/// Try to pack 4 ASCII bases into one byte using 2-bit encoding.
/// Returns None if any base is out-of-alphabet.
#[inline(always)]
pub fn try_pack_4_bases_2bit(src: &[u8]) -> Option<u8> {
    let c0 = TWO_BIT_ENCODE[src[0] as usize];
    let c1 = TWO_BIT_ENCODE[src[1] as usize];
    let c2 = TWO_BIT_ENCODE[src[2] as usize];
    let c3 = TWO_BIT_ENCODE[src[3] as usize];
    if (c0 | c1 | c2 | c3) & 0x80 != 0 {
        return None;
    }
    Some(c0 | (c1 << 2) | (c2 << 4) | (c3 << 6))
}

/// Try to pack 2 ASCII bases into one byte using 4-bit encoding.
/// Returns None if any base is out-of-alphabet.
#[inline(always)]
pub fn try_pack_2_bases_4bit(src: &[u8]) -> Option<u8> {
    let c0 = FOUR_BIT_ENCODE[src[0] as usize];
    let c1 = FOUR_BIT_ENCODE[src[1] as usize];
    if (c0 | c1) & 0x80 != 0 {
        return None;
    }
    Some(c0 | (c1 << 4))
}

/// Try to pack 8 ASCII bases into 2 bytes using 2-bit encoding.
/// Returns None if any base is out-of-alphabet (whitespace, OOA, etc).
#[inline(always)]
pub fn try_pack_8_bases_2bit(src: &[u8]) -> Option<[u8; 2]> {
    let c0 = TWO_BIT_ENCODE[src[0] as usize];
    let c1 = TWO_BIT_ENCODE[src[1] as usize];
    let c2 = TWO_BIT_ENCODE[src[2] as usize];
    let c3 = TWO_BIT_ENCODE[src[3] as usize];
    let c4 = TWO_BIT_ENCODE[src[4] as usize];
    let c5 = TWO_BIT_ENCODE[src[5] as usize];
    let c6 = TWO_BIT_ENCODE[src[6] as usize];
    let c7 = TWO_BIT_ENCODE[src[7] as usize];
    if (c0 | c1 | c2 | c3 | c4 | c5 | c6 | c7) & 0x80 != 0 {
        return None;
    }
    Some([
        c0 | (c1 << 2) | (c2 << 4) | (c3 << 6),
        c4 | (c5 << 2) | (c6 << 4) | (c7 << 6),
    ])
}

/// Try to pack 8 ASCII bases into 4 bytes using 4-bit IUPAC encoding.
/// Returns None if any base is out-of-alphabet.
#[inline(always)]
pub fn try_pack_8_bases_4bit(src: &[u8]) -> Option<[u8; 4]> {
    let c0 = FOUR_BIT_ENCODE[src[0] as usize];
    let c1 = FOUR_BIT_ENCODE[src[1] as usize];
    let c2 = FOUR_BIT_ENCODE[src[2] as usize];
    let c3 = FOUR_BIT_ENCODE[src[3] as usize];
    let c4 = FOUR_BIT_ENCODE[src[4] as usize];
    let c5 = FOUR_BIT_ENCODE[src[5] as usize];
    let c6 = FOUR_BIT_ENCODE[src[6] as usize];
    let c7 = FOUR_BIT_ENCODE[src[7] as usize];
    if (c0 | c1 | c2 | c3 | c4 | c5 | c6 | c7) & 0x80 != 0 {
        return None;
    }
    Some([
        c0 | (c1 << 4),
        c2 | (c3 << 4),
        c4 | (c5 << 4),
        c6 | (c7 << 4),
    ])
}

/// Build an 8-bit case mask from 8 consecutive bytes.
/// Bit k = CASE_BIT[s[k]]. Used by batch encoding paths.
#[inline(always)]
pub fn case_mask_8(s: &[u8]) -> u64 {
    (CASE_BIT[s[0] as usize] as u64)
        | ((CASE_BIT[s[1] as usize] as u64) << 1)
        | ((CASE_BIT[s[2] as usize] as u64) << 2)
        | ((CASE_BIT[s[3] as usize] as u64) << 3)
        | ((CASE_BIT[s[4] as usize] as u64) << 4)
        | ((CASE_BIT[s[5] as usize] as u64) << 5)
        | ((CASE_BIT[s[6] as usize] as u64) << 6)
        | ((CASE_BIT[s[7] as usize] as u64) << 7)
}

/// Build a 4-bit case mask from 4 consecutive bytes.
#[inline(always)]
pub fn case_mask_4(s: &[u8]) -> u64 {
    (CASE_BIT[s[0] as usize] as u64)
        | ((CASE_BIT[s[1] as usize] as u64) << 1)
        | ((CASE_BIT[s[2] as usize] as u64) << 2)
        | ((CASE_BIT[s[3] as usize] as u64) << 3)
}

/// Build a 2-bit case mask from 2 consecutive bytes.
#[inline(always)]
pub fn case_mask_2(s: &[u8]) -> u64 {
    (CASE_BIT[s[0] as usize] as u64) | ((CASE_BIT[s[1] as usize] as u64) << 1)
}

// --- Streaming Case Decoder ---

pub struct StreamingCaseDecoder {
    buf: Vec<u8>,
    pos: usize,
    is_upper: bool,
    remaining: u16,
}

impl Default for StreamingCaseDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingCaseDecoder {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            pos: 0,
            is_upper: false,
            remaining: 0,
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    #[inline(always)]
    pub fn next_is_upper(&mut self) -> bool {
        loop {
            if self.remaining > 0 {
                self.remaining -= 1;
                return self.is_upper;
            }
            if self.pos >= self.buf.len() {
                return self.is_upper;
            }
            self.remaining = self.buf[self.pos] as u16;
            self.pos += 1;
            self.is_upper = !self.is_upper;
        }
    }

    pub fn has_remaining(&self) -> bool {
        self.remaining > 0 || self.pos < self.buf.len()
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.pos = 0;
        self.is_upper = false;
        self.remaining = 0;
    }

    /// Initialize the decoder from a case checkpoint. The checkpoint's
    /// `is_upper` is the IndexBuilder state BEFORE it toggled for the
    /// byte at `case_byte_offset`. The decoder's internal state is the
    /// post-toggle value (matching the polarity of the remaining run).
    pub fn seek_to_checkpoint(&mut self, run_remainder: u16, is_upper_pre_toggle: bool) {
        self.buf.clear();
        self.pos = 0;
        self.is_upper = !is_upper_pre_toggle;
        self.remaining = run_remainder;
    }

    /// Apply case in-place to a buffer of uppercase ASCII bases.
    /// Consumes exactly `buf.len()` case bits from the RLE stream.
    pub fn apply_case(&mut self, buf: &mut [u8]) {
        let len = buf.len();
        // Fast path: the rest of the stream is one uniform run that covers
        // the chunk -- no per-iteration work needed at all.
        if self.remaining as usize >= len && self.pos >= self.buf.len() {
            if !self.is_upper {
                apply_lowercase(buf);
            }
            self.remaining -= len as u16;
            return;
        }
        let mut offset = 0usize;
        while offset < len {
            // Refill `remaining`. The encoder splits long runs into
            // (255, 0, 255, 0, ...) chunks so a `0` byte just toggles back;
            // collapse zero-length runs in this loop instead of paying an
            // outer iteration for each.
            while self.remaining == 0 {
                if self.pos >= self.buf.len() {
                    // Exhausted: rest stays as-is.
                    return;
                }
                self.remaining = self.buf[self.pos] as u16;
                self.pos += 1;
                self.is_upper = !self.is_upper;
            }
            let run = (self.remaining as usize).min(len - offset);
            if !self.is_upper {
                apply_lowercase(&mut buf[offset..offset + run]);
            }
            self.remaining -= run as u16;
            offset += run;
        }
    }
}

/// Unpack packed nucleotide bytes into uppercase ASCII bases.
/// Dispatches to const-generic monomorphized inner function.
pub fn unpack_bases(
    alphabet: crate::container::AlphabetMode,
    packed: &[u8],
    base_count: u64,
    out: &mut Vec<u8>,
) {
    use crate::container::AlphabetMode;
    match alphabet {
        AlphabetMode::TwoBit => unpack_bases_inner::<2>(packed, base_count, out),
        AlphabetMode::FourBit => unpack_bases_inner::<4>(packed, base_count, out),
        AlphabetMode::EightBit => unpack_bases_inner::<8>(packed, base_count, out),
    }
}

// --- SIMD Unpack (x86_64 SSSE3) ---
//
// PSHUFB-based: extracts 4 streams of 2-bit codes (or 2 streams of 4-bit
// codes), looks each up in a tiny ASCII LUT, and re-interleaves with
// unpack-byte/word. Per iteration: 16 packed bytes -> 64 ASCII (2-bit) or
// 32 ASCII (4-bit). Doing 4-8x fewer loads than a byte-wise LUT-load +
// 32-bit-store loop and using vector store ports is where the speed comes from.

// SSSE3 unpack helpers. Marked `inline(always)` so they fold into the
// `#[target_feature]` loop wrappers below -- without inlining, each invocation
// pays a function-call boundary because target_feature blocks normal inlining
// into callers without the same feature set.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn unpack_2bit_64bases_ssse3(packed: *const u8, out: *mut u8) {
    use std::arch::x86_64::*;
    let x = _mm_loadu_si128(packed as *const __m128i);
    let mask = _mm_set1_epi8(0x03);
    let lut = _mm_setr_epi8(
        b'C' as i8, b'G' as i8, b'A' as i8, b'T' as i8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    );
    // _mm_srli_epi16 leaks bits across byte boundaries within each 16-bit
    // word, but the subsequent AND with 0x03 discards them.
    let s0 = _mm_and_si128(x, mask);
    let s1 = _mm_and_si128(_mm_srli_epi16(x, 2), mask);
    let s2 = _mm_and_si128(_mm_srli_epi16(x, 4), mask);
    let s3 = _mm_and_si128(_mm_srli_epi16(x, 6), mask);
    let a0 = _mm_shuffle_epi8(lut, s0);
    let a1 = _mm_shuffle_epi8(lut, s1);
    let a2 = _mm_shuffle_epi8(lut, s2);
    let a3 = _mm_shuffle_epi8(lut, s3);
    let lo01 = _mm_unpacklo_epi8(a0, a1);
    let hi01 = _mm_unpackhi_epi8(a0, a1);
    let lo23 = _mm_unpacklo_epi8(a2, a3);
    let hi23 = _mm_unpackhi_epi8(a2, a3);
    let out0 = _mm_unpacklo_epi16(lo01, lo23);
    let out1 = _mm_unpackhi_epi16(lo01, lo23);
    let out2 = _mm_unpacklo_epi16(hi01, hi23);
    let out3 = _mm_unpackhi_epi16(hi01, hi23);
    _mm_storeu_si128(out as *mut __m128i, out0);
    _mm_storeu_si128(out.add(16) as *mut __m128i, out1);
    _mm_storeu_si128(out.add(32) as *mut __m128i, out2);
    _mm_storeu_si128(out.add(48) as *mut __m128i, out3);
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn unpack_4bit_32bases_ssse3(packed: *const u8, out: *mut u8) {
    use std::arch::x86_64::*;
    let x = _mm_loadu_si128(packed as *const __m128i);
    let mask_lo = _mm_set1_epi8(0x0F);
    let lut = _mm_setr_epi8(
        b'-' as i8, b'A' as i8, b'C' as i8, b'M' as i8, b'G' as i8, b'R' as i8, b'S' as i8,
        b'V' as i8, b'T' as i8, b'W' as i8, b'Y' as i8, b'H' as i8, b'K' as i8, b'D' as i8,
        b'B' as i8, b'N' as i8,
    );
    let lo = _mm_and_si128(x, mask_lo);
    let hi = _mm_and_si128(_mm_srli_epi16(x, 4), mask_lo);
    let a_lo = _mm_shuffle_epi8(lut, lo);
    let a_hi = _mm_shuffle_epi8(lut, hi);
    let out0 = _mm_unpacklo_epi8(a_lo, a_hi);
    let out1 = _mm_unpackhi_epi8(a_lo, a_hi);
    _mm_storeu_si128(out as *mut __m128i, out0);
    _mm_storeu_si128(out.add(16) as *mut __m128i, out1);
}

// Loop wrappers carrying the target_feature attribute so their inner SIMD
// helpers (declared `inline(always)`) actually inline.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn unpack_2bit_loop_ssse3(in_ptr: *const u8, out_ptr: *mut u8, full_bytes: usize) -> usize {
    let unrolled = full_bytes & !15;
    let mut i = 0usize;
    while i < unrolled {
        unpack_2bit_64bases_ssse3(in_ptr.add(i), out_ptr.add(i * 4));
        i += 16;
    }
    i
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn unpack_4bit_loop_ssse3(in_ptr: *const u8, out_ptr: *mut u8, full_bytes: usize) -> usize {
    let unrolled = full_bytes & !15;
    let mut i = 0usize;
    while i < unrolled {
        unpack_4bit_32bases_ssse3(in_ptr.add(i), out_ptr.add(i * 2));
        i += 16;
    }
    i
}

#[inline(always)]
#[allow(clippy::uninit_vec)]
fn unpack_bases_inner<const ALPHABET: u8>(packed: &[u8], base_count: u64, out: &mut Vec<u8>) {
    let bc = base_count as usize;
    let start = out.len();
    out.reserve(bc);
    // SAFETY: we write every byte in out[start..start+bc] below
    unsafe {
        out.set_len(start + bc);
    }
    let text = &mut out[start..];
    // SAFETY for all branches: caller guarantees packed has enough bytes
    // for base_count bases. Table indices are u8 (0..255) into 256-entry tables.
    match ALPHABET {
        2 => {
            let full_bytes = bc / 4;
            let needed = full_bytes + if !bc.is_multiple_of(4) { 1 } else { 0 };
            if packed.len() < needed {
                // Truncated packed data -- fill with placeholder to avoid UB.
                // This can happen with corrupted .he files.
                text.fill(b'N');
                return;
            }
            let out_ptr = text.as_mut_ptr();
            let in_ptr = packed.as_ptr();
            let mut i = 0usize;
            // SIMD: 16 packed bytes -> 64 ASCII per iter.
            #[cfg(target_arch = "x86_64")]
            unsafe {
                i = unpack_2bit_loop_ssse3(in_ptr, out_ptr, full_bytes);
            }
            // Scalar tail: original 4x-unrolled LUT loop.
            let unrolled4 = full_bytes & !3;
            while i < unrolled4 {
                unsafe {
                    let p0 = *in_ptr.add(i) as usize;
                    let p1 = *in_ptr.add(i + 1) as usize;
                    let p2 = *in_ptr.add(i + 2) as usize;
                    let p3 = *in_ptr.add(i + 3) as usize;
                    let o = out_ptr.add(i * 4);
                    (o as *mut [u8; 4]).write_unaligned(*TWO_BIT_UNPACK_4.get_unchecked(p0));
                    (o.add(4) as *mut [u8; 4]).write_unaligned(*TWO_BIT_UNPACK_4.get_unchecked(p1));
                    (o.add(8) as *mut [u8; 4]).write_unaligned(*TWO_BIT_UNPACK_4.get_unchecked(p2));
                    (o.add(12) as *mut [u8; 4])
                        .write_unaligned(*TWO_BIT_UNPACK_4.get_unchecked(p3));
                }
                i += 4;
            }
            while i < full_bytes {
                unsafe {
                    let p = *in_ptr.add(i) as usize;
                    (out_ptr.add(i * 4) as *mut [u8; 4])
                        .write_unaligned(*TWO_BIT_UNPACK_4.get_unchecked(p));
                }
                i += 1;
            }
            let remaining = bc - full_bytes * 4;
            if remaining > 0 {
                let u = TWO_BIT_UNPACK_4[packed[full_bytes] as usize];
                let tail = &mut text[full_bytes * 4..];
                tail[..remaining].copy_from_slice(&u[..remaining]);
            }
        }
        4 => {
            let full_bytes = bc / 2;
            let needed = full_bytes + if !bc.is_multiple_of(2) { 1 } else { 0 };
            if packed.len() < needed {
                text.fill(b'N');
                return;
            }
            let out_ptr = text.as_mut_ptr();
            let in_ptr = packed.as_ptr();
            let mut i = 0usize;
            // SIMD: 16 packed bytes -> 32 ASCII per iter.
            #[cfg(target_arch = "x86_64")]
            unsafe {
                i = unpack_4bit_loop_ssse3(in_ptr, out_ptr, full_bytes);
            }
            // Scalar tail.
            let unrolled4 = full_bytes & !3;
            while i < unrolled4 {
                unsafe {
                    let p0 = *in_ptr.add(i) as usize;
                    let p1 = *in_ptr.add(i + 1) as usize;
                    let p2 = *in_ptr.add(i + 2) as usize;
                    let p3 = *in_ptr.add(i + 3) as usize;
                    let o = out_ptr.add(i * 2);
                    (o as *mut [u8; 2]).write_unaligned(*FOUR_BIT_UNPACK_2.get_unchecked(p0));
                    (o.add(2) as *mut [u8; 2])
                        .write_unaligned(*FOUR_BIT_UNPACK_2.get_unchecked(p1));
                    (o.add(4) as *mut [u8; 2])
                        .write_unaligned(*FOUR_BIT_UNPACK_2.get_unchecked(p2));
                    (o.add(6) as *mut [u8; 2])
                        .write_unaligned(*FOUR_BIT_UNPACK_2.get_unchecked(p3));
                }
                i += 4;
            }
            while i < full_bytes {
                unsafe {
                    let p = *in_ptr.add(i) as usize;
                    (out_ptr.add(i * 2) as *mut [u8; 2])
                        .write_unaligned(*FOUR_BIT_UNPACK_2.get_unchecked(p));
                }
                i += 1;
            }
            if bc & 1 != 0 {
                text[full_bytes * 2] = FOUR_BIT_DECODE[(packed[full_bytes] & 0x0F) as usize];
            }
        }
        8 => {
            if packed.len() < bc {
                text.fill(b'N');
                return;
            }
            text.copy_from_slice(&packed[..bc]);
        }
        _ => unreachable!(),
    }
}

/// Extract the nuc code at a given base position from packed nuc data.
#[inline]
pub fn extract_nuc_code_from_packed(
    alphabet: crate::container::AlphabetMode,
    nuc: &[u8],
    pos: u64,
) -> u8 {
    use crate::container::AlphabetMode;
    match alphabet {
        AlphabetMode::TwoBit => {
            let byte_idx = pos as usize / 4;
            let bit_offset = (pos as usize % 4) * 2;
            (nuc[byte_idx] >> bit_offset) & 0x03
        }
        AlphabetMode::FourBit => {
            let byte_idx = pos as usize / 2;
            let bit_offset = (pos as usize % 2) * 4;
            (nuc[byte_idx] >> bit_offset) & 0x0F
        }
        AlphabetMode::EightBit => nuc[pos as usize],
    }
}

/// Decode packed nucleotides into text, applying extras and case.
///
/// Reuses `text_buf` across calls to avoid per-record allocation.
/// After return, `text_buf` contains `base_count` ASCII bytes ready
/// for output (with case and extras applied).
#[allow(clippy::uninit_vec)]
pub fn decode_bases_into(
    alphabet: crate::container::AlphabetMode,
    case_mode: crate::container::CaseMode,
    nuc: &[u8],
    case_data: &[u8],
    extras: &[(u64, u8)],
    base_count: u64,
    text_buf: &mut Vec<u8>,
) {
    use crate::container::{AlphabetMode, CaseMode};
    let bc = base_count as usize;

    // Phase 1: Bulk unpack via LUT into flat uppercase ASCII.
    text_buf.clear();
    text_buf.reserve(bc);
    // SAFETY: The unpack phase writes every byte in text[0..bc] before any read.
    unsafe {
        text_buf.set_len(bc);
    }
    let text = text_buf.as_mut_slice();
    match alphabet {
        AlphabetMode::TwoBit => {
            let full_bytes = bc / 4;
            for (chunk, &packed) in text.chunks_exact_mut(4).zip(nuc[..full_bytes].iter()) {
                let u = TWO_BIT_UNPACK_4[packed as usize];
                chunk[0] = u[0];
                chunk[1] = u[1];
                chunk[2] = u[2];
                chunk[3] = u[3];
            }
            let remaining = bc - full_bytes * 4;
            if remaining > 0 {
                let u = TWO_BIT_UNPACK_4[nuc[full_bytes] as usize];
                let tail = &mut text[full_bytes * 4..];
                tail[..remaining].copy_from_slice(&u[..remaining]);
            }
        }
        AlphabetMode::FourBit => {
            let full_bytes = bc / 2;
            for (chunk, &packed) in text.chunks_exact_mut(2).zip(nuc[..full_bytes].iter()) {
                let u = FOUR_BIT_UNPACK_2[packed as usize];
                chunk[0] = u[0];
                chunk[1] = u[1];
            }
            if bc & 1 != 0 {
                text[full_bytes * 2] = FOUR_BIT_DECODE[(nuc[full_bytes] & 0x0F) as usize];
            }
        }
        AlphabetMode::EightBit => {
            text.copy_from_slice(&nuc[..bc]);
        }
    }

    // Phase 2: Apply extras - overwrite positions with reconstructed bytes
    for &(pos, ch) in extras {
        text[pos as usize] = ch;
    }

    // Phase 3: Apply case
    if case_mode == CaseMode::Include && !case_data.is_empty() {
        let mut case_dec = CaseDecoder::new(case_data);
        let mut offset = 0usize;
        loop {
            let (is_upper, run_len) = case_dec.next_run();
            let end = if run_len == usize::MAX {
                bc
            } else {
                (offset + run_len).min(bc)
            };
            if !is_upper {
                apply_lowercase(&mut text[offset..end]);
            }
            offset = end;
            if offset >= bc {
                break;
            }
        }
    }
}

/// Apply lowercase to a slice of ASCII bytes.
#[inline]
pub(crate) fn apply_lowercase(buf: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        apply_lowercase_simd(buf);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        for b in buf.iter_mut() {
            *b = b.to_ascii_lowercase();
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn apply_lowercase_simd(buf: &mut [u8]) {
    let len = buf.len();
    let mut i = 0;
    unsafe {
        use std::arch::x86_64::*;
        let lower_mask = _mm_set1_epi8(0x20);
        while i + 16 <= len {
            let v = _mm_loadu_si128(buf.as_ptr().add(i) as *const __m128i);
            let lowered = _mm_or_si128(v, lower_mask);
            _mm_storeu_si128(buf.as_mut_ptr().add(i) as *mut __m128i, lowered);
            i += 16;
        }
    }
    for b in buf[i..].iter_mut() {
        *b = b.to_ascii_lowercase();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_bit_encode_decode() {
        assert_eq!(TWO_BIT_ENCODE[b'C' as usize], 0);
        assert_eq!(TWO_BIT_ENCODE[b'c' as usize], 0);
        assert_eq!(TWO_BIT_ENCODE[b'G' as usize], 1);
        assert_eq!(TWO_BIT_ENCODE[b'A' as usize], 2);
        assert_eq!(TWO_BIT_ENCODE[b'T' as usize], 3);
        assert_eq!(TWO_BIT_ENCODE[b'N' as usize], 0xFF);
        assert_eq!(TWO_BIT_ENCODE[b'X' as usize], 0xFF);

        assert_eq!(TWO_BIT_DECODE[0], b'C');
        assert_eq!(TWO_BIT_DECODE[1], b'G');
        assert_eq!(TWO_BIT_DECODE[2], b'A');
        assert_eq!(TWO_BIT_DECODE[3], b'T');
    }

    #[test]
    fn test_four_bit_encode_decode() {
        assert_eq!(FOUR_BIT_ENCODE[b'A' as usize], 1);
        assert_eq!(FOUR_BIT_ENCODE[b'C' as usize], 2);
        assert_eq!(FOUR_BIT_ENCODE[b'G' as usize], 4);
        assert_eq!(FOUR_BIT_ENCODE[b'T' as usize], 8);
        assert_eq!(FOUR_BIT_ENCODE[b'N' as usize], 15);
        assert_eq!(FOUR_BIT_ENCODE[b'-' as usize], 0);
        assert_eq!(FOUR_BIT_ENCODE[b'R' as usize], 5);
        assert_eq!(FOUR_BIT_ENCODE[b'X' as usize], 0xFF);

        assert_eq!(FOUR_BIT_DECODE[1], b'A');
        assert_eq!(FOUR_BIT_DECODE[15], b'N');
        assert_eq!(FOUR_BIT_DECODE[0], b'-');
    }

    #[test]
    fn test_case_bit() {
        assert_eq!(CASE_BIT[b'A' as usize], 1);
        assert_eq!(CASE_BIT[b'a' as usize], 0);
        assert_eq!(CASE_BIT[b'Z' as usize], 1);
        assert_eq!(CASE_BIT[b'z' as usize], 0);
        assert_eq!(CASE_BIT[b'N' as usize], 1);
        assert_eq!(CASE_BIT[b'0' as usize], 1); // non-alpha defaults to upper
    }

    #[test]
    fn test_pack_code_2bit() {
        let mut buf = Vec::new();
        let mut bit_pos = 0u8;
        // Pack CGAT -> 0b11_10_01_00 = 0xE4
        pack_code(2, &mut buf, &mut bit_pos, 0); // C
        pack_code(2, &mut buf, &mut bit_pos, 1); // G
        pack_code(2, &mut buf, &mut bit_pos, 2); // A
        pack_code(2, &mut buf, &mut bit_pos, 3); // T
        assert_eq!(buf.len(), 1);
        assert_eq!(buf[0], 0b11_10_01_00);
    }

    #[test]
    fn test_pack_code_4bit() {
        let mut buf = Vec::new();
        let mut bit_pos = 0u8;
        // Pack A(1), C(2) -> 0x21
        pack_code(4, &mut buf, &mut bit_pos, 1); // A
        pack_code(4, &mut buf, &mut bit_pos, 2); // C
        assert_eq!(buf.len(), 1);
        assert_eq!(buf[0], 0x21);
    }

    #[test]
    fn test_case_encoder_decoder_uppercase() {
        let mut enc = CaseEncoder::new();
        // "AAAA" - 4 uppercase
        for _ in 0..4 {
            enc.push_bit(1);
        }
        let data = enc.finish().to_vec();
        assert_eq!(data, vec![4]);

        let mut dec = CaseDecoder::new(&data);
        for _ in 0..4 {
            assert!(dec.next_is_upper());
        }
    }

    #[test]
    fn test_case_encoder_decoder_mixed() {
        let mut enc = CaseEncoder::new();
        // "AAaa" -> [2, 2]
        enc.push_bit(1);
        enc.push_bit(1);
        enc.push_bit(0);
        enc.push_bit(0);
        let data = enc.finish().to_vec();
        assert_eq!(data, vec![2, 2]);

        let mut dec = CaseDecoder::new(&data);
        assert!(dec.next_is_upper()); // A
        assert!(dec.next_is_upper()); // A
        assert!(!dec.next_is_upper()); // a
        assert!(!dec.next_is_upper()); // a
    }

    #[test]
    fn test_case_encoder_decoder_lowercase_first() {
        let mut enc = CaseEncoder::new();
        // "aaAA" -> [0, 2, 2]
        enc.push_bit(0);
        enc.push_bit(0);
        enc.push_bit(1);
        enc.push_bit(1);
        let data = enc.finish().to_vec();
        assert_eq!(data, vec![0, 2, 2]);

        let mut dec = CaseDecoder::new(&data);
        assert!(!dec.next_is_upper()); // a
        assert!(!dec.next_is_upper()); // a
        assert!(dec.next_is_upper()); // A
        assert!(dec.next_is_upper()); // A
    }

    #[test]
    fn test_case_encoder_max_run() {
        let mut enc = CaseEncoder::new();
        // 256 uppercase -> [255, 0, 1]
        for _ in 0..256 {
            enc.push_bit(1);
        }
        let data = enc.finish().to_vec();
        assert_eq!(data, vec![255, 0, 1]);

        let mut dec = CaseDecoder::new(&data);
        for i in 0..256 {
            assert!(dec.next_is_upper(), "failed at base {}", i);
        }
    }

    #[test]
    fn test_case_single_base() {
        let mut enc = CaseEncoder::new();
        enc.push_bit(1);
        let data = enc.finish().to_vec();
        assert_eq!(data, vec![1]);

        let mut dec = CaseDecoder::new(&data);
        assert!(dec.next_is_upper());
    }

    #[test]
    fn test_streaming_case_decoder() {
        let mut enc = CaseEncoder::new();
        for _ in 0..3 {
            enc.push_bit(1);
        }
        for _ in 0..3 {
            enc.push_bit(0);
        }
        let data = enc.finish().to_vec();

        // Feed data in two chunks
        let mut scd = StreamingCaseDecoder::new();
        scd.feed(&data[..1]);
        scd.feed(&data[1..]);

        for _ in 0..3 {
            assert!(scd.next_is_upper());
        }
        for _ in 0..3 {
            assert!(!scd.next_is_upper());
        }
    }

    #[test]
    fn test_streaming_case_decoder_seek_to_checkpoint_large_remainder() {
        // Verify that seek_to_checkpoint preserves the full u16 run_remainder.
        // Before the fix, remainder > 255 was silently truncated to u8.
        let mut scd = StreamingCaseDecoder::new();
        let run_remainder: u16 = 300;
        scd.seek_to_checkpoint(run_remainder, true); // pre-toggle=true -> post=false (lowercase)

        // Should get exactly 300 lowercase bases before exhausting the run
        for i in 0..300 {
            assert!(!scd.next_is_upper(), "base {} should be lowercase", i);
        }
        // After exhaustion, stays at current polarity (lowercase)
        assert!(
            !scd.next_is_upper(),
            "should remain lowercase after exhaustion"
        );
    }

    #[test]
    fn test_streaming_case_decoder_seek_to_checkpoint_exact_255() {
        // Edge case: remainder == 255 (max u8 value).
        let mut scd = StreamingCaseDecoder::new();
        scd.seek_to_checkpoint(255, false); // pre-toggle=false -> post=true (uppercase)

        for i in 0..255 {
            assert!(scd.next_is_upper(), "base {} should be uppercase", i);
        }
    }

    #[test]
    fn test_streaming_case_decoder_seek_then_feed() {
        // Seek to a checkpoint with remainder, then feed more RLE data.
        let mut scd = StreamingCaseDecoder::new();
        scd.seek_to_checkpoint(3, false); // 3 uppercase bases remaining

        // Feed: [5] means 5 lowercase bases
        scd.feed(&[5]);

        // 3 uppercase
        for _ in 0..3 {
            assert!(scd.next_is_upper());
        }
        // Then 5 lowercase
        for _ in 0..5 {
            assert!(!scd.next_is_upper());
        }
    }

    #[test]
    fn test_streaming_case_decoder_apply_case_u16_remainder() {
        // apply_case must correctly handle u16 remainder from seek_to_checkpoint.
        let mut scd = StreamingCaseDecoder::new();
        scd.seek_to_checkpoint(300, true); // post-toggle = !true = false (lowercase)

        let mut buf = vec![b'A'; 300];
        scd.apply_case(&mut buf);

        // All 300 should be lowercased
        assert!(
            buf.iter().all(|&b| b == b'a'),
            "all 300 bases should be lowercase"
        );
    }
}
