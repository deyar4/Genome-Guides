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
    build_compressors, build_decompressors, build_method_specs, cmp_header_size,
    compute_block_budget, open_input, open_output, parse_methods, parse_pattern, resolve_alphabet,
    resolve_case_mode, resolve_index_tier, wrap_processors,
};
use crate::Cli;
use hecate::container::*;
use hecate::index::IndexTier;
use hecate::mux::reader::StreamingMuxReader;
use hecate::mux::writer::MuxWriter;
use hecate::pipeline::sink::HecateWriterSink;
use hecate::pipeline::source::drive_hecate_source;
use hecate::stream::traits::StreamWrite;
use std::io;
use std::time::Instant;

pub fn cmd_recode(cli: &Cli) -> io::Result<()> {
    if cli.filter.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--filter is not supported in recode mode",
        ));
    }
    let start = Instant::now();
    let mut input = open_input(cli)?;

    let src_header = FileHeader::read_from(&mut input)?;
    let src_cmp = CompressionHeader::read_from(&mut input)?;
    src_cmp.validate(src_header.stream_count())?;

    let src_stream_count = src_header.stream_count() as usize;
    let decompressors = build_decompressors(&src_cmp.methods, &src_cmp.pattern)?;
    let mut mux_reader =
        StreamingMuxReader::new(input, src_stream_count, decompressors, src_header.indexed);

    let alphabet = if cli.two_bit || cli.four_bit || cli.eight_bit {
        resolve_alphabet(cli)
    } else {
        src_header.alphabet
    };
    let case_mode = if cli.ignore_case || cli.joint_case {
        resolve_case_mode(cli)
    } else {
        src_header.case_mode
    };
    let file_type = if cli.strip_quality && src_header.file_type == FileType::Fastq {
        FileType::Fasta
    } else {
        src_header.file_type
    };
    let stream_count = file_type.stream_count() as usize;

    let mut methods = if let Some(ref ms) = cli.methods {
        parse_methods(ms)?
    } else {
        src_cmp.methods.clone()
    };
    // Patch Markov/Bmarkov methods with four_bit based on target alphabet.
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
    } else if src_cmp.pattern.len() == stream_count {
        src_cmp.pattern.clone()
    } else if let Some(mi) = markov_idx {
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
        None => {
            let tier = if src_header.indexed {
                Some(IndexTier::Full)
            } else {
                None
            };
            (tier, None)
        }
    };
    let indexed = index_tier.is_some();
    let compressed = !methods.is_empty();

    let output_path = cli.output.clone().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "recode requires -o <output>")
    })?;
    let mut output = open_output(Some(&output_path), cli.force)?;

    let new_header = FileHeader {
        alphabet,
        file_type,
        case_mode,
        indexed,
        compressed,
        referential: false,
        illumina: false,
    };
    new_header.write_to(&mut *output)?;

    let new_cmp = CompressionHeader {
        stream_count: stream_count as u8,
        methods: methods.clone(),
        pattern: pattern.clone(),
    };
    new_cmp.write_to(&mut *output)?;

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
    let compressors = build_compressors(&methods, &pattern, block_budget, indexed)?;
    let method_specs = build_method_specs(&methods, &pattern);
    let mut mux_writer = MuxWriter::new(
        output,
        stream_count,
        block_budget,
        compressors,
        method_specs,
        index_tier,
        initial_offset,
    );

    {
        let sink: Box<dyn hecate::pipeline::handler::Handler + '_> =
            Box::new(HecateWriterSink::new(&mut mux_writer, case_mode, alphabet));
        let mut handler = wrap_processors(sink, cli, src_header.alphabet, src_header.case_mode);
        drive_hecate_source(
            src_header.file_type,
            src_header.alphabet,
            src_header.case_mode,
            src_header.illumina,
            &mut mux_reader,
            &mut *handler,
        )?;
    }

    let (mut output, index_builder, index_offset) = mux_writer.finalize()?;

    if let Some(ref ib) = index_builder {
        ib.write_to(&mut *output)?;
    }

    Trailer {
        index_offset: if indexed { index_offset } else { 0 },
    }
    .write_to(&mut *output)?;
    output.flush()?;

    if cli.verbose {
        eprintln!("recode: {:.2}s", start.elapsed().as_secs_f64());
    }
    Ok(())
}
