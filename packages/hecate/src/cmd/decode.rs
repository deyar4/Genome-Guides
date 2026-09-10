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
    build_decompressors, build_text_handler, open_filter_output, open_input, resolve_output_decode,
};
use crate::Cli;
use hecate::container::*;
use hecate::mux::reader::StreamingMuxReader;
use hecate::pipeline::source::drive_hecate_source;
use hecate::stream::traits::StreamWrite;
use std::io;
use std::time::Instant;

pub fn cmd_decode(cli: &Cli) -> io::Result<()> {
    let start = Instant::now();

    let mut input = open_input(cli)?;

    let file_header = FileHeader::read_from(&mut input)?;
    let cmp_header = CompressionHeader::read_from(&mut input)?;
    cmp_header.validate(file_header.stream_count())?;

    let stream_count = file_header.stream_count() as usize;
    let decompressors = build_decompressors(&cmp_header.methods, &cmp_header.pattern)?;
    let mut mux = StreamingMuxReader::new(input, stream_count, decompressors, file_header.indexed);

    let output_type = if cli.strip_quality {
        FileType::Fasta
    } else {
        file_header.file_type
    };
    let output_path = if cli.filter.is_some() {
        cli.output.clone()
    } else {
        resolve_output_decode(cli, output_type)?
    };
    let mut output = open_filter_output(cli, output_path.as_ref())?;

    {
        let mut handler = build_text_handler(cli, &mut *output, &file_header);
        drive_hecate_source(
            file_header.file_type,
            file_header.alphabet,
            file_header.case_mode,
            file_header.illumina,
            &mut mux,
            &mut *handler,
        )?;
    }

    output.flush()?;

    if cli.verbose {
        eprintln!("decode: {:.2}s", start.elapsed().as_secs_f64());
    }
    Ok(())
}
