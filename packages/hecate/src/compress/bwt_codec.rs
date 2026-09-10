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

use crate::compress::traits::{Compressor, Decompressor};
use libsais_sys::libsais::{
    libsais_bwt_aux_omp, libsais_bwt_omp, libsais_unbwt_aux_omp, libsais_unbwt_omp,
};
use libsais_sys::libsais64::{
    libsais64_bwt_aux_omp, libsais64_bwt_omp, libsais64_unbwt_aux_omp, libsais64_unbwt_omp,
};
use memmap2::MmapMut;
use rayon::prelude::*;
use std::io;

fn mmap_zeroed(size: usize) -> io::Result<MmapMut> {
    let mut mmap = MmapMut::map_anon(size)?;
    #[cfg(target_os = "linux")]
    unsafe {
        libc::madvise(
            mmap.as_mut_ptr() as *mut libc::c_void,
            size,
            libc::MADV_HUGEPAGE,
        );
    }
    Ok(mmap)
}

const CHUNK_SIZE: usize = 16 * 1024 * 1024;
const SMALL_BLOCK_THRESHOLD: usize = 32 * 1024;

#[derive(Clone, Copy, Debug)]
struct OpenMpState {
    max_threads: usize,
    thread_limit: usize,
}

fn positive_openmp_value(value: std::os::raw::c_int) -> usize {
    if value > 0 {
        value as usize
    } else {
        1
    }
}

fn openmp_state() -> OpenMpState {
    let max_threads = unsafe { openmp_sys::ffi::omp_get_max_threads() };
    let thread_limit = unsafe { openmp_sys::ffi::omp_get_thread_limit() };

    OpenMpState {
        max_threads: positive_openmp_value(max_threads),
        thread_limit: positive_openmp_value(thread_limit),
    }
}

fn compute_aux_stride(size: usize) -> usize {
    let mut v = size / 8;
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v >>= 1;
    v + 1
}

fn update0(p: &mut u16, x: u32) {
    *p = p.wrapping_sub(*p >> x);
}

fn update1(p: &mut u16, x: u32) {
    *p = p.wrapping_add(((*p) ^ 65535) >> x);
}

fn init_contexts(c0: &mut [u16; 256], c1: &mut [[u16; 256]; 256], c2: &mut [[u16; 17]; 512]) {
    c0.fill(1 << 15);
    for row in c1.iter_mut() {
        row.fill(1 << 15);
    }
    for f in 0..2 {
        for ctx in 0..256 {
            for k in 0..17 {
                c2[2 * ctx + f][k] = ((k as u16) << 12).wrapping_sub((k == 16) as u16);
            }
        }
    }
}

fn arithmetic_encode(input: &[u8], output: &mut Vec<u8>) {
    let mut c0 = [0u16; 256];
    let mut c1 = [[0u16; 256]; 256];
    let mut c2 = [[0u16; 17]; 512];
    init_contexts(&mut c0, &mut c1, &mut c2);

    let mut high: u32 = 0xFFFF_FFFF;
    let mut low: u32 = 0;
    let mut prev1: u32 = 0;
    let mut prev2: u32 = 0;
    let mut run: u32 = 0;

    for &b in input {
        let mut byte = b;
        if prev1 == prev2 {
            run += 1;
        } else {
            run = 0;
        }
        let f = (run > 2) as usize;
        let mut ctx: usize = 1;
        while ctx < 256 {
            let p0 = c0[ctx] as u32;
            let p1 = c1[prev1 as usize][ctx] as u32;
            let p2 = c1[prev2 as usize][ctx] as u32;
            let p = ((p0 + p1) * 6 + p2 * 4) >> 4;
            let j = (p >> 12) as usize;
            let x1 = c2[2 * ctx + f][j] as i32;
            let x2 = c2[2 * ctx + f][j + 1] as i32;
            let ssep = x1 + (((x2 - x1) * (p as i32 & 4095)) >> 12);

            if (byte & 128) != 0 {
                let range = (high as u64).wrapping_sub(low as u64);
                high = (low as u64 + ((range * ((ssep * 2 + p as i32 * 2) as u64)) >> 18)) as u32;
                while (low ^ high) < (1 << 24) {
                    output.push((low >> 24) as u8);
                    low <<= 8;
                    high = (high << 8) + 0xFF;
                }
                update1(&mut c0[ctx], 3);
                update1(&mut c1[prev1 as usize][ctx], 5);
                update1(&mut c2[2 * ctx + f][j], 7);
                update1(&mut c2[2 * ctx + f][j + 1], 7);
                ctx = ctx + ctx + 1;
            } else {
                let range = (high as u64).wrapping_sub(low as u64);
                low =
                    (low as u64 + ((range * ((ssep * 2 + p as i32 * 2) as u64)) >> 18) + 1) as u32;
                while (low ^ high) < (1 << 24) {
                    output.push((low >> 24) as u8);
                    low <<= 8;
                    high = (high << 8) + 0xFF;
                }
                update0(&mut c0[ctx], 3);
                update0(&mut c1[prev1 as usize][ctx], 5);
                update0(&mut c2[2 * ctx + f][j], 7);
                update0(&mut c2[2 * ctx + f][j + 1], 7);
                ctx += ctx;
            }
            byte <<= 1;
        }
        prev2 = prev1;
        prev1 = (ctx & 255) as u32;
    }

    output.push((low >> 24) as u8);
    low <<= 8;
    output.push((low >> 24) as u8);
    low <<= 8;
    output.push((low >> 24) as u8);
    low <<= 8;
    output.push((low >> 24) as u8);
}

fn arithmetic_decode(input: &[u8], expected: usize, output: &mut [u8]) -> io::Result<()> {
    let mut c0 = [0u16; 256];
    let mut c1 = [[0u16; 256]; 256];
    let mut c2 = [[0u16; 17]; 512];
    init_contexts(&mut c0, &mut c1, &mut c2);

    let mut high: u32 = 0xFFFF_FFFF;
    let mut low: u32 = 0;
    let mut code: u32 = 0;
    let mut in_pos: usize = 0;

    for _ in 0..4 {
        code = (code << 8)
            + *input.get(in_pos).ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "truncated arithmetic stream")
            })? as u32;
        in_pos += 1;
    }

    let mut prev1: u32 = 0;
    let mut prev2: u32 = 0;
    let mut run: u32 = 0;

    for out_byte in output.iter_mut().take(expected) {
        if prev1 == prev2 {
            run += 1;
        } else {
            run = 0;
        }
        let f = (run > 2) as usize;
        let mut ctx: usize = 1;
        while ctx < 256 {
            let p0 = c0[ctx] as u32;
            let p1 = c1[prev1 as usize][ctx] as u32;
            let p2 = c1[prev2 as usize][ctx] as u32;
            let p = ((p0 + p1) * 6 + p2 * 4) >> 4;
            let j = (p >> 12) as usize;
            let x1 = c2[2 * ctx + f][j] as i32;
            let x2 = c2[2 * ctx + f][j + 1] as i32;
            let ssep = x1 + (((x2 - x1) * (p as i32 & 4095)) >> 12);

            let mid = (low as u64).wrapping_add(
                ((high as u64 - low as u64) * ((ssep * 2 + p as i32 * 2) as u64)) >> 18,
            ) as u32;
            let bit = code <= mid;
            if bit {
                high = mid;
            } else {
                low = mid.wrapping_add(1);
            }
            while (low ^ high) < (1 << 24) {
                low <<= 8;
                high = (high << 8) + 255;
                let next = input.get(in_pos).copied().unwrap_or(0);
                code = (code << 8) + next as u32;
                in_pos += 1;
            }
            if bit {
                update1(&mut c0[ctx], 3);
                update1(&mut c1[prev1 as usize][ctx], 5);
                update1(&mut c2[2 * ctx + f][j], 7);
                update1(&mut c2[2 * ctx + f][j + 1], 7);
                ctx = ctx + ctx + 1;
            } else {
                update0(&mut c0[ctx], 3);
                update0(&mut c1[prev1 as usize][ctx], 5);
                update0(&mut c2[2 * ctx + f][j], 7);
                update0(&mut c2[2 * ctx + f][j + 1], 7);
                ctx += ctx;
            }
        }
        prev2 = prev1;
        prev1 = (ctx & 255) as u32;
        *out_byte = prev1 as u8;
    }

    Ok(())
}

/// BWT-based compressor using libsais and arithmetic coding.
/// Magic byte for 32-bit SA blocks.
const BWT_MAGIC_32: u8 = b'B';
/// Magic byte for 64-bit SA blocks.
const BWT_MAGIC_64: u8 = b'b';

pub struct BwtCompressor {
    block_size_mib: u16,
    block_bytes: usize,
    cores: usize,
}

impl BwtCompressor {
    pub fn new(block_size_mib: u16, cores: usize) -> io::Result<Self> {
        if block_size_mib == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "block size must be at least 1 MiB",
            ));
        }
        let block_bytes = block_size_mib as usize * 1024 * 1024;
        let omp = openmp_state();
        Ok(Self {
            block_size_mib,
            block_bytes,
            cores: cores
                .max(1)
                .min(omp.max_threads.min(omp.thread_limit).max(1)),
        })
    }

    fn compress_block(&self, block: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        let size = block.len();
        if size == 0 {
            return Ok(());
        }

        // Dispatch to 64-bit libsais when the actual block exceeds i32
        // range.  This is keyed on the data, not the configured block_mib,
        // so every call site gets the right SA width.
        let use_64bit = size > i32::MAX as usize;
        let small_block = size < SMALL_BLOCK_THRESHOLD;

        // In-place BWT: copy input into a mutable buffer, then transform
        // in-place (T == U). This avoids allocating a separate output buffer.
        let mut bwt_buf = mmap_zeroed(size)?;
        bwt_buf.copy_from_slice(block);
        let mut aux32 = [0i32; 256];
        let mut aux64 = [0i64; 256];

        if use_64bit {
            let sa_bytes = (size + 1) * std::mem::size_of::<i64>();
            let mut sa_mmap = mmap_zeroed(sa_bytes)?;
            let sais = sa_mmap.as_mut_ptr() as *mut i64;
            if small_block {
                let primary = unsafe {
                    libsais64_bwt_omp(
                        bwt_buf.as_ptr(),
                        bwt_buf.as_mut_ptr(),
                        sais,
                        size as i64,
                        0,
                        std::ptr::null_mut(),
                        self.cores as i64,
                    )
                };
                if primary < 0 {
                    return Err(io::Error::other(format!(
                        "libsais64_bwt failed with code {}",
                        primary
                    )));
                }
                aux64[0] = primary;
            } else {
                let r = compute_aux_stride(size) as i64;
                let ret = unsafe {
                    libsais64_bwt_aux_omp(
                        bwt_buf.as_ptr(),
                        bwt_buf.as_mut_ptr(),
                        sais,
                        size as i64,
                        0,
                        std::ptr::null_mut(),
                        r,
                        aux64.as_mut_ptr(),
                        self.cores as i64,
                    )
                };
                if ret < 0 {
                    return Err(io::Error::other(format!(
                        "libsais64_bwt_aux failed with code {}",
                        ret
                    )));
                }
            }
        } else {
            let sa_bytes = (size + 1) * std::mem::size_of::<i32>();
            let mut sa_mmap = mmap_zeroed(sa_bytes)?;
            let sais = sa_mmap.as_mut_ptr() as *mut i32;
            if small_block {
                let primary = unsafe {
                    libsais_bwt_omp(
                        bwt_buf.as_ptr(),
                        bwt_buf.as_mut_ptr(),
                        sais,
                        size as i32,
                        0,
                        std::ptr::null_mut(),
                        self.cores as i32,
                    )
                };
                if primary < 0 {
                    return Err(io::Error::other(format!(
                        "libsais_bwt failed with code {}",
                        primary
                    )));
                }
                aux32[0] = primary;
            } else {
                let r = compute_aux_stride(size) as i32;
                let ret = unsafe {
                    libsais_bwt_aux_omp(
                        bwt_buf.as_ptr(),
                        bwt_buf.as_mut_ptr(),
                        sais,
                        size as i32,
                        0,
                        std::ptr::null_mut(),
                        r,
                        aux32.as_mut_ptr(),
                        self.cores as i32,
                    )
                };
                if ret < 0 {
                    return Err(io::Error::other(format!(
                        "libsais_bwt_aux failed with code {}",
                        ret
                    )));
                }
            }
        }

        // Write block header.  Magic byte encodes the SA width so the
        // decompressor can parse the aux table without knowing block_mib.
        output.push(if use_64bit {
            BWT_MAGIC_64
        } else {
            BWT_MAGIC_32
        });
        output.extend_from_slice(&self.block_size_mib.to_le_bytes());
        if use_64bit {
            for val in aux64 {
                output.extend_from_slice(&val.to_le_bytes());
            }
        } else {
            for val in aux32 {
                output.extend_from_slice(&val.to_le_bytes());
            }
        }
        output.extend_from_slice(&(size as u64).to_le_bytes());

        // Split into chunks and encode
        let ranges: Vec<(usize, usize, usize)> = (0..size)
            .step_by(CHUNK_SIZE)
            .enumerate()
            .map(|(idx, start)| {
                let len = (size - start).min(CHUNK_SIZE);
                (idx, start, len)
            })
            .collect();

        let use_parallel = self.cores > 1 && !small_block;
        let mut encoded_chunks: Vec<(usize, u32, Vec<u8>)> = if use_parallel {
            ranges
                .par_iter()
                .map(|&(idx, start, len)| {
                    let mut data = Vec::new();
                    arithmetic_encode(&bwt_buf[start..start + len], &mut data);
                    (idx, len as u32, data)
                })
                .collect()
        } else {
            ranges
                .iter()
                .map(|&(idx, start, len)| {
                    let mut data = Vec::new();
                    arithmetic_encode(&bwt_buf[start..start + len], &mut data);
                    (idx, len as u32, data)
                })
                .collect()
        };

        encoded_chunks.sort_by_key(|(idx, _, _)| *idx);
        for (_, chunk_len, data) in encoded_chunks {
            output.extend_from_slice(&chunk_len.to_le_bytes());
            output.extend_from_slice(&(data.len() as u32).to_le_bytes());
            output.extend_from_slice(&data);
        }

        Ok(())
    }
}

impl BwtCompressor {
    /// Estimate peak memory for compressing `input_size` bytes with this
    /// BWT configuration.  Accounts for the suffix array and BWT buffer.
    pub fn estimate_compress_memory(block_mib: u16, input_size: usize) -> usize {
        let max_block = block_mib as usize * 1024 * 1024;
        let actual_size = max_block.min(input_size);
        // 64-bit dispatch is now per-block based on actual data size.
        let elem = if actual_size > i32::MAX as usize {
            8
        } else {
            4
        };
        // SA: (n+1) elements, BWT buffer (copy of input), arith output ~= input.
        (actual_size + 1) * elem + actual_size + actual_size
    }
}

impl Compressor for BwtCompressor {
    fn compress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        let mut offset = 0;
        while offset < input.len() {
            let end = (offset + self.block_bytes).min(input.len());
            self.compress_block(&input[offset..end], output)?;
            offset = end;
        }
        Ok(())
    }

    fn finish(&mut self, _output: &mut Vec<u8>) -> io::Result<()> {
        Ok(()) // BWT blocks are self-contained; nothing to flush.
    }

    fn reset(&mut self) {
        // BWT blocks are independent; nothing to reset.
    }

    fn compress_memory(&self, input_size: usize) -> usize {
        BwtCompressor::estimate_compress_memory(self.block_size_mib, input_size)
    }
}

/// BWT-based decompressor.
pub struct BwtDecompressor {
    block_size_mib: u16,
    block_bytes: usize,
    cores: usize,
}

impl BwtDecompressor {
    pub fn new(block_size_mib: u16, cores: usize) -> io::Result<Self> {
        if block_size_mib == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "block size must be at least 1 MiB",
            ));
        }
        let block_bytes = block_size_mib as usize * 1024 * 1024;
        let omp = openmp_state();
        Ok(Self {
            block_size_mib,
            block_bytes,
            cores: cores
                .max(1)
                .min(omp.max_threads.min(omp.thread_limit).max(1)),
        })
    }
}

impl BwtDecompressor {
    /// Estimate peak memory for decompressing one block.
    /// Uses the configured block_mib to bound the worst case; dispatch
    /// to 64-bit SA happens per-block based on actual data size.
    pub fn estimate_decompress_memory(block_mib: u16) -> usize {
        let block_bytes = block_mib as usize * 1024 * 1024;
        // Worst case: if block_bytes > i32::MAX, SA uses 8-byte elements.
        let elem = if block_bytes > i32::MAX as usize {
            8
        } else {
            4
        };
        // SA (n+1) + BWT buffer + output buffer.
        (block_bytes + 1) * elem + block_bytes + block_bytes
    }
}

impl Decompressor for BwtDecompressor {
    fn decompress(&mut self, input: &[u8], output: &mut Vec<u8>) -> io::Result<()> {
        let mut idx = 0;
        while idx < input.len() {
            // Minimum header: magic(1) + block_mib(2) + aux32(1024) + size(8)
            if idx + 1 + 2 + 256 * 4 + 8 > input.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated bwt block header",
                ));
            }

            let magic = input[idx];
            let use_64bit = match magic {
                BWT_MAGIC_32 => false,
                BWT_MAGIC_64 => true,
                // Legacy files always used 'B'; accept them and derive
                // width from the configured block_mib.
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid bwt block magic",
                    ));
                }
            };
            idx += 1;

            // For 64-bit aux, verify we have enough header bytes.
            if use_64bit && idx + 2 + 256 * 8 + 8 > input.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated bwt block header (64-bit aux)",
                ));
            }

            let block_size_mib = u16::from_le_bytes([input[idx], input[idx + 1]]);
            idx += 2;
            if block_size_mib != self.block_size_mib {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "block size mismatch in bwt stream",
                ));
            }

            let mut aux32 = [0i32; 256];
            let mut aux64 = [0i64; 256];
            if use_64bit {
                for slot in &mut aux64 {
                    *slot = i64::from_le_bytes(input[idx..idx + 8].try_into().unwrap());
                    idx += 8;
                }
            } else {
                for slot in &mut aux32 {
                    *slot = i32::from_le_bytes(input[idx..idx + 4].try_into().unwrap());
                    idx += 4;
                }
            }

            let size = u64::from_le_bytes(input[idx..idx + 8].try_into().unwrap()) as usize;
            idx += 8;

            if size == 0 {
                continue;
            }
            if size > self.block_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bwt block size exceeds configured limit",
                ));
            }

            let small_block = size < SMALL_BLOCK_THRESHOLD;

            // Read all chunks
            let mut decoded_total = 0usize;
            let mut tasks = Vec::new();
            while decoded_total < size {
                if idx + 8 > input.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "truncated bwt chunk header",
                    ));
                }
                let chunk_len =
                    u32::from_le_bytes(input[idx..idx + 4].try_into().unwrap()) as usize;
                idx += 4;
                let comp_len = u32::from_le_bytes(input[idx..idx + 4].try_into().unwrap()) as usize;
                idx += 4;

                let new_total = decoded_total.checked_add(chunk_len).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "bwt chunk length overflow")
                })?;
                if chunk_len == 0 || chunk_len > CHUNK_SIZE || new_total > size {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid bwt chunk length",
                    ));
                }
                if idx + comp_len > input.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "truncated bwt chunk data",
                    ));
                }

                tasks.push((decoded_total, chunk_len, idx, comp_len));
                idx += comp_len;
                decoded_total += chunk_len;
            }

            // Decode chunks into bwt_buf
            let mut bwt_buf = mmap_zeroed(size)?;
            let use_parallel = self.cores > 1 && !small_block;

            if use_parallel {
                let buf_ptr = bwt_buf.as_mut_ptr() as usize;
                tasks.par_iter().try_for_each(
                    |&(out_off, chunk_len, comp_start, comp_len)| -> io::Result<()> {
                        let comp_data = &input[comp_start..comp_start + comp_len];
                        let out_slice = unsafe {
                            std::slice::from_raw_parts_mut(
                                (buf_ptr as *mut u8).add(out_off),
                                chunk_len,
                            )
                        };
                        arithmetic_decode(comp_data, chunk_len, out_slice)
                    },
                )?;
            } else {
                for &(out_off, chunk_len, comp_start, comp_len) in &tasks {
                    let comp_data = &input[comp_start..comp_start + comp_len];
                    arithmetic_decode(
                        comp_data,
                        chunk_len,
                        &mut bwt_buf[out_off..out_off + chunk_len],
                    )?;
                }
            }

            // Inverse BWT using libsais
            let ret: i64 = if use_64bit {
                let mut sa_mmap = mmap_zeroed((size + 1) * std::mem::size_of::<i64>())?;
                let sais = sa_mmap.as_mut_ptr() as *mut i64;
                if small_block {
                    let primary = aux64[0];
                    if primary < 0 || primary as usize >= size {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "invalid primary index for small bwt block",
                        ));
                    }
                    unsafe {
                        libsais64_unbwt_omp(
                            bwt_buf.as_ptr(),
                            bwt_buf.as_mut_ptr(),
                            sais,
                            size as i64,
                            std::ptr::null(),
                            primary,
                            self.cores as i64,
                        )
                    }
                } else {
                    let r = compute_aux_stride(size) as i64;
                    unsafe {
                        libsais64_unbwt_aux_omp(
                            bwt_buf.as_ptr(),
                            bwt_buf.as_mut_ptr(),
                            sais,
                            size as i64,
                            std::ptr::null(),
                            r,
                            aux64.as_ptr(),
                            self.cores as i64,
                        )
                    }
                }
            } else {
                let mut sa_mmap = mmap_zeroed((size + 1) * std::mem::size_of::<i32>())?;
                let sais = sa_mmap.as_mut_ptr() as *mut i32;
                let r32 = if small_block {
                    let primary = aux32[0];
                    if primary < 0 || primary as usize >= size {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "invalid primary index for small bwt block",
                        ));
                    }
                    unsafe {
                        libsais_unbwt_omp(
                            bwt_buf.as_ptr(),
                            bwt_buf.as_mut_ptr(),
                            sais,
                            size as i32,
                            std::ptr::null(),
                            primary,
                            self.cores as i32,
                        )
                    }
                } else {
                    let r = compute_aux_stride(size) as i32;
                    unsafe {
                        libsais_unbwt_aux_omp(
                            bwt_buf.as_ptr(),
                            bwt_buf.as_mut_ptr(),
                            sais,
                            size as i32,
                            std::ptr::null(),
                            r,
                            aux32.as_ptr(),
                            self.cores as i32,
                        )
                    }
                };
                r32 as i64
            };

            if ret < 0 {
                return Err(io::Error::other(format!(
                    "libsais unbwt failed with code {}",
                    ret
                )));
            }

            output.extend_from_slice(&bwt_buf[..size]);
        }

        Ok(())
    }

    fn reset(&mut self) {
        // BWT blocks are independent; nothing to reset.
    }

    fn decompress_memory(&self, _output_size: usize) -> usize {
        BwtDecompressor::estimate_decompress_memory(self.block_size_mib)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_roundtrip_simple() {
        let input = b"AAAAACCCCCGGGGGTTTTTNNNNN";
        let mut encoded = Vec::new();
        arithmetic_encode(input, &mut encoded);

        let mut decoded = vec![0u8; input.len()];
        arithmetic_decode(&encoded, input.len(), &mut decoded).unwrap();
        assert_eq!(&decoded, input);
    }

    #[test]
    fn arithmetic_roundtrip_random_pattern() {
        let input: Vec<u8> = (0..1024).map(|i| ((i * 37 + i / 5) & 0xFF) as u8).collect();
        let mut encoded = Vec::new();
        arithmetic_encode(&input, &mut encoded);

        let mut decoded = vec![0u8; input.len()];
        arithmetic_decode(&encoded, input.len(), &mut decoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn arithmetic_roundtrip_all_zeros() {
        let input = vec![0u8; 512];
        let mut encoded = Vec::new();
        arithmetic_encode(&input, &mut encoded);

        let mut decoded = vec![0u8; input.len()];
        arithmetic_decode(&encoded, input.len(), &mut decoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn arithmetic_roundtrip_all_ff() {
        let input = vec![0xFFu8; 512];
        let mut encoded = Vec::new();
        arithmetic_encode(&input, &mut encoded);

        let mut decoded = vec![0u8; input.len()];
        arithmetic_decode(&encoded, input.len(), &mut decoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn arithmetic_roundtrip_single_byte() {
        let input = [42u8];
        let mut encoded = Vec::new();
        arithmetic_encode(&input, &mut encoded);

        let mut decoded = vec![0u8; 1];
        arithmetic_decode(&encoded, 1, &mut decoded).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn bwt_roundtrip_small_block() {
        // < 32 KiB triggers the no-aux path
        let size = 16 * 1024;
        let input: Vec<u8> = (0..size).map(|i| ((i * 13 + i / 5) & 0xFF) as u8).collect();

        let mut comp = BwtCompressor::new(1, 1).unwrap();
        let mut compressed = Vec::new();
        comp.compress(&input, &mut compressed).unwrap();

        let mut decomp = BwtDecompressor::new(1, 1).unwrap();
        let mut output = Vec::new();
        decomp.decompress(&compressed, &mut output).unwrap();

        assert_eq!(output, input);
    }

    #[test]
    fn bwt_roundtrip_medium_block() {
        // >= 32 KiB triggers aux-table path
        let size = 64 * 1024;
        let input: Vec<u8> = (0..size)
            .map(|i| ((i * 19 + i / 11) & 0xFF) as u8)
            .collect();

        let mut comp = BwtCompressor::new(1, 1).unwrap();
        let mut compressed = Vec::new();
        comp.compress(&input, &mut compressed).unwrap();

        let mut decomp = BwtDecompressor::new(1, 1).unwrap();
        let mut output = Vec::new();
        decomp.decompress(&compressed, &mut output).unwrap();

        assert_eq!(output, input);
    }

    #[test]
    fn bwt_roundtrip_genomic_data() {
        let bases = b"ACGT";
        let size = 100_000;
        let input: Vec<u8> = (0..size).map(|i| bases[i % 4]).collect();

        let mut comp = BwtCompressor::new(1, 1).unwrap();
        let mut compressed = Vec::new();
        comp.compress(&input, &mut compressed).unwrap();

        let mut decomp = BwtDecompressor::new(1, 1).unwrap();
        let mut output = Vec::new();
        decomp.decompress(&compressed, &mut output).unwrap();

        assert_eq!(output, input);
        assert!(compressed.len() < input.len());
    }

    #[test]
    fn bwt_roundtrip_64bit_config() {
        let size = SMALL_BLOCK_THRESHOLD + 1024;
        let input: Vec<u8> = (0..size)
            .map(|i| ((i * 19 + i / 11) & 0xFF) as u8)
            .collect();

        let mut comp = BwtCompressor::new(2049, 1).unwrap();
        let mut compressed = Vec::new();
        comp.compress(&input, &mut compressed).unwrap();

        let mut decomp = BwtDecompressor::new(2049, 1).unwrap();
        let mut output = Vec::new();
        decomp.decompress(&compressed, &mut output).unwrap();

        assert_eq!(output, input);
    }

    #[test]
    fn bwt_roundtrip_parallel() {
        let size = 64 * 1024;
        let input: Vec<u8> = (0..size).map(|i| ((i * 7 + i / 3) & 0xFF) as u8).collect();

        let mut comp = BwtCompressor::new(1, 4).unwrap();
        let mut compressed = Vec::new();
        comp.compress(&input, &mut compressed).unwrap();

        let mut decomp = BwtDecompressor::new(1, 4).unwrap();
        let mut output = Vec::new();
        decomp.decompress(&compressed, &mut output).unwrap();

        assert_eq!(output, input);
    }

    #[test]
    fn bwt_zero_block_size_rejected() {
        assert!(BwtCompressor::new(0, 1).is_err());
        assert!(BwtDecompressor::new(0, 1).is_err());
    }

    #[test]
    fn compute_aux_stride_values() {
        assert_eq!(compute_aux_stride(32 * 1024), 4096);
        assert_eq!(compute_aux_stride(1024 * 1024), 131072);
        assert_eq!(compute_aux_stride(64 * 1024), 8192);
    }

    /// Verify BWT blocks are independently decompressible after reset
    /// (indexed mode pattern).
    #[test]
    fn bwt_indexed_blocks_independent() {
        let mut comp = BwtCompressor::new(1, 1).unwrap();

        let blocks: Vec<Vec<u8>> = (0..5)
            .map(|i| {
                (0..64 * 1024)
                    .map(|j| ((j * 13 + i * 997) & 0xFF) as u8)
                    .collect()
            })
            .collect();

        let mut compressed_blocks: Vec<Vec<u8>> = Vec::new();
        for block in &blocks {
            let mut compressed = Vec::new();
            comp.compress(block, &mut compressed).unwrap();
            comp.reset();
            compressed_blocks.push(compressed);
        }

        for (i, (compressed, original)) in compressed_blocks.iter().zip(blocks.iter()).enumerate() {
            let mut fresh_decomp = BwtDecompressor::new(1, 1).unwrap();
            let mut result = Vec::new();
            fresh_decomp
                .decompress(compressed, &mut result)
                .unwrap_or_else(|e| panic!("block {} decompress failed: {}", i, e));
            assert_eq!(result, *original, "block {} content mismatch", i);
        }
    }
}
