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

//! rANS order-1 entropy coder for quality data compression.

use super::traits::{Compressor, Decompressor};
use std::io;

const TF_SHIFT: u32 = 11;
const CUMFREQ: usize = 1 << TF_SHIFT;
const RANS_INIT: u32 = 1 << 15;
const MAX_DECOMPRESS: usize = 256 << 20;

#[derive(Clone, Copy)]
#[repr(C)]
struct EncSymbol {
    x_max: u32,
    rcp_freq: u32,
    bias: u32,
    cmpl_freq: u16,
    rcp_shift: u16,
}

impl EncSymbol {
    const ZERO: Self = Self {
        x_max: 0,
        rcp_freq: 0,
        bias: 0,
        cmpl_freq: 0,
        rcp_shift: 0,
    };

    fn init(start: u32, freq: u32) -> Self {
        let x_max = ((RANS_INIT >> TF_SHIFT) << 16) * freq;
        let cmpl_freq = (CUMFREQ as u32 - freq) as u16;
        if freq < 2 {
            Self {
                x_max,
                rcp_freq: !0u32,
                rcp_shift: 32,
                bias: start + CUMFREQ as u32 - 1,
                cmpl_freq,
            }
        } else {
            let mut shift = 0u32;
            while freq > (1u32 << shift) {
                shift += 1;
            }
            let rcp_freq = (1u64 << (shift + 31)).div_ceil(freq as u64) as u32;
            Self {
                x_max,
                rcp_freq,
                rcp_shift: shift as u16 - 1 + 32,
                bias: start,
                cmpl_freq,
            }
        }
    }
}

// --- Decode lookup entry ---

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
struct DecEntry {
    sym: u8,
    freq: u16,
    bias: u16,
}

#[inline(always)]
unsafe fn rans_put(x: &mut u32, ptr: &mut *mut u8, sym: &EncSymbol) {
    if *x >= sym.x_max {
        *ptr = (*ptr).sub(2);
        (*ptr).cast::<u16>().write_unaligned((*x as u16).to_le());
        *x >>= 16;
    }
    let q = ((*x as u64 * sym.rcp_freq as u64) >> sym.rcp_shift) as u32;
    *x = *x + sym.bias + q * sym.cmpl_freq as u32;
}

#[inline(always)]
unsafe fn rans_renorm(x: &mut u32, ptr: &mut *const u8, end: *const u8) {
    if *x >= RANS_INIT {
        return;
    }
    if (*ptr).add(2) > end {
        // Out of input data -- force x to a valid state to avoid infinite loop.
        *x = RANS_INIT;
        return;
    }
    let val = u16::from_le((*ptr).cast::<u16>().read_unaligned()) as u32;
    *x = (*x << 16) | val;
    *ptr = (*ptr).add(2);
}

#[inline(always)]
unsafe fn write_u32le(ptr: &mut *mut u8, val: u32) {
    *ptr = (*ptr).sub(4);
    (*ptr).cast::<u32>().write_unaligned(val.to_le());
}

#[inline(always)]
unsafe fn read_u32le(ptr: &mut *const u8) -> u32 {
    let val = u32::from_le((*ptr).cast::<u32>().read_unaligned());
    *ptr = (*ptr).add(4);
    val
}

// --- Histogram ---

fn histo_o1(input: &[u8], f: &mut [[u32; 256]], t: &mut [u32; 256]) {
    let mut last = 0usize;
    for &b in input {
        let c = b as usize;
        // SAFETY: last and c are derived from u8 values (0..255), arrays are 256 wide.
        unsafe {
            *f.get_unchecked_mut(last).get_unchecked_mut(c) += 1;
            *t.get_unchecked_mut(last) += 1;
        }
        last = c;
    }
}

// --- Frequency normalization ---

fn normalize_freqs(f: &mut [[u32; 256]], t: &[u32; 256]) {
    for i in 0..256 {
        if t[i] == 0 {
            continue;
        }
        let scale = CUMFREQ as f64 / t[i] as f64;
        let mut t2 = 0i32;
        let mut max_freq = 0u32;
        let mut max_sym = 0usize;

        for j in 0..256 {
            if f[i][j] == 0 {
                continue;
            }
            if f[i][j] > max_freq {
                max_freq = f[i][j];
                max_sym = j;
            }
            f[i][j] = ((f[i][j] as f64 * scale) as u32).max(1);
            t2 += f[i][j] as i32;
        }

        let adjust = CUMFREQ as i32 - t2;
        if adjust > 0 {
            f[i][max_sym] += adjust as u32;
        } else if adjust < 0 {
            let neg = (-adjust) as u32;
            if f[i][max_sym] > neg {
                f[i][max_sym] -= neg;
            } else {
                let mut remain = neg as i32 - f[i][max_sym] as i32 + 1;
                f[i][max_sym] = 1;
                for j in 0..256 {
                    if remain <= 0 {
                        break;
                    }
                    if f[i][j] < 2 {
                        continue;
                    }
                    let take = (f[i][j] as i32 - 1).min(remain);
                    f[i][j] -= take as u32;
                    remain -= take;
                }
                assert!(
                    remain <= 0,
                    "frequency normalization failed: could not redistribute"
                );
            }
        }
    }
}

// --- Table serialization (zstd) ---

fn encode_freq_tables(f: &[[u32; 256]], t: &[u32; 256]) -> io::Result<Vec<u8>> {
    let mut raw = Vec::with_capacity(4096);

    for i in 0..256 {
        if t[i] == 0 {
            continue;
        }
        raw.push(i as u8);
        for j in 0..256 {
            if f[i][j] == 0 {
                continue;
            }
            raw.push(j as u8);
            raw.push((f[i][j] >> 8) as u8);
            raw.push((f[i][j] & 0xFF) as u8);
        }
        raw.push(0); // end of symbols for this context
    }
    raw.push(0); // end of contexts

    let compressed = zstd::bulk::compress(&raw, 22)
        .map_err(|e| io::Error::other(format!("zstd table compress: {}", e)))?;

    let sz = compressed.len();
    if sz > 0xFFFFFF {
        return Err(io::Error::other("frequency table too large"));
    }
    let mut result = Vec::with_capacity(3 + sz);
    result.push((sz & 0xFF) as u8);
    result.push(((sz >> 8) & 0xFF) as u8);
    result.push(((sz >> 16) & 0xFF) as u8);
    result.extend_from_slice(&compressed);
    Ok(result)
}

fn decode_freq_tables(data: &[u8], pos: &mut usize, dec: &mut [DecEntry]) -> io::Result<()> {
    let p = *pos;
    if p + 3 > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated table header",
        ));
    }
    let sz = data[p] as usize | ((data[p + 1] as usize) << 8) | ((data[p + 2] as usize) << 16);
    *pos = p + 3;

    if *pos + sz > data.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated table data",
        ));
    }

    let raw = zstd::bulk::decompress(&data[*pos..*pos + sz], 0x70000)
        .map_err(|e| io::Error::other(format!("zstd table decompress: {}", e)))?;
    *pos += sz;

    let mut rp = 0usize;
    let end = raw.len();

    macro_rules! get {
        () => {{
            if rp >= end {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated frequency table",
                ));
            }
            let v = raw[rp];
            rp += 1;
            v
        }};
    }

    let mut i = get!() as usize;
    loop {
        let mut x = 0usize;
        let mut j = get!() as usize;

        loop {
            let freq = ((get!() as u32) << 8) | get!() as u32;
            let freq = if freq == 0 { CUMFREQ as u32 } else { freq };

            if x + freq as usize > CUMFREQ {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "frequency table overflow",
                ));
            }

            let base = i * CUMFREQ + x;
            for y in 0..freq as usize {
                dec[base + y] = DecEntry {
                    sym: j as u8,
                    freq: freq as u16,
                    bias: y as u16,
                };
            }
            x += freq as usize;

            j = get!() as usize;
            if j == 0 {
                break;
            }
        }

        i = get!() as usize;
        if i == 0 {
            break;
        }
    }

    Ok(())
}

// --- Main encoder ---

fn rans_bound(size: usize) -> usize {
    size + size / 20 + 0x31000
}

fn rans_compress_block(input: &[u8]) -> io::Result<Vec<u8>> {
    let in_size = input.len();
    if in_size == 0 {
        return Ok(vec![0, 0, 0, 0]);
    }

    if in_size < 4 {
        let mut out = Vec::with_capacity(4 + in_size);
        out.extend_from_slice(&(in_size as u32).to_le_bytes());
        out.extend_from_slice(input);
        return Ok(out);
    }

    let mut f = vec![[0u32; 256]; 256];
    let mut t = [0u32; 256];
    histo_o1(input, &mut f, &mut t);

    let isz4 = in_size >> 2;
    f[0][input[isz4] as usize] += 1;
    f[0][input[2 * isz4] as usize] += 1;
    f[0][input[3 * isz4] as usize] += 1;
    t[0] += 3;

    normalize_freqs(&mut f, &t);

    let mut syms = vec![[EncSymbol::ZERO; 256]; 256];
    for i in 0..256 {
        if t[i] == 0 {
            continue;
        }
        let mut x = 0u32;
        for j in 0..256 {
            if f[i][j] == 0 {
                continue;
            }
            syms[i][j] = EncSymbol::init(x, f[i][j]);
            x += f[i][j];
        }
    }

    let table_data = encode_freq_tables(&f, &t)?;

    let bound = rans_bound(in_size);
    let total_alloc = 4 + table_data.len() + bound;
    let mut out = vec![0u8; total_alloc];
    out[..4].copy_from_slice(&(in_size as u32).to_le_bytes());
    let tab_end = 4 + table_data.len();
    out[4..tab_end].copy_from_slice(&table_data);

    // SAFETY: All pointer writes stay within out[tab_end..total_alloc].
    // rans_bound() guarantees the buffer fits worst-case rANS expansion.
    // Input reads use indices 0..in_size-1. Symbol table lookups use
    // indices 0..255 derived from u8 input bytes.
    unsafe {
        let inp = input.as_ptr();
        let mut ptr = out.as_mut_ptr().add(total_alloc);

        let mut r0 = RANS_INIT;
        let mut r1 = RANS_INIT;
        let mut r2 = RANS_INIT;
        let mut r3 = RANS_INIT;

        let mut l0 = *inp.add(isz4 - 1) as usize;
        let mut l1 = *inp.add(2 * isz4 - 1) as usize;
        let mut l2 = *inp.add(3 * isz4 - 1) as usize;
        let mut l3 = *inp.add(in_size - 1) as usize;

        let isz4_i = isz4 as isize;
        let mut i3 = in_size as isize - 2;
        while i3 > 4 * isz4_i - 2 {
            let c3 = *inp.add(i3 as usize) as usize;
            rans_put(&mut r3, &mut ptr, syms.get_unchecked(c3).get_unchecked(l3));
            l3 = c3;
            i3 -= 1;
        }

        let mut i0 = isz4_i - 2;
        let mut i1 = 2 * isz4_i - 2;
        let mut i2 = 3 * isz4_i - 2;

        while i0 >= 0 {
            let c3 = *inp.add(i3 as usize) as usize;
            let c2 = *inp.add(i2 as usize) as usize;
            let c1 = *inp.add(i1 as usize) as usize;
            let c0 = *inp.add(i0 as usize) as usize;

            rans_put(&mut r3, &mut ptr, syms.get_unchecked(c3).get_unchecked(l3));
            rans_put(&mut r2, &mut ptr, syms.get_unchecked(c2).get_unchecked(l2));
            rans_put(&mut r1, &mut ptr, syms.get_unchecked(c1).get_unchecked(l1));
            rans_put(&mut r0, &mut ptr, syms.get_unchecked(c0).get_unchecked(l0));

            l3 = c3;
            l2 = c2;
            l1 = c1;
            l0 = c0;
            i0 -= 1;
            i1 -= 1;
            i2 -= 1;
            i3 -= 1;
        }

        rans_put(&mut r3, &mut ptr, syms.get_unchecked(0).get_unchecked(l3));
        rans_put(&mut r2, &mut ptr, syms.get_unchecked(0).get_unchecked(l2));
        rans_put(&mut r1, &mut ptr, syms.get_unchecked(0).get_unchecked(l1));
        rans_put(&mut r0, &mut ptr, syms.get_unchecked(0).get_unchecked(l0));

        write_u32le(&mut ptr, r3);
        write_u32le(&mut ptr, r2);
        write_u32le(&mut ptr, r1);
        write_u32le(&mut ptr, r0);

        let rans_start = ptr.offset_from(out.as_ptr()) as usize;
        let rans_len = total_alloc - rans_start;
        out.copy_within(rans_start..total_alloc, tab_end);
        out.truncate(tab_end + rans_len);
    }

    Ok(out)
}

// --- Main decoder ---

fn rans_decompress_block(input: &[u8]) -> io::Result<Vec<u8>> {
    if input.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated rANS block",
        ));
    }

    let out_sz = u32::from_le_bytes([input[0], input[1], input[2], input[3]]) as usize;
    if out_sz == 0 {
        return Ok(Vec::new());
    }

    if out_sz < 4 {
        if input.len() < 4 + out_sz {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated literal rANS frame",
            ));
        }
        return Ok(input[4..4 + out_sz].to_vec());
    }

    if out_sz > MAX_DECOMPRESS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "rANS decompressed size exceeds limit",
        ));
    }

    let mut dec_table = vec![DecEntry::default(); 256 * CUMFREQ];
    let mut pos = 4usize;
    decode_freq_tables(input, &mut pos, &mut dec_table)?;

    if pos + 16 > input.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "truncated rANS state",
        ));
    }

    let mut output = vec![0u8; out_sz];
    let isz4 = out_sz >> 2;

    // SAFETY: Frequency tables are validated by decode_freq_tables (sums
    // checked, bounds checked). Decode table indices are l*CUMFREQ+m where
    // l < 256 (from decoded u8 symbols) and m < CUMFREQ (masked). Output
    // indices p0..p3 stay within [0, out_sz). Initial rANS state read
    // validated above (pos+16 <= input.len()).
    // Renormalization bounds: in_end tracks the safe read limit so that
    // rans_renorm never reads past the input buffer.
    let in_end = input.as_ptr().wrapping_add(input.len());
    unsafe {
        let mut in_ptr = input.as_ptr().add(pos);

        let mut r0 = read_u32le(&mut in_ptr);
        let mut r1 = read_u32le(&mut in_ptr);
        let mut r2 = read_u32le(&mut in_ptr);
        let mut r3 = read_u32le(&mut in_ptr);

        let dec = dec_table.as_ptr();
        let out = output.as_mut_ptr();
        let mask = CUMFREQ as u32 - 1;

        let mut l0 = 0usize;
        let mut l1 = 0usize;
        let mut l2 = 0usize;
        let mut l3 = 0usize;

        let mut p0 = 0usize;
        let mut p1 = isz4;
        let mut p2 = 2 * isz4;
        let mut p3 = 3 * isz4;

        for _ in 0..isz4 {
            let e0 = &*dec.add(l0 * CUMFREQ + (r0 & mask) as usize);
            *out.add(p0) = e0.sym;
            r0 = e0.freq as u32 * (r0 >> TF_SHIFT) + e0.bias as u32;

            let e1 = &*dec.add(l1 * CUMFREQ + (r1 & mask) as usize);
            *out.add(p1) = e1.sym;
            r1 = e1.freq as u32 * (r1 >> TF_SHIFT) + e1.bias as u32;

            let e2 = &*dec.add(l2 * CUMFREQ + (r2 & mask) as usize);
            *out.add(p2) = e2.sym;
            r2 = e2.freq as u32 * (r2 >> TF_SHIFT) + e2.bias as u32;

            let e3 = &*dec.add(l3 * CUMFREQ + (r3 & mask) as usize);
            *out.add(p3) = e3.sym;
            r3 = e3.freq as u32 * (r3 >> TF_SHIFT) + e3.bias as u32;

            rans_renorm(&mut r0, &mut in_ptr, in_end);
            rans_renorm(&mut r1, &mut in_ptr, in_end);
            rans_renorm(&mut r2, &mut in_ptr, in_end);
            rans_renorm(&mut r3, &mut in_ptr, in_end);

            l0 = e0.sym as usize;
            l1 = e1.sym as usize;
            l2 = e2.sym as usize;
            l3 = e3.sym as usize;

            p0 += 1;
            p1 += 1;
            p2 += 1;
            p3 += 1;
        }

        while p3 < out_sz {
            let e3 = &*dec.add(l3 * CUMFREQ + (r3 & mask) as usize);
            *out.add(p3) = e3.sym;
            r3 = e3.freq as u32 * (r3 >> TF_SHIFT) + e3.bias as u32;
            rans_renorm(&mut r3, &mut in_ptr, in_end);
            l3 = e3.sym as usize;
            p3 += 1;
        }
    }

    Ok(output)
}

// --- Compressor / Decompressor trait impls ---

pub struct RansCompressor;

impl Default for RansCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl RansCompressor {
    pub fn new() -> Self {
        Self
    }
}

impl Compressor for RansCompressor {
    fn compress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        // Chunk into frames that stay within MAX_DECOMPRESS.
        // Each frame: compressed_len(u32 LE) + rans_block.
        for chunk in input.chunks(MAX_DECOMPRESS) {
            let compressed = rans_compress_block(chunk)?;
            output.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
            output.extend_from_slice(&compressed);
        }
        Ok(())
    }

    fn finish(&mut self, _output: &mut Vec<u8>) -> io::Result<()> {
        Ok(())
    }

    fn reset(&mut self) {}

    fn compress_memory(&self, input_size: usize) -> usize {
        // Encode tables (~1.3 MB) + histogram (~256 KB) + output buffer
        rans_bound(input_size) + 2 * 1024 * 1024
    }
}

pub struct RansDecompressor;

impl Default for RansDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl RansDecompressor {
    pub fn new() -> Self {
        Self
    }
}

impl Decompressor for RansDecompressor {
    fn decompress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        // Read framed blocks: compressed_len(u32 LE) + rans_block.
        let mut pos = 0;
        while pos < input.len() {
            if pos + 4 > input.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated rANS frame header",
                ));
            }
            let block_len = u32::from_le_bytes(input[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;
            if pos + block_len > input.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated rANS frame data",
                ));
            }
            let decoded = rans_decompress_block(&input[pos..pos + block_len])?;
            output.extend_from_slice(&decoded);
            pos += block_len;
        }
        Ok(())
    }

    fn reset(&mut self) {}

    fn decompress_memory(&self, output_size: usize) -> usize {
        // Decode tables (~2.6 MB) + output buffer
        output_size + 3 * 1024 * 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rans_round_trip_simple() {
        let data = b"AAABBBCCCDDDEEEFFFGGG";
        let compressed = rans_compress_block(data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_rans_round_trip_quality() {
        // Simulate FASTQ quality scores (ASCII 33-73)
        let mut data = Vec::with_capacity(10000);
        let mut x: u32 = 42;
        for _ in 0..10000 {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            data.push(33 + ((x >> 16) % 41) as u8);
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn test_rans_round_trip_all_bytes() {
        let mut data = Vec::with_capacity(256 * 4);
        for _ in 0..4 {
            for b in 0..=255u8 {
                data.push(b);
            }
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_rans_round_trip_repeated() {
        let data = vec![42u8; 10000];
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_rans_empty() {
        let compressed = rans_compress_block(&[]).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_rans_compressor_trait() {
        let data: Vec<u8> = (0..5000).map(|i| (i % 94 + 33) as u8).collect();
        let mut comp = RansCompressor::new();
        let mut compressed = Vec::new();
        comp.compress(&data, &mut compressed).unwrap();
        comp.finish(&mut compressed).unwrap();

        let mut decomp = RansDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&compressed, &mut result).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_rans_reset() {
        let mut comp = RansCompressor::new();

        let data_a = vec![65u8; 1000];
        let mut comp_a = Vec::new();
        comp.compress(&data_a, &mut comp_a).unwrap();
        comp.finish(&mut comp_a).unwrap();
        comp.reset();

        let data_b = vec![66u8; 1000];
        let mut comp_b = Vec::new();
        comp.compress(&data_b, &mut comp_b).unwrap();
        comp.finish(&mut comp_b).unwrap();

        let mut decomp = RansDecompressor::new();
        let mut result_a = Vec::new();
        decomp.decompress(&comp_a, &mut result_a).unwrap();
        assert_eq!(result_a, data_a);

        let mut result_b = Vec::new();
        decomp.decompress(&comp_b, &mut result_b).unwrap();
        assert_eq!(result_b, data_b);
    }

    // --- Size / edge-case tests ---

    #[test]
    fn test_rans_small_sizes() {
        // Exercise every size from 1..=32 to catch off-by-ones in the
        // 4-way interleave and tail handling.
        for n in 1..=32 {
            let data: Vec<u8> = (0..n).map(|i| (i % 7 + 40) as u8).collect();
            let compressed = rans_compress_block(&data).unwrap();
            let decompressed = rans_decompress_block(&compressed).unwrap();
            assert_eq!(decompressed, data, "round-trip failed for size {}", n);
        }
    }

    #[test]
    fn test_rans_size_not_multiple_of_4() {
        // The 4-way interleave splits into quarters; sizes that are not a
        // multiple of 4 exercise the tail path.
        for &n in &[5, 7, 13, 17, 31, 63, 127, 255, 1001, 4093, 10007] {
            let data: Vec<u8> = (0..n)
                .map(|i| (i as u8).wrapping_mul(37).wrapping_add(33))
                .collect();
            let compressed = rans_compress_block(&data).unwrap();
            let decompressed = rans_decompress_block(&compressed).unwrap();
            assert_eq!(decompressed, data, "round-trip failed for size {}", n);
        }
    }

    #[test]
    fn test_rans_single_byte_input() {
        let data = vec![99u8];
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_rans_two_bytes() {
        let data = vec![0u8, 255];
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    // --- Alphabet / distribution tests ---

    #[test]
    fn test_rans_binary_alphabet() {
        // Only two symbols, like a binary quality flag.
        let mut data = Vec::with_capacity(8000);
        let mut x: u32 = 1;
        for _ in 0..8000 {
            x = x.wrapping_mul(2654435769).wrapping_add(1);
            data.push(if (x >> 31) != 0 { b'!' } else { b'~' });
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
        assert!(compressed.len() < data.len(), "binary data should compress");
    }

    #[test]
    fn test_rans_skewed_distribution() {
        // One dominant symbol (like high-quality reads where most scores are
        // the same) -- stresses the single-symbol-per-context path.
        let mut data = vec![b'F'; 50000];
        let mut x: u32 = 7;
        for byte in data.iter_mut() {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            if (x >> 16) % 100 == 0 {
                *byte = b'!' + ((x >> 24) % 10) as u8;
            }
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
        assert!(compressed.len() < data.len() / 4);
    }

    #[test]
    fn test_rans_uniform_256() {
        // All 256 byte values equally likely -- worst case for the frequency
        // normalization (many symbols sharing CUMFREQ).
        let mut data = Vec::with_capacity(256 * 100);
        for _ in 0..100 {
            for b in 0..=255u8 {
                data.push(b);
            }
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_rans_strong_order1_correlation() {
        // Data with very strong order-1 structure: ascending runs.
        // rANS-o1 should exploit this much better than order-0.
        let mut data = Vec::with_capacity(10000);
        for _ in 0..100 {
            for b in 33..=132u8 {
                data.push(b);
            }
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
        // With perfect order-1 context each symbol is deterministic,
        // so compression should be extremely good.
        assert!(
            compressed.len() < data.len() / 8,
            "expected >8x compression on deterministic o1 data, got {} -> {}",
            data.len(),
            compressed.len()
        );
    }

    // --- Indexed-mode (multi-block) tests ---

    #[test]
    fn test_rans_indexed_blocks_independent() {
        // Simulate indexed mode: compress many blocks with reset() between
        // each, then verify every block decompresses independently.
        let mut comp = RansCompressor::new();
        let blocks: Vec<Vec<u8>> = (0..8)
            .map(|i| {
                (0..2000u32)
                    .map(|j| (33 + ((j.wrapping_mul(7).wrapping_add(i * 1337)) % 41)) as u8)
                    .collect()
            })
            .collect();

        let mut compressed_blocks: Vec<Vec<u8>> = Vec::new();
        for block in &blocks {
            let mut compressed = Vec::new();
            comp.compress(block, &mut compressed).unwrap();
            comp.finish(&mut compressed).unwrap();
            comp.reset();
            compressed_blocks.push(compressed);
        }

        for (i, (compressed, original)) in compressed_blocks.iter().zip(blocks.iter()).enumerate() {
            let mut decomp = RansDecompressor::new();
            let mut result = Vec::new();
            decomp
                .decompress(compressed, &mut result)
                .unwrap_or_else(|e| panic!("block {} decompress failed: {}", i, e));
            assert_eq!(
                result, *original,
                "block {} content mismatch (reset not working)",
                i
            );
        }
    }

    #[test]
    fn test_rans_reset_produces_identical_output() {
        let data: Vec<u8> = (0..4096).map(|i| (33 + i % 41) as u8).collect();
        let mut comp = RansCompressor::new();

        let mut out1 = Vec::new();
        comp.compress(&data, &mut out1).unwrap();
        comp.finish(&mut out1).unwrap();
        comp.reset();

        let mut out2 = Vec::new();
        comp.compress(&data, &mut out2).unwrap();
        comp.finish(&mut out2).unwrap();

        assert_eq!(
            out1, out2,
            "reset should produce identical output for identical input"
        );
    }

    // --- Realistic quality data tests ---

    #[test]
    fn test_rans_illumina_quality_pattern() {
        // Simulate Illumina-like quality: high at start, dropping toward end,
        // with occasional dips.  Typical 150-bp reads.
        let mut data = Vec::new();
        let mut rng: u32 = 0xDEADBEEF;
        for _ in 0..1000 {
            for pos in 0..150 {
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                let base_q: u8 = if pos < 10 {
                    30
                } else if pos < 100 {
                    35
                } else {
                    25
                };
                let noise = ((rng >> 16) % 11) as i8 - 5;
                let q = (base_q as i8 + noise).clamp(2, 41) as u8;
                data.push(q + 33); // Phred+33 encoding
            }
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
        assert!(
            compressed.len() < data.len(),
            "quality data should compress: {} -> {}",
            data.len(),
            compressed.len()
        );
    }

    #[test]
    fn test_rans_nanopore_quality_pattern() {
        // Nanopore: wider quality distribution, more variable, longer reads.
        let mut data = Vec::new();
        let mut rng: u32 = 0xCAFEBABE;
        for _ in 0..100 {
            // Variable read length 500-2000
            rng = rng.wrapping_mul(2654435769).wrapping_add(1);
            let read_len = 500 + ((rng >> 16) % 1501) as usize;
            for _ in 0..read_len {
                rng = rng.wrapping_mul(2654435769).wrapping_add(1);
                let q = ((rng >> 16) % 30) as u8 + 2;
                data.push(q + 33);
            }
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    // --- Compression effectiveness ---

    #[test]
    fn test_rans_compresses_quality_better_than_raw() {
        // Verify that rANS actually achieves meaningful compression on
        // realistic quality strings, not just correctness.
        let mut data = Vec::with_capacity(100_000);
        let mut rng: u32 = 42;
        for _ in 0..100_000 {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            data.push(33 + ((rng >> 16) % 41) as u8);
        }

        let compressed = rans_compress_block(&data).unwrap();
        let ratio = data.len() as f64 / compressed.len() as f64;
        assert!(
            ratio > 1.4,
            "expected at least 1.4x compression on quality data, got {:.2}x ({} -> {})",
            ratio,
            data.len(),
            compressed.len()
        );
    }

    // --- Decompressor edge cases ---

    #[test]
    fn test_rans_decompressor_empty_input() {
        let mut decomp = RansDecompressor::new();
        let mut result = Vec::new();
        decomp.decompress(&[], &mut result).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_rans_decompressor_corrupt_magic() {
        let mut decomp = RansDecompressor::new();
        let mut result = Vec::new();
        // 3 bytes is too short (need at least 4 for the size header).
        assert!(decomp.decompress(&[0xFF, 0xFF, 0xFF], &mut result).is_err());
    }

    // --- Table round-trip ---

    #[test]
    fn test_rans_freq_table_round_trip() {
        // Build frequency tables, encode them, decode them, and verify
        // the decode lookup tables produce the correct symbol mappings.
        let mut f = vec![[0u32; 256]; 256];
        let mut t = [0u32; 256];

        // Context 0 -> symbols 33, 34, 35
        f[0][33] = 100;
        f[0][34] = 200;
        f[0][35] = 300;
        t[0] = 600;

        // Context 33 -> symbol 33 (self-loop)
        f[33][33] = 500;
        t[33] = 500;

        normalize_freqs(&mut f, &t);

        let table_data = encode_freq_tables(&f, &t).unwrap();
        let mut dec_table = vec![DecEntry::default(); 256 * CUMFREQ];
        let mut pos = 0usize;
        decode_freq_tables(&table_data, &mut pos, &mut dec_table).unwrap();
        assert_eq!(pos, table_data.len());

        // Verify context 33: all entries should map to symbol 33
        for m in 0..CUMFREQ {
            assert_eq!(
                dec_table[33 * CUMFREQ + m].sym,
                33,
                "context 33 entry {} should map to symbol 33",
                m
            );
        }

        // Verify context 0: entries should map to 33, 34, or 35
        let mut seen = [false; 256];
        for m in 0..CUMFREQ {
            let sym = dec_table[0 * CUMFREQ + m].sym;
            assert!(
                sym == 33 || sym == 34 || sym == 35,
                "context 0 entry {} mapped to unexpected symbol {}",
                m,
                sym
            );
            seen[sym as usize] = true;
        }
        assert!(seen[33] && seen[34] && seen[35]);
    }

    // --- Large block ---

    #[test]
    fn test_rans_large_block() {
        // 1 MiB block -- matches the default block budget.
        let n = 1024 * 1024;
        let mut data = Vec::with_capacity(n);
        let mut rng: u32 = 0x12345678;
        for _ in 0..n {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            data.push(33 + ((rng >> 16) % 41) as u8);
        }
        let compressed = rans_compress_block(&data).unwrap();
        let decompressed = rans_decompress_block(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_rans_truncated_input_does_not_crash() {
        // Compress valid data, then truncate the compressed output.
        // The decoder should either return an error or produce output
        // without crashing (no UB from reading past buffer).
        let data: Vec<u8> = (0..1000).map(|i| (33 + i % 41) as u8).collect();
        let compressed = rans_compress_block(&data).unwrap();

        // Try progressively shorter truncations
        for trunc_len in [4, 8, 16, 32, compressed.len() / 2] {
            if trunc_len >= compressed.len() {
                continue;
            }
            let truncated = &compressed[..trunc_len];
            // Should not panic -- may return Ok (partial) or Err
            let _ = rans_decompress_block(truncated);
        }
    }

    #[test]
    fn test_rans_oversized_output_rejected() {
        // Craft a 4-byte header claiming MAX_DECOMPRESS+1 output size.
        let huge_sz = (MAX_DECOMPRESS as u32 + 1).to_le_bytes();
        let result = rans_decompress_block(&huge_sz);
        assert!(
            result.is_err(),
            "should reject output size exceeding MAX_DECOMPRESS"
        );
    }
}
