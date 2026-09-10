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

//! Microbenchmark comparing five 2-bit nucleotide encoding strategies:
//!   - "scalar":   naive byte-at-a-time LUT + shift packing (256-byte LUT)
//!   - "lut_64k":  2-bytes-at-a-time via 64KB precomputed table
//!   - "ssse3":    pshufb LUT encode -> maddubs/hadd/packus packing (old SIMD)
//!   - "clmul":    bitwise encode -> movemask + carry-less-square packing (new SIMD)
//!
//! Run with:  cargo bench --bench encode_2bit

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ---------------------------------------------------------------------------
// CGAT encoding table (shared)
// ---------------------------------------------------------------------------

/// CGAT: C=0, G=1, A=2, T=3
const TWO_BIT_ENCODE: [u8; 256] = {
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

// ---------------------------------------------------------------------------
// Scalar / slice-by-1 (256-byte LUT)
// ---------------------------------------------------------------------------

fn encode_scalar(src: &[u8], dst: &mut [u8]) {
    let mut di = 0;
    let mut si = 0;
    while si + 3 < src.len() {
        dst[di] = TWO_BIT_ENCODE[src[si] as usize]
            | (TWO_BIT_ENCODE[src[si + 1] as usize] << 2)
            | (TWO_BIT_ENCODE[src[si + 2] as usize] << 4)
            | (TWO_BIT_ENCODE[src[si + 3] as usize] << 6);
        si += 4;
        di += 1;
    }
}

// ---------------------------------------------------------------------------
// Slice-by-2 (64KB LUT) -- 2 ASCII bytes -> 4-bit packed nibble
// ---------------------------------------------------------------------------

fn build_lut_64k() -> Vec<u8> {
    let mut lut = vec![0u8; 65536];
    for lo in 0u16..256 {
        let c0 = TWO_BIT_ENCODE[lo as usize];
        if c0 == 0xFF {
            continue;
        }
        for hi in 0u16..256 {
            let c1 = TWO_BIT_ENCODE[hi as usize];
            if c1 == 0xFF {
                continue;
            }
            // Little-endian u16: lo byte is first base, hi byte is second.
            let idx = (hi << 8) | lo;
            lut[idx as usize] = c0 | (c1 << 2);
        }
    }
    lut
}

fn encode_lut64k(src: &[u8], dst: &mut [u8], lut: &[u8; 65536]) {
    let mut di = 0;
    let mut si = 0;
    while si + 3 < src.len() {
        let pair0 = u16::from_le_bytes([src[si], src[si + 1]]);
        let pair1 = u16::from_le_bytes([src[si + 2], src[si + 3]]);
        dst[di] = lut[pair0 as usize] | (lut[pair1 as usize] << 4);
        si += 4;
        di += 1;
    }
}

// ---------------------------------------------------------------------------
// SSSE3 (old) -- pshufb + maddubs + hadd + packus
// ---------------------------------------------------------------------------

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn encode_16_ssse3(src: &[u8]) -> [u8; 4] {
    use std::arch::x86_64::*;
    let lut = _mm_setr_epi8(-1, 2, -1, 0, 3, -1, -1, 1, -1, -1, -1, -1, -1, -1, -1, -1);
    let input = _mm_loadu_si128(src.as_ptr() as *const __m128i);
    let nibbles = _mm_and_si128(input, _mm_set1_epi8(0x0F));
    let codes = _mm_shuffle_epi8(lut, nibbles);
    // (skip validation for benchmark -- input is known-good)
    let weights = _mm_setr_epi8(1, 4, 16, 64, 1, 4, 16, 64, 1, 4, 16, 64, 1, 4, 16, 64);
    let pairs = _mm_maddubs_epi16(codes, weights);
    let quads = _mm_hadd_epi16(pairs, pairs);
    let packed_u8 = _mm_packus_epi16(quads, _mm_setzero_si128());
    let packed = _mm_extract_epi32(packed_u8, 0) as u32;
    packed.to_le_bytes()
}

#[cfg(target_arch = "x86_64")]
fn encode_ssse3(src: &[u8], dst: &mut [u8]) {
    let mut si = 0;
    let mut di = 0;
    while si + 16 <= src.len() {
        let packed = unsafe { encode_16_ssse3(&src[si..si + 16]) };
        dst[di..di + 4].copy_from_slice(&packed);
        si += 16;
        di += 4;
    }
}

// ---------------------------------------------------------------------------
// SSSE3 + validation (old, with the movemask branch)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn encode_16_ssse3_validated(src: &[u8]) -> Option<[u8; 4]> {
    use std::arch::x86_64::*;
    let lut = _mm_setr_epi8(-1, 2, -1, 0, 3, -1, -1, 1, -1, -1, -1, -1, -1, -1, -1, -1);
    let input = _mm_loadu_si128(src.as_ptr() as *const __m128i);
    let nibbles = _mm_and_si128(input, _mm_set1_epi8(0x0F));
    let codes = _mm_shuffle_epi8(lut, nibbles);
    if _mm_movemask_epi8(codes) != 0 {
        return None;
    }
    let weights = _mm_setr_epi8(1, 4, 16, 64, 1, 4, 16, 64, 1, 4, 16, 64, 1, 4, 16, 64);
    let pairs = _mm_maddubs_epi16(codes, weights);
    let quads = _mm_hadd_epi16(pairs, pairs);
    let packed_u8 = _mm_packus_epi16(quads, _mm_setzero_si128());
    let packed = _mm_extract_epi32(packed_u8, 0) as u32;
    Some(packed.to_le_bytes())
}

#[cfg(target_arch = "x86_64")]
fn encode_ssse3_validated(src: &[u8], dst: &mut [u8]) {
    let mut si = 0;
    let mut di = 0;
    while si + 16 <= src.len() {
        let packed = unsafe { encode_16_ssse3_validated(&src[si..si + 16]) }.unwrap();
        dst[di..di + 4].copy_from_slice(&packed);
        si += 16;
        di += 4;
    }
}

// ---------------------------------------------------------------------------
// CLMUL (new) -- bitwise + movemask + carry-less square
// ---------------------------------------------------------------------------

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3,pclmulqdq")]
unsafe fn encode_16_clmul(src: &[u8]) -> [u8; 4] {
    use std::arch::x86_64::*;
    let input = _mm_loadu_si128(src.as_ptr() as *const __m128i);

    // Validation (same pshufb -- needed for apples-to-apples with ssse3)
    let valid_lut = _mm_setr_epi8(-1, 0, -1, 0, 0, -1, -1, 0, -1, -1, -1, -1, -1, -1, -1, -1);
    let nibbles = _mm_and_si128(input, _mm_set1_epi8(0x0F));
    // Intentionally keep the validation pshufb (same work as ssse3 variant),
    // but its result does NOT feed into the packing chain.
    let _valid = _mm_movemask_epi8(_mm_shuffle_epi8(valid_lut, nibbles));

    let bit2_mask = _mm_movemask_epi8(_mm_slli_epi16(input, 5)) as u64;
    let bit1_inv = (!_mm_movemask_epi8(_mm_slli_epi16(input, 6))) as u64 & 0xFFFF;

    let spread_lo = _mm_clmulepi64_si128(
        _mm_set_epi64x(0, bit2_mask as i64),
        _mm_set_epi64x(0, bit2_mask as i64),
        0,
    );
    let spread_hi = _mm_clmulepi64_si128(
        _mm_set_epi64x(0, bit1_inv as i64),
        _mm_set_epi64x(0, bit1_inv as i64),
        0,
    );
    let packed = _mm_or_si128(spread_lo, _mm_slli_epi64(spread_hi, 1));
    (_mm_extract_epi32(packed, 0) as u32).to_le_bytes()
}

#[cfg(target_arch = "x86_64")]
fn encode_clmul(src: &[u8], dst: &mut [u8]) {
    let mut si = 0;
    let mut di = 0;
    while si + 16 <= src.len() {
        let packed = unsafe { encode_16_clmul(&src[si..si + 16]) };
        dst[di..di + 4].copy_from_slice(&packed);
        si += 16;
        di += 4;
    }
}

// ---------------------------------------------------------------------------
// CLMUL validated (new, with the branch)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3,pclmulqdq")]
unsafe fn encode_16_clmul_validated(src: &[u8]) -> Option<[u8; 4]> {
    use std::arch::x86_64::*;
    let input = _mm_loadu_si128(src.as_ptr() as *const __m128i);

    let valid_lut = _mm_setr_epi8(-1, 0, -1, 0, 0, -1, -1, 0, -1, -1, -1, -1, -1, -1, -1, -1);
    let nibbles = _mm_and_si128(input, _mm_set1_epi8(0x0F));
    if _mm_movemask_epi8(_mm_shuffle_epi8(valid_lut, nibbles)) != 0 {
        return None;
    }

    let bit2_mask = _mm_movemask_epi8(_mm_slli_epi16(input, 5)) as u64;
    let bit1_inv = (!_mm_movemask_epi8(_mm_slli_epi16(input, 6))) as u64 & 0xFFFF;

    let spread_lo = _mm_clmulepi64_si128(
        _mm_set_epi64x(0, bit2_mask as i64),
        _mm_set_epi64x(0, bit2_mask as i64),
        0,
    );
    let spread_hi = _mm_clmulepi64_si128(
        _mm_set_epi64x(0, bit1_inv as i64),
        _mm_set_epi64x(0, bit1_inv as i64),
        0,
    );
    let packed = _mm_or_si128(spread_lo, _mm_slli_epi64(spread_hi, 1));
    Some((_mm_extract_epi32(packed, 0) as u32).to_le_bytes())
}

#[cfg(target_arch = "x86_64")]
fn encode_clmul_validated(src: &[u8], dst: &mut [u8]) {
    let mut si = 0;
    let mut di = 0;
    while si + 16 <= src.len() {
        let packed = unsafe { encode_16_clmul_validated(&src[si..si + 16]) }.unwrap();
        dst[di..di + 4].copy_from_slice(&packed);
        si += 16;
        di += 4;
    }
}

// ---------------------------------------------------------------------------
// Benchmark harness
// ---------------------------------------------------------------------------

fn make_dna(len: usize) -> Vec<u8> {
    let bases = [b'C', b'G', b'A', b'T', b'c', b'g', b'a', b't'];
    let mut rng: u32 = 0xDEADBEEF;
    (0..len)
        .map(|_| {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            bases[((rng >> 16) & 7) as usize]
        })
        .collect()
}

fn bench_encode(c: &mut Criterion) {
    let sizes: &[usize] = &[16, 64, 256, 1024, 4096, 65536];
    let lut_64k_vec = build_lut_64k();
    let lut_64k: &[u8; 65536] = lut_64k_vec.as_slice().try_into().unwrap();

    let mut group = c.benchmark_group("encode_2bit");

    for &size in sizes {
        let src = make_dna(size);
        let mut dst = vec![0u8; size / 4];

        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |b, _| {
            b.iter(|| {
                encode_scalar(criterion::black_box(&src), criterion::black_box(&mut dst));
            });
        });

        group.bench_with_input(BenchmarkId::new("lut_64k", size), &size, |b, _| {
            b.iter(|| {
                encode_lut64k(
                    criterion::black_box(&src),
                    criterion::black_box(&mut dst),
                    criterion::black_box(lut_64k),
                );
            });
        });

        #[cfg(target_arch = "x86_64")]
        {
            group.bench_with_input(BenchmarkId::new("ssse3", size), &size, |b, _| {
                b.iter(|| {
                    encode_ssse3(criterion::black_box(&src), criterion::black_box(&mut dst));
                });
            });

            group.bench_with_input(BenchmarkId::new("ssse3_val", size), &size, |b, _| {
                b.iter(|| {
                    encode_ssse3_validated(
                        criterion::black_box(&src),
                        criterion::black_box(&mut dst),
                    );
                });
            });

            group.bench_with_input(BenchmarkId::new("clmul", size), &size, |b, _| {
                b.iter(|| {
                    encode_clmul(criterion::black_box(&src), criterion::black_box(&mut dst));
                });
            });

            group.bench_with_input(BenchmarkId::new("clmul_val", size), &size, |b, _| {
                b.iter(|| {
                    encode_clmul_validated(
                        criterion::black_box(&src),
                        criterion::black_box(&mut dst),
                    );
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
