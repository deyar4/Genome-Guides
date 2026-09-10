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

use super::nuc_pattern::{parse_nuc_pattern, NucPattern};
use super::{build_decompressors, open_input, open_output};
use crate::Cli;
use hecate::codec::common::{unpack_bases, StreamingCaseDecoder};
use hecate::container::*;
use hecate::mux::reader::StreamingMuxReader;
use hecate::pipeline::handler::{Handler, RecordInfo};
use hecate::pipeline::source::drive_hecate_source;
use hecate::processor::slice_filter::SliceFilter;
use hecate::stream::traits::StreamWrite;
use std::io;

/// Handler that searches for nucleotide patterns in streaming fashion,
/// processing data chunk-by-chunk with a sliding window.  Memory usage
/// is O(chunk_size + overlap) regardless of sequence length.
///
/// Handles `nuc_skip` correctly: when upstream is a SliceFilter that
/// prepends padding bases for byte alignment, those padding bases are
/// excluded from pattern matching and position reporting.
struct QueryHandler<'a> {
    output: &'a mut dyn StreamWrite,
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    pattern: NucPattern,
    neighbourhood: usize,
    /// Whether the CSV header has been printed.
    header_printed: bool,
    /// Global base offset across all sequences (counts only real bases,
    /// excluding nuc_skip padding).
    global_offset: u64,

    // Per-record state
    header_buf: Vec<u8>,
    base_count: u64,
    nuc_skip: u64,
    case_dec: StreamingCaseDecoder,

    // Streaming search state
    /// Sliding window of decoded ASCII bases (post-skip).
    window: Vec<u8>,
    /// Extras for the current chunk (cleared after each nuc_chunk).
    chunk_extras: Vec<(u64, u8)>,
    /// Per-chunk scratch buffer for unpacking.
    text_buf: Vec<u8>,
    /// Sequence position (0-based, post-skip) of window[0].
    window_seq_pos: u64,
    /// Total bases decoded so far (pre-skip), for computing chunk_bases.
    packed_base_offset: u64,
    /// Minimum seq_pos for which matches haven't been fully processed.
    /// Matches at seq_pos < next_emit_pos are skipped (already committed).
    next_emit_pos: u64,
    /// Precomputed overlap: pat_len - 1 + 2 * neighbourhood.
    overlap: usize,
    /// Early exit flag for non-global search.
    found_first: bool,
}

impl<'a> QueryHandler<'a> {
    fn new(
        output: &'a mut dyn StreamWrite,
        alphabet: AlphabetMode,
        case_mode: CaseMode,
        pattern: NucPattern,
        neighbourhood: usize,
    ) -> Self {
        let overlap = pattern.len().saturating_sub(1) + 2 * neighbourhood;
        Self {
            output,
            alphabet,
            case_mode,
            pattern,
            neighbourhood,
            header_printed: false,
            global_offset: 0,
            header_buf: Vec::new(),
            base_count: 0,
            nuc_skip: 0,
            case_dec: StreamingCaseDecoder::new(),
            window: Vec::new(),
            chunk_extras: Vec::new(),
            text_buf: Vec::new(),
            window_seq_pos: 0,
            packed_base_offset: 0,
            next_emit_pos: 0,
            overlap,
            found_first: false,
        }
    }

    fn emit_csv_header(&mut self) -> io::Result<()> {
        if self.header_printed {
            return Ok(());
        }
        self.header_printed = true;
        if self.neighbourhood > 0 {
            self.output
                .write_all(b"seq_name,seq_pos,global_pos,match,left,right\n")
        } else {
            self.output
                .write_all(b"seq_name,seq_pos,global_pos,match\n")
        }
    }

    fn csv_escape_name(name: &[u8]) -> String {
        let s = String::from_utf8_lossy(name);
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.into_owned()
        }
    }

    fn emit_match(
        &mut self,
        window_pos: usize,
        seq_pos: u64,
        global_pos: u64,
        matched: &[u8],
    ) -> io::Result<()> {
        let seq_name = Self::csv_escape_name(&self.header_buf);
        let nbr = self.neighbourhood;
        let pat_len = self.pattern.len();
        let matched_str = String::from_utf8_lossy(matched);

        if nbr > 0 {
            let left_start = window_pos.saturating_sub(nbr);
            let left = String::from_utf8_lossy(&self.window[left_start..window_pos]);
            let right_start = window_pos + pat_len;
            let right_end = (right_start + nbr).min(self.window.len());
            let right = if right_start < right_end {
                String::from_utf8_lossy(&self.window[right_start..right_end])
            } else {
                std::borrow::Cow::Borrowed("")
            };
            let line = format!(
                "{},{},{},{},{},{}\n",
                seq_name, seq_pos, global_pos, matched_str, left, right,
            );
            self.output.write_all(line.as_bytes())?;
        } else {
            let line = format!("{},{},{},{}\n", seq_name, seq_pos, global_pos, matched_str,);
            self.output.write_all(line.as_bytes())?;
        }
        Ok(())
    }
}

impl Handler for QueryHandler<'_> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        self.header_buf.clear();
        self.header_buf.extend_from_slice(header);
        self.window.clear();
        self.chunk_extras.clear();
        self.case_dec.reset();
        self.base_count = 0;
        self.nuc_skip = 0;
        self.window_seq_pos = 0;
        self.packed_base_offset = 0;
        self.next_emit_pos = 0;
        self.found_first = false;
        Ok(())
    }

    fn record_meta(&mut self, info: &RecordInfo) -> io::Result<()> {
        self.base_count = info.base_count;
        self.nuc_skip = info.nuc_skip;
        Ok(())
    }

    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        self.case_dec.feed(data);
        Ok(())
    }

    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        self.chunk_extras.push((position, ch));
        Ok(())
    }

    fn quality_chunk(&mut self, _data: &[u8]) -> io::Result<()> {
        Ok(())
    }

    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        let bases_per_byte: u64 = match self.alphabet {
            AlphabetMode::TwoBit => 4,
            AlphabetMode::FourBit => 2,
            AlphabetMode::EightBit => 1,
        };
        let max_bases = data.len() as u64 * bases_per_byte;
        let remaining_bases = self.base_count - self.packed_base_offset;
        let chunk_bases = max_bases.min(remaining_bases);

        // Early exit for non-global after first match found
        if self.found_first {
            self.packed_base_offset += chunk_bases;
            self.chunk_extras.clear();
            return Ok(());
        }

        // Decode chunk to ASCII
        self.text_buf.clear();
        if self.alphabet == AlphabetMode::EightBit {
            // 8-bit: data IS ASCII text, no extras, nuc_skip = 0
            self.text_buf.extend_from_slice(data);
            if self.case_mode == CaseMode::Include {
                self.case_dec.apply_case(&mut self.text_buf);
            }
        } else {
            // 2/4-bit: unpack + extras + case
            unpack_bases(self.alphabet, data, chunk_bases, &mut self.text_buf);
            for &(pos, ch) in &self.chunk_extras {
                let idx = (pos - self.packed_base_offset) as usize;
                if idx < self.text_buf.len() {
                    self.text_buf[idx] = ch;
                }
            }
            if self.case_mode == CaseMode::Include {
                self.case_dec.apply_case(&mut self.text_buf);
            }
        }
        self.chunk_extras.clear();

        // Strip nuc_skip padding from front of first chunk(s)
        let skip = self.nuc_skip.saturating_sub(self.packed_base_offset) as usize;
        let skip = skip.min(self.text_buf.len());

        // Append decoded (post-skip) bases to sliding window
        self.window.extend_from_slice(&self.text_buf[skip..]);

        self.packed_base_offset += chunk_bases;
        let is_last = self.packed_base_offset >= self.base_count;

        // Search the window
        let pat_len = self.pattern.len();
        let nbr = self.neighbourhood;

        // Start search past already-committed positions
        let search_start = (self.next_emit_pos.saturating_sub(self.window_seq_pos)) as usize;
        let search_start = search_start.min(self.window.len());

        if self.window.len().saturating_sub(search_start) >= pat_len {
            let matches = self.pattern.find_in(&self.window[search_start..]);

            for m in &matches {
                let window_pos = search_start + m.pos;

                // Check right context (on last chunk, emit all)
                if !is_last && window_pos + pat_len + nbr > self.window.len() {
                    break;
                }

                let seq_pos = self.window_seq_pos + window_pos as u64;
                let global_pos = self.global_offset + seq_pos;

                self.emit_csv_header()?;
                self.emit_match(window_pos, seq_pos, global_pos, &m.matched)?;

                if !self.pattern.global {
                    self.found_first = true;
                    break;
                }
            }
        }

        // Advance next_emit_pos past fully-committed region
        if !is_last && self.window.len() >= pat_len + nbr {
            let committed = self.window_seq_pos + (self.window.len() - pat_len - nbr) as u64 + 1;
            self.next_emit_pos = self.next_emit_pos.max(committed);
        }

        // Drain committed bytes, keeping overlap for boundary matches + context
        if !is_last && self.window.len() > self.overlap {
            let drain = self.window.len() - self.overlap;
            self.window.drain(..drain);
            self.window_seq_pos += drain as u64;
        }

        Ok(())
    }

    fn end_record(&mut self, _info: &RecordInfo) -> io::Result<()> {
        // Only count real bases (not padding) in the global offset
        self.global_offset += self.base_count - self.nuc_skip;
        self.window.clear();
        Ok(())
    }

    fn eof(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn cmd_query(cli: &Cli) -> io::Result<()> {
    let grep_str = cli.grep.as_deref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "no grep pattern specified (-G)",
        )
    })?;
    let pattern = parse_nuc_pattern(grep_str)?;

    let mut input = open_input(cli)?;
    let file_header = FileHeader::read_from(&mut input)?;
    let cmp_header = CompressionHeader::read_from(&mut input)?;
    cmp_header.validate(file_header.stream_count())?;

    let stream_count = file_header.stream_count() as usize;
    let decompressors = build_decompressors(&cmp_header.methods, &cmp_header.pattern)?;
    let mut mux = StreamingMuxReader::new(input, stream_count, decompressors, file_header.indexed);

    let mut output = open_output(cli.output.as_ref(), cli.force)?;

    {
        let handler = QueryHandler::new(
            &mut *output,
            file_header.alphabet,
            file_header.case_mode,
            pattern,
            cli.neighbourhood,
        );

        if cli.slice.is_empty() {
            let mut handler = handler;
            drive_hecate_source(
                file_header.file_type,
                file_header.alphabet,
                file_header.case_mode,
                file_header.illumina,
                &mut mux,
                &mut handler,
            )?;
        } else {
            let mut slice_specs = Vec::new();
            for s in &cli.slice {
                slice_specs.push(hecate::processor::slice_filter::SliceSpec::parse(s)?);
            }
            let mut filter = SliceFilter::new(Box::new(handler), slice_specs, file_header.alphabet);
            drive_hecate_source(
                file_header.file_type,
                file_header.alphabet,
                file_header.case_mode,
                file_header.illumina,
                &mut mux,
                &mut filter,
            )?;
        }
    }

    output.flush()?;
    Ok(())
}
