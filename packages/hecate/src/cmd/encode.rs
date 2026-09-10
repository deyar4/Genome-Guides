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

use super::{
    build_compressors, build_method_specs, cmp_header_size, compute_block_budget, detect_file_type,
    open_input, open_output, parse_methods, parse_pattern, resolve_alphabet, resolve_case_mode,
    resolve_index_tier, resolve_output_encode,
};
use crate::Cli;
use hecate::codec;
use hecate::compress::markov_codec::{MarkovCompressor, ProbSink};
use hecate::compress::traits::Compressor;
use hecate::compress::xz_codec::{XzCompressor, XzParams};
use hecate::container::*;
use hecate::mux::writer::MuxWriter;
use hecate::stream::buffered::BufferedReader;
use hecate::stream::pipe::PipeInput;
use hecate::stream::traits::{BorrowRead, StreamRead, StreamWrite};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub fn cmd_encode(cli: &Cli) -> io::Result<()> {
    let start = Instant::now();

    let mut input = if let Some(ref filter_cmd) = cli.filter {
        let path = cli.file.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "--filter requires a file input",
            )
        })?;
        let file = std::fs::File::open(path)?;
        let pipe = PipeInput::filter_file(filter_cmd, file)?;
        BufferedReader::new(Box::new(pipe) as Box<dyn StreamRead>)
    } else {
        open_input(cli)?
    };
    let peek = input.fill_buf(3)?.to_vec();
    let file_type = detect_file_type(cli, &peek)?;

    let alphabet = resolve_alphabet(cli);
    let case_mode = resolve_case_mode(cli);
    if case_mode == CaseMode::Joint && alphabet != AlphabetMode::EightBit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "joint case requires 8-bit alphabet",
        ));
    }

    let illumina = cli.illumina;
    if illumina && file_type != FileType::Fastq {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "illumina mode (-L) requires FASTQ input",
        ));
    }

    let stream_count = if illumina {
        ILLUMINA_FASTQ_STREAM_COUNT as usize
    } else {
        file_type.stream_count() as usize
    };

    // Build quality binning LUT if requested
    let quality_lut = if let Some(ref spec_str) = cli.quality_bin {
        let spec = codec::quality::parse_quality_bin_spec(spec_str)?;
        Some(codec::quality::build_quality_lut(&spec))
    } else {
        None
    };

    let mut methods = if let Some(ref ms) = cli.methods {
        parse_methods(ms)?
    } else if illumina {
        // Auto-add rANS for illumina mode when no explicit -M
        vec![MethodSpec::Rans]
    } else {
        vec![]
    };
    // Patch Markov/Bmarkov methods with the correct four_bit flag based on alphabet.
    let four_bit = alphabet == AlphabetMode::FourBit;
    for m in &mut methods {
        match m {
            MethodSpec::Markov { four_bit: fb } => {
                if alphabet == AlphabetMode::EightBit {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "markov compression is not supported with 8-bit alphabet",
                    ));
                }
                *fb = four_bit;
            }
            _ => {}
        }
    }

    let markov_idx = methods.iter().position(|m| {
        matches!(
            m,
            MethodSpec::Markov { .. }
        )
    });
    let pattern = if let Some(ref ps) = cli.pattern {
        parse_pattern(ps, stream_count)?
    } else if methods.is_empty() {
        vec![0xFF; stream_count]
    } else if illumina && cli.methods.is_none() {
        // Auto rANS pattern for illumina: rANS on QUALITY (5) and HDR_COORDS (6), raw for rest
        let rans_idx = methods
            .iter()
            .position(|m| matches!(m, MethodSpec::Rans))
            .unwrap_or(0);
        let mut pat = vec![0xFF; stream_count];
        pat[STREAM_QUALITY] = rans_idx as u8;
        pat[STREAM_HDR_COORDS] = rans_idx as u8;
        pat
    } else if let Some(mi) = markov_idx {
        // Markov/Bmarkov is nucleotide-specific; assign it to NUC (stream 1).
        // If another method exists, use the first non-markov for everything else.
        let other = methods.iter().position(|m| {
            !matches!(
                m,
                MethodSpec::Markov { .. }
            )
        });
        let fill = other.map(|i| i as u8).unwrap_or(0xFF);
        let mut pat = vec![fill; stream_count];
        if stream_count > 1 {
            pat[1] = mi as u8;
        }
        pat
    } else {
        vec![0; stream_count]
    };

    let (index_tier, budget_override) = match resolve_index_tier(cli)? {
        Some((tier, budget)) => (Some(tier), budget),
        None => (None, None),
    };
    let indexed = index_tier.is_some();
    let compressed = !methods.is_empty();

    let output_path = resolve_output_encode(cli)?;
    let mut output = open_output(output_path.as_ref(), cli.force)?;

    let file_header = FileHeader {
        alphabet,
        file_type,
        case_mode,
        indexed,
        compressed,
        referential: false,
        illumina,
    };
    file_header.write_to(&mut *output)?;

    let cmp_header = CompressionHeader {
        stream_count: stream_count as u8,
        methods: methods.clone(),
        pattern: pattern.clone(),
    };
    cmp_header.write_to(&mut *output)?;

    let initial_offset = 8 + cmp_header_size(&methods, stream_count);
    let block_budget = budget_override.unwrap_or_else(|| compute_block_budget(&methods));
    for m in &methods {
        if let MethodSpec::Bwt { block_mib } = m {
            let bwt_bytes = *block_mib as usize * 1024 * 1024;
            if block_budget != bwt_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "block budget ({} MiB) must match BWT block size ({} MiB)",
                        block_budget / (1024 * 1024),
                        block_mib
                    ),
                ));
            }
        }
    }
    let mut compressors = build_compressors(&methods, &pattern, block_budget, indexed)?;
    let method_specs = build_method_specs(&methods, &pattern);

    // --mprob: capture per-base markov probabilities on the NUC stream.
    let mprob_sink: Option<ProbSink> = if cli.mprob.is_some() {
        const NUC: usize = 1;
        if indexed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--mprob is not compatible with indexing (-I)",
            ));
        }
        let on_nuc = markov_idx.is_some()
            && stream_count > NUC
            && pattern.get(NUC).copied() == markov_idx.map(|i| i as u8);
        if !on_nuc {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--mprob requires the markov codec on the NUC stream (use -M markov)",
            ));
        }
        let sink: ProbSink = Arc::new(Mutex::new(Vec::new()));
        compressors[NUC] =
            Some(Box::new(MarkovCompressor::with_prob(four_bit, sink.clone())) as Box<dyn Compressor>);
        Some(sink)
    } else {
        None
    };

    let mut mux = MuxWriter::new(
        output,
        stream_count,
        block_budget,
        compressors,
        method_specs,
        index_tier,
        initial_offset,
    );

    let done_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut bg_thread = None;
    if let (Some(sink), Some(path)) = (mprob_sink.clone(), cli.mprob.as_ref()) {
        let path = path.clone();
        let sink = sink.clone();
        let done = done_flag.clone();
        let force = cli.force;
        bg_thread = Some(std::thread::spawn(move || {
            write_mprob_stream(&sink, &path, force, done)
        }));
    }
    if mprob_sink.is_some() {
        // Probabilities must be captured in stream order -> no parallel blocks.
        mux.force_sequential();
    }

    match file_type {
        FileType::Fasta => codec::fasta::encode_fasta(alphabet, case_mode, &mut input, &mut mux)?,
        FileType::Fastq => codec::fastq::encode_fastq(
            alphabet,
            case_mode,
            illumina,
            quality_lut.as_ref(),
            &mut input,
            &mut mux,
        )?,
    }

    let (mut output, index_builder, index_offset) = mux.finalize()?;

    if let Some(ref ib) = index_builder {
        ib.write_to(&mut *output)?;
    }

    Trailer {
        index_offset: if indexed { index_offset } else { 0 },
    }
    .write_to(&mut *output)?;
    output.flush()?;

    done_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Some(handle) = bg_thread {
        handle.join().unwrap()?;
    }

    if cli.verbose {
        eprintln!(
            "encode: {} {:.2}s",
            if file_type == FileType::Fasta {
                "FASTA"
            } else {
                "FASTQ"
            },
            start.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

/// Write the captured per-base probabilities to `path` as an xz -9 stream of
/// newline-separated 5-decimal values, and verify it decompresses back exactly.
fn write_mprob_stream(
    sink: &ProbSink,
    path: &Path,
    force: bool,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> io::Result<()> {
    if path.exists() && !force {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{}: exists (use --force)", path.display()),
        ));
    }

    let mut compressor = XzCompressor::new(XzParams::new_preset(3, false), num_cpus::get() as u32);
    let mut file = std::fs::File::create(path)?;

    let mut text_buf = String::with_capacity(1024 * 1024);
    let mut comp_out = Vec::with_capacity(1024 * 1024);

    use std::fmt::Write as _;
    use std::io::Write as _;

    loop {
        let is_done = done.load(std::sync::atomic::Ordering::SeqCst);
        let drained = {
            let mut guard = sink.lock().unwrap();
            let data: Vec<u32> = guard.drain(..).collect();
            data
        };

        for &q in &drained {
            let _ = writeln!(text_buf, "{:.5}", q as f64 / 100_000.0);
            if text_buf.len() > 512 * 1024 {
                compressor.compress(text_buf.as_bytes(), &mut comp_out)?;
                file.write_all(&comp_out)?;
                text_buf.clear();
                comp_out.clear();
            }
        }

        if is_done && drained.is_empty() {
            break;
        }

        if drained.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    if !text_buf.is_empty() {
        compressor.compress(text_buf.as_bytes(), &mut comp_out)?;
        file.write_all(&comp_out)?;
        comp_out.clear();
    }

    compressor.finish(&mut comp_out)?;
    file.write_all(&comp_out)?;

    Ok(())
}
