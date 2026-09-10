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

pub mod cat;
pub mod decode;
pub mod encode;
pub mod list;
pub mod nuc_pattern;
pub mod query;
pub mod recode;
pub mod ref_decode;
pub mod ref_encode;
pub mod slice;
pub mod stats;
pub mod verify;

use crate::Cli;
use hecate::compress::bwt_codec::{BwtCompressor, BwtDecompressor};
use hecate::compress::rans_codec::{RansCompressor, RansDecompressor};
use hecate::compress::traits::Compressor;
use hecate::compress::markov_codec::{MarkovCompressor, MarkovDecompressor};
use hecate::compress::xz_codec::{XzCompressor, XzDecompressor, XzParams};
use hecate::compress::zstd_codec::{ZstdCompressor, ZstdDecompressor};
use hecate::container::*;
use hecate::index::IndexTier;
use hecate::pipeline::handler::Handler;
use hecate::pipeline::sink::{FastaTextSink, FastqTextSink};
use hecate::processor::case_invert::CaseInverter;
use hecate::processor::quality_strip::QualityStripper;
use hecate::stream::buffered::{BufferedReader, BufferedWriter};
use hecate::stream::file::{FileInput, FileOutput};
use hecate::stream::mmap::MmapInput;
use hecate::stream::pipe::PipeOutput;
use hecate::stream::stdio::{StdinStream, StdoutStream};
use hecate::stream::traits::{StreamRead, StreamSeek, StreamWrite};
use std::io;
use std::path::{Path, PathBuf};

/// Minimal cursor over `&[u8]` implementing `StreamRead` for parsing headers
/// from mmap slices.
pub(crate) struct SliceCursor<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> SliceCursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
}

impl StreamRead for SliceCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let avail = self.data.len() - self.pos;
        let n = avail.min(buf.len());
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

pub(crate) fn cmp_header_size(methods: &[MethodSpec], stream_count: usize) -> u64 {
    let mut sz = 6u64; // CMP(3) + version(1) + stream_count(1) + method_count(1)
    for m in methods {
        sz += match m {
            MethodSpec::Zstd { .. } => 6,    // tag(1) + level(1) + window_log(4)
            MethodSpec::Xz { .. } => 8,      // tag(1) + preset(1) + dict(4) + lc(1) + pb(1)
            MethodSpec::Bwt { .. } => 3,     // tag(1) + block_mib(2)
            MethodSpec::Markov { .. } => 2, // tag(1) + four_bit(1)
            MethodSpec::Rans => 1,          // tag(1)
        };
    }
    sz += stream_count as u64; // pattern
    sz
}

pub(crate) fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

pub(crate) fn resolve_alphabet(cli: &Cli) -> AlphabetMode {
    if cli.two_bit {
        AlphabetMode::TwoBit
    } else if cli.eight_bit {
        AlphabetMode::EightBit
    } else {
        AlphabetMode::FourBit
    }
}

pub(crate) fn resolve_case_mode(cli: &Cli) -> CaseMode {
    if cli.joint_case {
        CaseMode::Joint
    } else if cli.ignore_case {
        CaseMode::Ignore
    } else {
        CaseMode::Include
    }
}

pub(crate) fn resolve_index_tier(cli: &Cli) -> io::Result<Option<(IndexTier, Option<usize>)>> {
    match &cli.index {
        Some(s) => {
            let (tier_str, budget) = if let Some(pos) = s.find(',') {
                let budget_mib: usize = s[pos + 1..].parse().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("bad budget in -I: {}", e),
                    )
                })?;
                (&s[..pos], Some(budget_mib * 1024 * 1024))
            } else {
                (s.as_str(), None)
            };
            let tier = match tier_str {
                "block" => IndexTier::BlockOnly,
                "full" => IndexTier::Full,
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "unknown index tier {:?}, expected \"full\" or \"block\"",
                            other
                        ),
                    ));
                }
            };
            Ok(Some((tier, budget)))
        }
        None => Ok(None),
    }
}

pub(crate) fn detect_file_type(cli: &Cli, peek: &[u8]) -> io::Result<FileType> {
    if cli.fasta {
        return Ok(FileType::Fasta);
    }
    if cli.fastq {
        return Ok(FileType::Fastq);
    }
    if let Some(ref path) = cli.file {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "fa" | "fasta" | "fna" => return Ok(FileType::Fasta),
                "fq" | "fastq" => return Ok(FileType::Fastq),
                _ => {}
            }
        }
    }
    if peek.is_empty() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty input"));
    }
    match peek[0] {
        b'>' => Ok(FileType::Fasta),
        b'@' => Ok(FileType::Fastq),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("cannot detect file type from first byte 0x{:02X}", peek[0]),
        )),
    }
}

fn parse_xz_preset(s: &str) -> io::Result<(u8, bool)> {
    let s = s.trim();
    let (digits, extreme) = if let Some(p) = s.strip_suffix('e').or_else(|| s.strip_suffix('E')) {
        (p, true)
    } else {
        (s, false)
    };
    let level: u8 = digits.parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bad xz preset {:?}: {}", s, e),
        )
    })?;
    if level > 9 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("xz preset must be 0-9 (got {})", level),
        ));
    }
    Ok((level, extreme))
}

fn parse_size_bytes_u32(s: &str) -> io::Result<u32> {
    let s = s.trim();
    let suffixes: &[(&str, u64)] = &[
        ("GiB", 1u64 << 30),
        ("MiB", 1u64 << 20),
        ("KiB", 1u64 << 10),
        ("G", 1u64 << 30),
        ("M", 1u64 << 20),
        ("K", 1u64 << 10),
        ("B", 1),
    ];
    let (num_str, mul) = suffixes
        .iter()
        .find_map(|(suf, mul)| s.strip_suffix(suf).map(|n| (n, *mul)))
        .unwrap_or((s, 1));
    let n: u64 = num_str.trim().parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("bad size {:?}: {}", s, e),
        )
    })?;
    let bytes = n.checked_mul(mul).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("size overflow: {}", s))
    })?;
    if bytes > u32::MAX as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("xz dict size exceeds 4 GiB: {}", s),
        ));
    }
    Ok(bytes as u32)
}

fn xz_param_key(token: &str) -> Option<&str> {
    let key = token.split('=').next()?.trim();
    match key {
        "preset" | "dict" | "lc" | "pb" => Some(key),
        _ => None,
    }
}

pub(crate) fn parse_methods(s: &str) -> io::Result<Vec<MethodSpec>> {
    let parts: Vec<&str> = s.split(',').collect();
    let mut methods = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let part = parts[i].trim();
        i += 1;
        if part.is_empty() {
            continue;
        }
        let fields: Vec<&str> = part.split(':').collect();
        match fields[0] {
            "zstd" => {
                let level: i8 = fields.get(1).unwrap_or(&"3").parse().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("bad zstd level: {}", e),
                    )
                })?;
                let window_log: u32 = if fields.len() > 2 {
                    fields[2].parse().map_err(|e| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("bad window_log: {}", e),
                        )
                    })?
                } else {
                    0
                };
                methods.push(MethodSpec::Zstd { level, window_log });
            }
            "xz" => {
                let mut preset: u8 = 6;
                let mut extreme: bool = false;
                let mut dict_size: u32 = 0;
                let mut lc: u8 = 0xFF;
                let mut pb: u8 = 0xFF;
                let mut kvs: Vec<&str> = Vec::new();

                if fields.len() > 1 {
                    let f = fields[1].trim();
                    if !f.is_empty() {
                        if f.contains('=') {
                            kvs.push(f);
                        } else {
                            let (lvl, ext) = parse_xz_preset(f)?;
                            preset = lvl;
                            extreme = ext;
                        }
                    }
                }
                while i < parts.len() {
                    let next = parts[i].trim();
                    if next.contains('=') && !next.contains(':') && xz_param_key(next).is_some() {
                        kvs.push(next);
                        i += 1;
                    } else {
                        break;
                    }
                }
                for kv in kvs {
                    let (k, v) = kv.split_once('=').ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("bad xz param {:?}", kv),
                        )
                    })?;
                    let v = v.trim();
                    match k.trim() {
                        "preset" => {
                            let (l, e) = parse_xz_preset(v)?;
                            preset = l;
                            extreme = e;
                        }
                        "dict" => {
                            dict_size = parse_size_bytes_u32(v)?;
                        }
                        "lc" => {
                            let n: u8 = v.parse().map_err(|e| {
                                io::Error::new(
                                    io::ErrorKind::InvalidInput,
                                    format!("bad lc: {}", e),
                                )
                            })?;
                            if n > 4 {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidInput,
                                    format!("xz lc must be 0-4 (got {})", n),
                                ));
                            }
                            lc = n;
                        }
                        "pb" => {
                            let n: u8 = v.parse().map_err(|e| {
                                io::Error::new(
                                    io::ErrorKind::InvalidInput,
                                    format!("bad pb: {}", e),
                                )
                            })?;
                            if n > 4 {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidInput,
                                    format!("xz pb must be 0-4 (got {})", n),
                                ));
                            }
                            pb = n;
                        }
                        other => {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                format!("unknown xz param: {}", other),
                            ));
                        }
                    }
                }
                methods.push(MethodSpec::Xz {
                    preset,
                    extreme,
                    dict_size,
                    lc,
                    pb,
                });
            }
            "bwt" => {
                let block_mib: u16 = fields.get(1).unwrap_or(&"32").parse().map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidInput, format!("bad bwt block: {}", e))
                })?;
                methods.push(MethodSpec::Bwt { block_mib });
            }
            "markov" => {
                methods.push(MethodSpec::Markov { four_bit: false });
            }
            "rans" => {
                methods.push(MethodSpec::Rans);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown compression method: {}", fields[0]),
                ));
            }
        }
    }
    Ok(methods)
}

pub(crate) fn parse_pattern(s: &str, stream_count: usize) -> io::Result<Vec<u8>> {
    if s.len() != stream_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "pattern has {} characters, expected {}",
                s.len(),
                stream_count
            ),
        ));
    }
    s.chars()
        .map(|c| {
            if c == '_' {
                Ok(0xFF)
            } else if let Some(d) = c.to_digit(10) {
                Ok(d as u8)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("bad pattern character '{}': expected digit or '_'", c),
                ))
            }
        })
        .collect()
}

pub(crate) fn compute_block_budget(methods: &[MethodSpec]) -> usize {
    use hecate::mux::writer::DEFAULT_BLOCK_BUDGET;
    let mut budget: usize = DEFAULT_BLOCK_BUDGET;
    for m in methods {
        match m {
            MethodSpec::Bwt { block_mib } => {
                let bytes = *block_mib as usize * 1024 * 1024;
                budget = budget.max(bytes);
            }
            MethodSpec::Zstd { .. } => {
                budget = budget.max(8 * 1024 * 1024);
            }
            MethodSpec::Xz { .. } => {
                // xz benefits from large blocks (dict up to 1 GiB at preset 9).
                budget = budget.max(8 * 1024 * 1024);
            }
            MethodSpec::Markov { .. } => {
                budget = budget.max(7 * 1024 * 1024);
            }
            MethodSpec::Rans => {
                budget = budget.max(2 * 1024 * 1024);
            }
        }
    }
    budget
}

pub(crate) fn build_compressors(
    methods: &[MethodSpec],
    pattern: &[u8],
    block_budget: usize,
    indexed: bool,
) -> io::Result<Vec<Option<Box<dyn Compressor>>>> {
    let cores = num_cpus::get();

    // In non-indexed mode, zstd and xz compressors use a worker pool for
    // internal pipelining.  Each worker has its own private state (window /
    // dict + hash tables / lzma state), so memory scales with thread count.
    // Compute a safe shared thread count that keeps total compressor memory
    // within available RAM (50% safety margin for I/O buffers, pending data,
    // and process overhead).
    let zstd_threads = if indexed {
        1u32
    } else {
        let avail = hecate::util::available_memory_bytes();
        let budget = avail / 2;

        // Count how many threaded streams there are.
        let mt_stream_count = pattern
            .iter()
            .filter(|&&idx| {
                idx != 0xFF
                    && matches!(
                        methods.get(idx as usize),
                        Some(MethodSpec::Zstd { .. }) | Some(MethodSpec::Xz { .. })
                    )
            })
            .count();

        if mt_stream_count == 0 {
            cores as u32
        } else {
            // Sum the non-threaded fixed compressor memory.
            let fixed_mem: usize = pattern
                .iter()
                .filter_map(|&idx| {
                    if idx == 0xFF {
                        return None;
                    }
                    match methods.get(idx as usize)? {
                        MethodSpec::Bwt { block_mib } => Some(
                            BwtCompressor::estimate_compress_memory(*block_mib, block_budget),
                        ),
                        MethodSpec::Markov { .. } => {
                            Some(MarkovCompressor::estimate_memory(block_budget))
                        }
                        MethodSpec::Zstd { .. } | MethodSpec::Xz { .. } | MethodSpec::Rans => None,
                    }
                })
                .sum();

            // Find a thread count where total zstd+xz memory fits in budget.
            // Each threaded instance gets the same worker count.
            let remaining = budget.saturating_sub(fixed_mem);
            let mut threads = cores as u32;
            while threads > 1 {
                let total_mt: usize = pattern
                    .iter()
                    .filter_map(|&idx| {
                        if idx == 0xFF {
                            return None;
                        }
                        match methods.get(idx as usize)? {
                            MethodSpec::Zstd { level, window_log } => {
                                Some(ZstdCompressor::estimate_memory(
                                    *level as i32,
                                    *window_log,
                                    threads,
                                    block_budget,
                                ))
                            }
                            MethodSpec::Xz {
                                preset,
                                extreme,
                                dict_size,
                                lc,
                                pb,
                            } => {
                                let params = XzParams {
                                    preset: *preset,
                                    extreme: *extreme,
                                    dict_size: *dict_size,
                                    lc: *lc,
                                    pb: *pb,
                                };
                                Some(XzCompressor::estimate_memory(
                                    &params,
                                    threads,
                                    block_budget,
                                ))
                            }
                            _ => None,
                        }
                    })
                    .sum();
                if total_mt <= remaining {
                    break;
                }
                threads /= 2;
            }
            threads.max(1)
        }
    };

    pattern
        .iter()
        .enumerate()
        .map(|(stream, &idx)| {
            if idx == 0xFF {
                Ok(None)
            } else {
                let c: Box<dyn Compressor> = match &methods[idx as usize] {
                    MethodSpec::Zstd { level, window_log } => Box::new(ZstdCompressor::new(
                        *level as i32,
                        *window_log,
                        zstd_threads,
                    )),
                    MethodSpec::Xz {
                        preset,
                        extreme,
                        dict_size,
                        lc,
                        pb,
                    } => {
                        let params = XzParams {
                            preset: *preset,
                            extreme: *extreme,
                            dict_size: *dict_size,
                            lc: *lc,
                            pb: *pb,
                        };
                        // Use the same memory-bounded thread count as zstd: xz MT
                        // workers each hold a private dict + state.
                        Box::new(XzCompressor::new(params, zstd_threads))
                    }
                    MethodSpec::Bwt { block_mib } => {
                        Box::new(BwtCompressor::new(*block_mib, cores)?)
                    }
                    MethodSpec::Markov { four_bit } => {
                        if stream != 1 {
                            return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "markov compression can only be applied to the NUC stream (stream 1)",
                        ));
                        }
                        Box::new(MarkovCompressor::new(*four_bit))
                    }
                    MethodSpec::Rans => Box::new(RansCompressor::new()),
                };
                Ok(Some(c))
            }
        })
        .collect()
}

pub(crate) fn build_method_specs(
    methods: &[MethodSpec],
    pattern: &[u8],
) -> Vec<Option<MethodSpec>> {
    pattern
        .iter()
        .map(|&idx| {
            if idx == 0xFF {
                None
            } else {
                Some(methods[idx as usize].clone())
            }
        })
        .collect()
}

pub(crate) fn build_decompressors(
    methods: &[MethodSpec],
    pattern: &[u8],
) -> io::Result<Vec<Option<Box<dyn hecate::compress::traits::Decompressor>>>> {
    let cores = num_cpus::get();
    pattern.iter().enumerate().map(|(stream, &idx)| {
        if idx == 0xFF {
            Ok(None)
        } else {
            let d: Box<dyn hecate::compress::traits::Decompressor> =
                match &methods[idx as usize] {
                    MethodSpec::Zstd { .. } => Box::new(ZstdDecompressor::new()),
                    MethodSpec::Xz { .. } => Box::new(XzDecompressor::new()),
                    MethodSpec::Bwt { block_mib } => {
                        Box::new(BwtDecompressor::new(*block_mib, cores)?)
                    }
                    MethodSpec::Markov { four_bit } => {
                        if stream != 1 {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "markov compression can only be applied to the NUC stream (stream 1)",
                            ));
                        }
                        Box::new(MarkovDecompressor::new(*four_bit))
                    }
                    MethodSpec::Rans => Box::new(RansDecompressor::new()),
                };
            Ok(Some(d))
        }
    }).collect()
}

pub(crate) fn make_decompressor(
    methods: &[MethodSpec],
    pattern: &[u8],
    stream_id: usize,
) -> io::Result<Box<dyn hecate::compress::traits::Decompressor>> {
    let idx = pattern[stream_id];
    match &methods[idx as usize] {
        MethodSpec::Zstd { .. } => Ok(Box::new(ZstdDecompressor::new())),
        MethodSpec::Xz { .. } => Ok(Box::new(XzDecompressor::new())),
        MethodSpec::Bwt { block_mib } => Ok(Box::new(BwtDecompressor::new(*block_mib, 1)?)),
        MethodSpec::Markov { four_bit } => Ok(Box::new(MarkovDecompressor::new(*four_bit))),
        MethodSpec::Rans => Ok(Box::new(RansDecompressor::new())),
    }
}

pub(crate) fn resolve_output_encode(cli: &Cli) -> io::Result<Option<PathBuf>> {
    if let Some(ref p) = cli.output {
        return Ok(Some(p.clone()));
    }
    if cli.to_stdout {
        return Ok(None);
    }
    if let Some(ref p) = cli.file {
        let stem = p.file_stem().unwrap_or_default();
        let parent = p.parent().unwrap_or_else(|| Path::new("."));
        return Ok(Some(parent.join(format!("{}.he", stem.to_string_lossy()))));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "no output - use -o or -c",
    ))
}

pub(crate) fn resolve_output_decode(cli: &Cli, file_type: FileType) -> io::Result<Option<PathBuf>> {
    if let Some(ref p) = cli.output {
        return Ok(Some(p.clone()));
    }
    if cli.to_stdout {
        return Ok(None);
    }
    if let Some(ref p) = cli.file {
        let stem = p.file_stem().unwrap_or_default();
        let parent = p.parent().unwrap_or_else(|| Path::new("."));
        let ext = match file_type {
            FileType::Fasta => "fa",
            FileType::Fastq => "fq",
        };
        return Ok(Some(parent.join(format!(
            "{}.{}",
            stem.to_string_lossy(),
            ext
        ))));
    }
    Ok(None) // stdout
}

pub(crate) fn open_input_path(
    path: &Path,
    no_mmap: bool,
) -> io::Result<BufferedReader<Box<dyn StreamRead>>> {
    if !no_mmap {
        if let Ok(mmap) = MmapInput::open(path) {
            mmap.advise_sequential();
            // Tiny BufferedReader capacity: mmap.read already copies from
            // the page-cache-backed mapping, so we don't want to pay a
            // second memcpy through a large internal buffer. The 4 KiB
            // floor still satisfies BufferedReader's invariants for the
            // few small per-block header reads.
            return Ok(BufferedReader::with_capacity(
                4096,
                Box::new(mmap) as Box<dyn StreamRead>,
            ));
        }
    }
    let fi = FileInput::open(path)?;
    Ok(BufferedReader::new(Box::new(fi) as Box<dyn StreamRead>))
}

pub(crate) fn open_input(cli: &Cli) -> io::Result<BufferedReader<Box<dyn StreamRead>>> {
    if let Some(ref path) = cli.file {
        if !cli.no_mmap {
            if let Ok(mmap) = MmapInput::open(path) {
                mmap.advise_sequential();
                return Ok(BufferedReader::with_capacity(
                    4096,
                    Box::new(mmap) as Box<dyn StreamRead>,
                ));
            }
        }
        let fi = FileInput::open(path)?;
        Ok(BufferedReader::new(Box::new(fi) as Box<dyn StreamRead>))
    } else {
        let si = StdinStream::new();
        Ok(BufferedReader::new(Box::new(si) as Box<dyn StreamRead>))
    }
}

pub(crate) fn open_filter_output(
    cli: &Cli,
    output_path: Option<&PathBuf>,
) -> io::Result<Box<dyn StreamWrite>> {
    let filter_cmd = match &cli.filter {
        Some(cmd) => cmd,
        None => return open_output(output_path, cli.force),
    };
    match output_path {
        Some(p) => {
            if p.exists() && !cli.force {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{}: exists (use --force)", p.display()),
                ));
            }
            let file = std::fs::File::create(p)?;
            Ok(Box::new(PipeOutput::filter_to_file(filter_cmd, file)?))
        }
        None => Ok(Box::new(PipeOutput::filter_to_stdout(filter_cmd)?)),
    }
}

pub(crate) fn open_output(path: Option<&PathBuf>, force: bool) -> io::Result<Box<dyn StreamWrite>> {
    match path {
        Some(p) => {
            if p.exists() && !force {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{}: exists (use --force)", p.display()),
                ));
            }
            Ok(Box::new(BufferedWriter::with_capacity(
                1 << 20,
                FileOutput::create(p)?,
            )))
        }
        None => Ok(Box::new(BufferedWriter::with_capacity(
            1 << 20,
            StdoutStream::new(),
        ))),
    }
}

pub(crate) fn wrap_processors<'a>(
    mut handler: Box<dyn Handler + 'a>,
    cli: &Cli,
    alphabet: AlphabetMode,
    case_mode: CaseMode,
) -> Box<dyn Handler + 'a> {
    if cli.invert_case {
        handler = Box::new(CaseInverter::new(handler, alphabet, case_mode));
    }
    if cli.strip_quality {
        handler = Box::new(QualityStripper::new(handler));
    }
    handler
}

pub(crate) fn build_text_handler<'a>(
    cli: &Cli,
    output: &'a mut dyn StreamWrite,
    file_header: &FileHeader,
) -> Box<dyn Handler + 'a> {
    let output_type = if cli.strip_quality {
        FileType::Fasta
    } else {
        file_header.file_type
    };
    let handler: Box<dyn Handler + 'a> = match output_type {
        FileType::Fasta => Box::new(FastaTextSink::new(
            output,
            file_header.alphabet,
            file_header.case_mode,
            cli.line_width,
        )),
        FileType::Fastq => Box::new(FastqTextSink::new(
            output,
            file_header.alphabet,
            file_header.case_mode,
        )),
    };
    wrap_processors(handler, cli, file_header.alphabet, file_header.case_mode)
}

pub(crate) fn skip_bytes(input: &mut dyn StreamRead, mut remaining: u64) -> io::Result<()> {
    let mut discard = [0u8; 8192];
    while remaining > 0 {
        let n = remaining.min(discard.len() as u64) as usize;
        input.read_exact(&mut discard[..n])?;
        remaining -= n as u64;
    }
    Ok(())
}

pub(crate) fn read_index_stats(path: &Path) -> io::Result<(String, String)> {
    let mut f = FileInput::open(path)?;
    f.seek(io::SeekFrom::End(-(Trailer::SIZE as i64)))?;
    let trailer = Trailer::read_from(&mut f)?;
    if trailer.index_offset == 0 {
        return Ok(("-".into(), "-".into()));
    }
    f.seek(io::SeekFrom::Start(trailer.index_offset))?;
    let index = hecate::index::reader::read_index(&mut f)?;
    let seq_count = format!("{}", index.sequence_count());
    let total_bases = if index.sequence_entries.is_empty() {
        "-".into()
    } else {
        format_number(index.sequence_entries.iter().map(|e| e.total_bases).sum())
    };
    Ok((seq_count, total_bases))
}

pub(crate) fn print_stats(
    cli: &Cli,
    file_header: &FileHeader,
    cmp_header: &CompressionHeader,
    block_count: u64,
    total_compressed: &[u64],
    total_uncompressed: &[u64],
    seq_count_str: &str,
    total_bases_str: &str,
) {
    let stream_count = total_compressed.len();
    let file_name = cli
        .file
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<stdin>".to_string());
    let type_str = match file_header.file_type {
        FileType::Fasta => "FASTA",
        FileType::Fastq => "FASTQ",
    };
    let alphabet_str = match file_header.alphabet {
        AlphabetMode::TwoBit => "2-bit",
        AlphabetMode::FourBit => "4-bit",
        AlphabetMode::EightBit => "8-bit",
    };
    let case_str = match file_header.case_mode {
        CaseMode::Include => "include",
        CaseMode::Ignore => "ignore",
        CaseMode::Joint => "joint",
    };
    let indexed_str = if file_header.indexed { "yes" } else { "no" };
    let compress_str = if cmp_header.methods.is_empty() {
        "none".to_string()
    } else {
        cmp_header
            .methods
            .iter()
            .map(|m| match m {
                MethodSpec::Zstd { level, .. } => format!("zstd:{}", level),
                MethodSpec::Xz {
                    preset, extreme, ..
                } => {
                    format!("xz:{}{}", preset, if *extreme { "e" } else { "" })
                }
                MethodSpec::Bwt { block_mib } => format!("bwt:{}", block_mib),
                MethodSpec::Markov { .. } => "markov".to_string(),
                MethodSpec::Rans => "rans".to_string(),
            })
            .collect::<Vec<_>>()
            .join(";")
    };

    println!("File: {}", file_name);
    println!(
        "Type: {}  Alphabet: {}  Case: {}",
        type_str, alphabet_str, case_str
    );
    println!("Indexed: {}  Compression: {}", indexed_str, compress_str);
    println!(
        "Blocks: {}  Sequences: {}  Total bases: {}",
        block_count, seq_count_str, total_bases_str
    );
    println!();

    let stream_names: &[&str] = match stream_count {
        7 => &[
            "CTRL",
            "NUC",
            "CASE",
            "HDR",
            "EXTRA",
            "QUALITY",
            "HDR_COORDS",
        ],
        6 => &["CTRL", "NUC", "CASE", "HDR", "EXTRA", "QUALITY"],
        _ => &["CTRL", "NUC", "CASE", "HDR", "EXTRA"],
    };

    println!(
        "{:<12} {:>14} {:>14} {:>8}",
        "Stream", "Compressed", "Uncompressed", "Ratio"
    );
    let mut grand_comp = 0u64;
    let mut grand_uncomp = 0u64;
    for s in 0..stream_count {
        let ratio_str = if total_uncompressed[s] == 0 {
            "-".to_string()
        } else {
            format!(
                "{:.2}x",
                total_uncompressed[s] as f64 / total_compressed[s].max(1) as f64
            )
        };
        println!(
            "{:<12} {:>14} {:>14} {:>8}",
            stream_names[s],
            format_number(total_compressed[s]),
            format_number(total_uncompressed[s]),
            ratio_str
        );
        grand_comp += total_compressed[s];
        grand_uncomp += total_uncompressed[s];
    }
    let grand_ratio = if grand_uncomp == 0 {
        "-".to_string()
    } else {
        format!("{:.2}x", grand_uncomp as f64 / grand_comp.max(1) as f64)
    };
    println!(
        "{:<12} {:>14} {:>14} {:>8}",
        "Total",
        format_number(grand_comp),
        format_number(grand_uncomp),
        grand_ratio
    );
}
