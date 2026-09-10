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
    build_decompressors, build_text_handler, cmp_header_size, open_filter_output, open_input,
};
use crate::Cli;
use hecate::container::*;
use hecate::mux::reader::StreamingMuxReader;
use hecate::pipeline::source::drive_hecate_source;
use hecate::processor::slice_filter::SliceFilter;
use hecate::stream::buffered::BufferedReader;
use hecate::stream::file::FileInput;
use hecate::stream::mmap::MmapInput;
use hecate::stream::traits::{StreamRead, StreamSeek, StreamWrite};
use std::io;

pub fn cmd_slice(cli: &Cli) -> io::Result<()> {
    if cli.slice.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no slice specs - use -S \"name\" or -S \"name:start:end\"",
        ));
    }

    let mut slice_specs = Vec::new();
    for s in &cli.slice {
        slice_specs.push(hecate::processor::slice_filter::SliceSpec::parse(s)?);
    }

    let output_path = cli.output.clone();
    let mut output = open_filter_output(cli, output_path.as_ref())?;

    // Try the indexed path: requires a seekable file input + indexed flag
    if let Some(ref path) = cli.file {
        if !cli.no_mmap {
            if let Ok(input) = MmapInput::open(path) {
                input.advise_random();
                return slice_seekable(input, cli, &mut *output, &slice_specs);
            }
        }
        let input = BufferedReader::new(FileInput::open(path)?);
        return slice_seekable(input, cli, &mut *output, &slice_specs);
    }

    // Stdin path: sequential only
    let mut input = open_input(cli)?;
    let file_header = FileHeader::read_from(&mut input)?;
    let cmp_header = CompressionHeader::read_from(&mut input)?;
    cmp_header.validate(file_header.stream_count())?;

    let stream_count = file_header.stream_count() as usize;
    let decompressors = build_decompressors(&cmp_header.methods, &cmp_header.pattern)?;
    let mut mux = StreamingMuxReader::new(input, stream_count, decompressors, file_header.indexed);

    {
        let handler = build_text_handler(cli, &mut *output, &file_header);
        let mut filter = SliceFilter::new(handler, slice_specs, file_header.alphabet);
        drive_hecate_source(
            file_header.file_type,
            file_header.alphabet,
            file_header.case_mode,
            file_header.illumina,
            &mut mux,
            &mut filter,
        )?;
    }

    output.flush()?;
    Ok(())
}

fn slice_seekable<R: StreamRead + StreamSeek>(
    mut input: R,
    cli: &Cli,
    output: &mut dyn StreamWrite,
    slice_specs: &[hecate::processor::slice_filter::SliceSpec],
) -> io::Result<()> {
    use hecate::index::reader::read_index;
    use hecate::mux::IndexedMuxReader;
    use hecate::pipeline::indexed_source::drive_indexed_slice;

    let file_header = FileHeader::read_from(&mut input)?;
    let cmp_header = CompressionHeader::read_from(&mut input)?;
    cmp_header.validate(file_header.stream_count())?;

    if file_header.indexed {
        input.seek(io::SeekFrom::End(-(Trailer::SIZE as i64)))?;
        let trailer = Trailer::read_from(&mut input)?;

        if trailer.index_offset > 0 {
            input.seek(io::SeekFrom::Start(trailer.index_offset))?;
            let index = read_index(&mut input)?;

            if index.tier == hecate::index::IndexTier::Full {
                let stream_count = file_header.stream_count() as usize;
                let decompressors = build_decompressors(&cmp_header.methods, &cmp_header.pattern)?;
                let mut reader = IndexedMuxReader::new(input, stream_count, decompressors, index);

                {
                    let handler = build_text_handler(cli, output, &file_header);
                    let mut filter =
                        SliceFilter::new(handler, slice_specs.to_vec(), file_header.alphabet);
                    drive_indexed_slice(
                        &mut reader,
                        file_header.file_type,
                        file_header.alphabet,
                        file_header.case_mode,
                        file_header.illumina,
                        slice_specs,
                        &mut filter,
                    )?;
                }

                output.flush()?;
                return Ok(());
            }
        }
    }

    // Fall through to sequential path
    let stream_count = file_header.stream_count() as usize;
    let decompressors = build_decompressors(&cmp_header.methods, &cmp_header.pattern)?;
    let data_start = 8 + cmp_header_size(&cmp_header.methods, stream_count);
    input.seek(io::SeekFrom::Start(data_start))?;

    let mut mux = StreamingMuxReader::new(input, stream_count, decompressors, file_header.indexed);
    {
        let handler = build_text_handler(cli, output, &file_header);
        let mut filter = SliceFilter::new(handler, slice_specs.to_vec(), file_header.alphabet);
        drive_hecate_source(
            file_header.file_type,
            file_header.alphabet,
            file_header.case_mode,
            file_header.illumina,
            &mut mux,
            &mut filter,
        )?;
    }
    output.flush()?;
    Ok(())
}
