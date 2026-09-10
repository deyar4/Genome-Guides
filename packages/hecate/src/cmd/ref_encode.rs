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
    compute_block_budget, open_input_path, open_output, parse_methods, parse_pattern,
};
use crate::Cli;
use hecate::container::*;
use hecate::hdiff::{self, HdiffOptions};
use hecate::mux::reader::StreamingMuxReader;
use hecate::mux::writer::MuxWriter;
use hecate::referential::{drain_all_streams, serialize_ref_meta, unsqueeze_nuc};
use hecate::stream::traits::StreamWrite;
use std::io;
use std::time::Instant;

pub fn cmd_ref_encode(cli: &Cli) -> io::Result<()> {
    let start = Instant::now();

    let ref_path = cli.reference.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "referential encode requires -R <reference.he>",
        )
    })?;
    let input_path = cli.file.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "referential encode requires an input .he file",
        )
    })?;
    let output_path = cli.output.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "referential encode requires -o <output.he>",
        )
    })?;

    // Open and drain reference
    let mut ref_input = open_input_path(ref_path, cli.no_mmap)?;
    let ref_header = FileHeader::read_from(&mut ref_input)?;
    let ref_cmp = CompressionHeader::read_from(&mut ref_input)?;
    ref_cmp.validate(ref_header.stream_count())?;
    let ref_stream_count = ref_header.stream_count() as usize;
    let ref_decomps = build_decompressors(&ref_cmp.methods, &ref_cmp.pattern)?;
    let mut ref_mux =
        StreamingMuxReader::new(ref_input, ref_stream_count, ref_decomps, ref_header.indexed);
    let mut ref_streams = drain_all_streams(&mut ref_mux, ref_stream_count)?;

    // Open and drain input
    let mut inp_input = open_input_path(input_path, cli.no_mmap)?;
    let inp_header = FileHeader::read_from(&mut inp_input)?;
    let inp_cmp = CompressionHeader::read_from(&mut inp_input)?;
    inp_cmp.validate(inp_header.stream_count())?;
    let inp_stream_count = inp_header.stream_count() as usize;
    let inp_decomps = build_decompressors(&inp_cmp.methods, &inp_cmp.pattern)?;
    let mut inp_mux =
        StreamingMuxReader::new(inp_input, inp_stream_count, inp_decomps, inp_header.indexed);
    let mut inp_streams = drain_all_streams(&mut inp_mux, inp_stream_count)?;

    // Validate compatibility
    if ref_header.alphabet != inp_header.alphabet {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "reference and input have different alphabet modes",
        ));
    }
    if ref_header.file_type != inp_header.file_type {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "reference and input have different file types (FASTA vs FASTQ)",
        ));
    }
    if ref_header.case_mode != inp_header.case_mode {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "reference and input have different case modes",
        ));
    }

    let stream_count = inp_stream_count;
    let alphabet = inp_header.alphabet;

    // Record original sizes before unsqueezing
    let original_sizes: Vec<u64> = inp_streams.iter().map(|s| s.len() as u64).collect();

    // Build diff blobs per stream
    let hdiff_opts = HdiffOptions {
        thread_num: num_cpus::get(),
        ..HdiffOptions::default()
    };

    let mut diff_blobs: Vec<Vec<u8>> = Vec::with_capacity(stream_count);
    for s in 0..stream_count {
        let (mut inp_data, mut ref_data) = if s == STREAM_NUC {
            (
                unsqueeze_nuc(alphabet, &inp_streams[s]),
                unsqueeze_nuc(alphabet, &ref_streams[s]),
            )
        } else {
            (
                std::mem::take(&mut inp_streams[s]),
                std::mem::take(&mut ref_streams[s]),
            )
        };

        // hdiff cannot handle empty inputs; store empty diff for empty streams.
        if inp_data.is_empty() && ref_data.is_empty() {
            diff_blobs.push(Vec::new());
            continue;
        }

        let diff = hdiff::create_single_compressed_diff_block(
            &mut inp_data,
            &mut ref_data,
            hdiff_opts.clone(),
        )
        .map_err(|_| io::Error::other(format!("hdiff failed on stream {}", s)))?;

        diff_blobs.push(diff.to_vec());
    }

    // Serialize metadata
    let meta = serialize_ref_meta(&inp_header, &inp_cmp, &original_sizes)?;

    // Set up output compression for the diff blobs
    let methods = if let Some(ref ms) = cli.methods {
        parse_methods(ms)?
    } else {
        vec![]
    };
    let pattern = if let Some(ref ps) = cli.pattern {
        parse_pattern(ps, stream_count)?
    } else if methods.is_empty() {
        vec![0xFF; stream_count]
    } else {
        vec![0; stream_count]
    };

    let compressed = !methods.is_empty();
    let mut output = open_output(Some(&output_path), cli.force)?;

    let out_header = FileHeader {
        alphabet: inp_header.alphabet,
        file_type: inp_header.file_type,
        case_mode: inp_header.case_mode,
        indexed: false,
        compressed,
        referential: true,
        illumina: false,
    };
    out_header.write_to(&mut *output)?;

    let out_cmp = CompressionHeader {
        stream_count: stream_count as u8,
        methods: methods.clone(),
        pattern: pattern.clone(),
    };
    out_cmp.write_to(&mut *output)?;

    let initial_offset = 8 + cmp_header_size(&methods, stream_count);
    let block_budget = compute_block_budget(&methods);
    let compressors = build_compressors(&methods, &pattern, block_budget, false)?;
    let method_specs = build_method_specs(&methods, &pattern);
    let mut mux = MuxWriter::new(
        output,
        stream_count,
        block_budget,
        compressors,
        method_specs,
        None,
        initial_offset,
    );

    // Write metadata as length-prefixed blob to stream 0, followed by stream 0's diff
    let meta_len = meta.len() as u32;
    mux.write_to_stream(STREAM_CTRL, &meta_len.to_le_bytes())?;
    mux.write_to_stream(STREAM_CTRL, &meta)?;
    mux.write_to_stream(STREAM_CTRL, &diff_blobs[STREAM_CTRL])?;

    // Write remaining streams
    for s in 1..stream_count {
        mux.write_to_stream(s, &diff_blobs[s])?;
    }

    let (mut output, _, _) = mux.finalize()?;
    Trailer { index_offset: 0 }.write_to(&mut *output)?;
    output.flush()?;

    if cli.verbose {
        eprintln!("ref-encode: {:.2}s", start.elapsed().as_secs_f64());
    }
    Ok(())
}
