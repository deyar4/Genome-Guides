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
use hecate::hdiff;
use hecate::mux::reader::StreamingMuxReader;
use hecate::mux::writer::MuxWriter;
use hecate::referential::{drain_all_streams, parse_ref_meta, squeeze_nuc, unsqueeze_nuc};
use hecate::stream::traits::StreamWrite;
use std::io;
use std::time::Instant;

pub fn cmd_ref_decode(cli: &Cli) -> io::Result<()> {
    let start = Instant::now();

    let ref_path = cli.reference.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "referential decode requires -R <reference.he>",
        )
    })?;
    let diff_path = cli.file.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "referential decode requires a diff .he file",
        )
    })?;
    let output_path = cli.output.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "referential decode requires -o <output.he>",
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
    let ref_streams = drain_all_streams(&mut ref_mux, ref_stream_count)?;

    // Open and drain diff file
    let mut diff_input = open_input_path(diff_path, cli.no_mmap)?;
    let diff_header = FileHeader::read_from(&mut diff_input)?;
    if !diff_header.referential {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input file is not a referential diff (FLAG_REFERENTIAL not set)",
        ));
    }
    let diff_cmp = CompressionHeader::read_from(&mut diff_input)?;
    diff_cmp.validate(diff_header.stream_count())?;
    let diff_stream_count = diff_header.stream_count() as usize;
    let diff_decomps = build_decompressors(&diff_cmp.methods, &diff_cmp.pattern)?;
    let mut diff_mux = StreamingMuxReader::new(
        diff_input,
        diff_stream_count,
        diff_decomps,
        diff_header.indexed,
    );
    let diff_streams = drain_all_streams(&mut diff_mux, diff_stream_count)?;

    // Parse metadata from stream 0
    let stream0 = &diff_streams[STREAM_CTRL];
    if stream0.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "stream 0 too short for metadata length prefix",
        ));
    }
    let meta_len = u32::from_le_bytes([stream0[0], stream0[1], stream0[2], stream0[3]]) as usize;
    let meta_end = 4 + meta_len;
    if stream0.len() < meta_end {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "stream 0 too short for metadata",
        ));
    }
    let (target_header, target_cmp, original_sizes, _) = parse_ref_meta(&stream0[4..meta_end])?;
    let ctrl_diff_data = &stream0[meta_end..];

    // Validate reference compatibility
    if ref_header.alphabet != target_header.alphabet {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reference alphabet does not match target metadata",
        ));
    }
    if ref_header.file_type != target_header.file_type {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reference file type does not match target metadata",
        ));
    }
    if ref_header.case_mode != target_header.case_mode {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reference case mode does not match target metadata",
        ));
    }

    let stream_count = ref_stream_count;
    let alphabet = target_header.alphabet;

    // Reconstruct each stream
    let mut reconstructed: Vec<Vec<u8>> = Vec::with_capacity(stream_count);
    for s in 0..stream_count {
        let diff_data = if s == STREAM_CTRL {
            ctrl_diff_data
        } else {
            &diff_streams[s]
        };

        // Empty diff blob means the stream was empty in both files.
        if diff_data.is_empty() {
            reconstructed.push(Vec::new());
            continue;
        }

        let ref_data = if s == STREAM_NUC {
            unsqueeze_nuc(alphabet, &ref_streams[s])
        } else {
            ref_streams[s].clone()
        };

        let patched = hdiff::patch_single_compressed_diff(&ref_data, diff_data)
            .map_err(|_| io::Error::other(format!("hdiff patch failed on stream {}", s)))?;

        let stream_data = if s == STREAM_NUC {
            let expanded = patched.to_vec();
            // Trim to original size before squeezing (remove unsqueeze padding)
            let orig_packed_len = original_sizes[s] as usize;
            let bases_per_byte = match alphabet {
                AlphabetMode::TwoBit => 4,
                AlphabetMode::FourBit => 2,
                AlphabetMode::EightBit => 1,
            };
            let orig_base_count = orig_packed_len * bases_per_byte;
            let trimmed = if expanded.len() > orig_base_count {
                &expanded[..orig_base_count]
            } else {
                &expanded
            };
            squeeze_nuc(alphabet, trimmed)
        } else {
            patched.to_vec()
        };

        reconstructed.push(stream_data);
    }

    // Write output .he
    let methods = if let Some(ref ms) = cli.methods {
        parse_methods(ms)?
    } else {
        target_cmp.methods.clone()
    };
    let pattern = if let Some(ref ps) = cli.pattern {
        parse_pattern(ps, stream_count)?
    } else if methods.is_empty() {
        vec![0xFF; stream_count]
    } else {
        target_cmp.pattern.clone()
    };

    let compressed = !methods.is_empty();
    let mut output = open_output(Some(&output_path), cli.force)?;

    let out_header = FileHeader {
        alphabet: target_header.alphabet,
        file_type: target_header.file_type,
        case_mode: target_header.case_mode,
        indexed: false,
        compressed,
        referential: false,
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

    for s in 0..stream_count {
        mux.write_to_stream(s, &reconstructed[s])?;
    }

    let (mut output, _, _) = mux.finalize()?;
    Trailer { index_offset: 0 }.write_to(&mut *output)?;
    output.flush()?;

    if cli.verbose {
        eprintln!("ref-decode: {:.2}s", start.elapsed().as_secs_f64());
    }
    Ok(())
}
