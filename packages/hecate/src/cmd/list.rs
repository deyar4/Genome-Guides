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

use super::{build_decompressors, open_input};
use crate::Cli;
use hecate::container::*;
use hecate::mux::reader::StreamingMuxReader;
use hecate::pipeline::handler::{Handler, RecordInfo};
use hecate::pipeline::source::drive_hecate_source;
use std::io;

pub fn cmd_list(cli: &Cli) -> io::Result<()> {
    let mut input = open_input(cli)?;
    let file_header = FileHeader::read_from(&mut input)?;
    let cmp_header = CompressionHeader::read_from(&mut input)?;
    cmp_header.validate(file_header.stream_count())?;

    let stream_count = file_header.stream_count() as usize;
    let decompressors = build_decompressors(&cmp_header.methods, &cmp_header.pattern)?;
    let mut mux = StreamingMuxReader::new(input, stream_count, decompressors, file_header.indexed);

    let mut lister = ListHandler::new();
    drive_hecate_source(
        file_header.file_type,
        file_header.alphabet,
        file_header.case_mode,
        file_header.illumina,
        &mut mux,
        &mut lister,
    )?;
    Ok(())
}

struct ListHandler {
    current_header: Vec<u8>,
}

impl ListHandler {
    fn new() -> Self {
        Self {
            current_header: Vec::new(),
        }
    }
}

impl Handler for ListHandler {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        self.current_header.clear();
        self.current_header.extend_from_slice(header);
        Ok(())
    }
    fn nuc_chunk(&mut self, _data: &[u8]) -> io::Result<()> {
        Ok(())
    }
    fn case_chunk(&mut self, _data: &[u8]) -> io::Result<()> {
        Ok(())
    }
    fn extra(&mut self, _position: u64, _ch: u8) -> io::Result<()> {
        Ok(())
    }
    fn quality_chunk(&mut self, _data: &[u8]) -> io::Result<()> {
        Ok(())
    }
    fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
        println!(
            "{}\t{}",
            String::from_utf8_lossy(&self.current_header),
            info.base_count
        );
        Ok(())
    }
    fn eof(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn needs_stream_data(&self) -> bool {
        false
    }
}
