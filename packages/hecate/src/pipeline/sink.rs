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

use crate::codec::common::{unpack_bases, StreamingCaseDecoder};
use crate::container::*;
use crate::mux::writer::MuxWriter;
use crate::pipeline::handler::{Handler, RecordInfo};
use crate::stream::traits::StreamWrite;
use std::io;

/// Sink that writes events into a MuxWriter to produce a .he file.
pub struct HecateWriterSink<'a, W: StreamWrite> {
    mux: &'a mut MuxWriter<W>,
    case_mode: CaseMode,
    alphabet: AlphabetMode,
    last_extra_pos: u64,
    nuc_buf: Vec<u8>,
}

impl<'a, W: StreamWrite> HecateWriterSink<'a, W> {
    pub fn new(mux: &'a mut MuxWriter<W>, case_mode: CaseMode, alphabet: AlphabetMode) -> Self {
        Self {
            mux,
            case_mode,
            alphabet,
            last_extra_pos: 0,
            nuc_buf: Vec::new(),
        }
    }
}

impl<'a, W: StreamWrite> Handler for HecateWriterSink<'a, W> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        self.last_extra_pos = 0;
        self.nuc_buf.clear();
        let mut ctrl = [0u8; 5];
        ctrl[0] = TAG_HEADER;
        ctrl[1..5].copy_from_slice(&(header.len() as u32).to_le_bytes());
        self.mux.write_to_stream(STREAM_CTRL, &ctrl)?;
        self.mux.write_to_stream(STREAM_HDR, header)?;
        Ok(())
    }

    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        // Buffer nuc data for extra reconstruction, also write through
        self.nuc_buf.extend_from_slice(data);
        self.mux.write_to_stream(STREAM_NUC, data)
    }

    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        if self.case_mode == CaseMode::Include {
            self.mux.write_to_stream(STREAM_CASE, data)?;
        }
        Ok(())
    }

    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        // Split full byte into delta + upper bits for the on-disk format
        let (upper, _nuc_code) = match self.alphabet {
            AlphabetMode::TwoBit => (ch >> 2, ch & 0x03),
            AlphabetMode::FourBit => (ch >> 4, ch & 0x0F),
            AlphabetMode::EightBit => (ch, 0),
        };
        let delta = (position - self.last_extra_pos) as u32;
        self.last_extra_pos = position;
        let mut buf = [0u8; 5];
        buf[..4].copy_from_slice(&delta.to_le_bytes());
        buf[4] = upper;
        self.mux.write_to_stream(STREAM_EXTRA, &buf)
    }

    fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        self.mux.write_to_stream(STREAM_QUALITY, data)
    }

    fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
        let mut ctrl = Vec::with_capacity(25);
        ctrl.push(TAG_SEQ);
        ctrl.extend_from_slice(&info.packed_bytes.to_le_bytes());
        ctrl.extend_from_slice(&info.base_count.to_le_bytes());
        ctrl.extend_from_slice(&info.extra_count.to_le_bytes());
        ctrl.extend_from_slice(&info.case_bytes.to_le_bytes());
        self.mux.write_to_stream(STREAM_CTRL, &ctrl)?;

        if info.quality_bytes > 0 {
            let mut qctrl = Vec::with_capacity(9);
            qctrl.push(TAG_QUALITY);
            qctrl.extend_from_slice(&info.quality_bytes.to_le_bytes());
            self.mux.write_to_stream(STREAM_CTRL, &qctrl)?;
        }

        Ok(())
    }

    fn eof(&mut self) -> io::Result<()> {
        self.mux.write_to_stream(STREAM_CTRL, &[TAG_EOF])
    }
}

/// Sink that emits FASTA text from Handler events.
/// Decodes per-chunk for O(chunk_size) memory usage.
pub struct FastaTextSink<'a> {
    output: &'a mut dyn StreamWrite,
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    line_width: usize,
    // Per-record streaming state
    case_dec: StreamingCaseDecoder,
    extras: Vec<(u64, u8)>,
    base_offset: u64,
    base_count: u64,
    nuc_skip: u64,
    // Reusable scratch
    text_buf: Vec<u8>,
    wrap_buf: Vec<u8>,
    line_pos: usize,
}

impl<'a> FastaTextSink<'a> {
    pub fn new(
        output: &'a mut dyn StreamWrite,
        alphabet: AlphabetMode,
        case_mode: CaseMode,
        line_width: usize,
    ) -> Self {
        Self {
            output,
            alphabet,
            case_mode,
            line_width,
            case_dec: StreamingCaseDecoder::new(),
            extras: Vec::new(),
            base_offset: 0,
            base_count: 0,
            nuc_skip: 0,
            text_buf: Vec::new(),
            wrap_buf: Vec::new(),
            line_pos: 0,
        }
    }
}

impl<'a> Handler for FastaTextSink<'a> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        self.case_dec.reset();
        self.extras.clear();
        self.base_offset = 0;
        self.base_count = 0;
        self.nuc_skip = 0;
        self.line_pos = 0;
        self.output.write_all(b">")?;
        self.output.write_all(header)?;
        self.output.write_all(b"\n")
    }

    fn record_meta(&mut self, info: &RecordInfo) -> io::Result<()> {
        self.base_count = info.base_count;
        self.nuc_skip = info.nuc_skip;
        Ok(())
    }

    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        // Compute how many bases this chunk covers
        let bases_per_byte: u64 = match self.alphabet {
            AlphabetMode::TwoBit => 4,
            AlphabetMode::FourBit => 2,
            AlphabetMode::EightBit => 1,
        };
        let max_bases = data.len() as u64 * bases_per_byte;
        let remaining_bases = self.base_count - self.base_offset;
        let chunk_bases = max_bases.min(remaining_bases);

        // Unpack packed bytes to ASCII
        self.text_buf.clear();
        unpack_bases(self.alphabet, data, chunk_bases, &mut self.text_buf);

        // Apply extras that fall within [base_offset, base_offset + chunk_bases)
        let chunk_end = self.base_offset + chunk_bases;
        let mut i = 0;
        while i < self.extras.len() {
            let (pos, ch) = self.extras[i];
            if pos >= self.base_offset && pos < chunk_end {
                self.text_buf[(pos - self.base_offset) as usize] = ch;
                self.extras.swap_remove(i);
            } else {
                i += 1;
            }
        }

        // Apply case
        if self.case_mode == CaseMode::Include {
            self.case_dec.apply_case(&mut self.text_buf);
        }

        // Strip nuc_skip padding bases from the front of the first chunk
        let skip = self.nuc_skip.saturating_sub(self.base_offset) as usize;
        let out_buf = &self.text_buf[skip..];

        // Write with line wrapping. Build the wrapped chunk in `wrap_buf`
        // first, then issue a single write_all -- avoids one syscall per line.
        if self.line_width == 0 {
            self.output.write_all(out_buf)?;
        } else if !out_buf.is_empty() {
            let lw = self.line_width;
            let buf = &mut self.wrap_buf;
            buf.clear();
            // Worst-case extra newlines: ceil(out_buf.len() / lw) + 1.
            buf.reserve(out_buf.len() + out_buf.len() / lw + 2);

            let mut src = out_buf;
            // Finish any partially-written line first.
            if self.line_pos > 0 {
                let avail = lw - self.line_pos;
                let take = avail.min(src.len());
                buf.extend_from_slice(&src[..take]);
                self.line_pos += take;
                src = &src[take..];
                if self.line_pos == lw {
                    buf.push(b'\n');
                    self.line_pos = 0;
                }
            }
            // Bulk-emit complete lines.
            let full_lines = src.len() / lw;
            for i in 0..full_lines {
                let line = &src[i * lw..(i + 1) * lw];
                buf.extend_from_slice(line);
                buf.push(b'\n');
            }
            // Trailing partial line stays as in-progress.
            let tail_start = full_lines * lw;
            if tail_start < src.len() {
                let tail = &src[tail_start..];
                buf.extend_from_slice(tail);
                self.line_pos = tail.len();
            }
            self.output.write_all(buf)?;
        }

        self.base_offset = chunk_end;
        Ok(())
    }

    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        self.case_dec.feed(data);
        Ok(())
    }

    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        self.extras.push((position, ch));
        Ok(())
    }

    fn quality_chunk(&mut self, _data: &[u8]) -> io::Result<()> {
        Ok(())
    }

    fn end_record(&mut self, _info: &RecordInfo) -> io::Result<()> {
        // Finish the last line
        if self.line_width == 0 || self.line_pos > 0 {
            self.output.write_all(b"\n")?;
        }
        Ok(())
    }

    fn eof(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Sink that emits FASTQ text from Handler events.
/// Decodes per-chunk for O(chunk_size) memory usage.
pub struct FastqTextSink<'a> {
    output: &'a mut dyn StreamWrite,
    alphabet: AlphabetMode,
    case_mode: CaseMode,
    // Per-record streaming state
    case_dec: StreamingCaseDecoder,
    extras: Vec<(u64, u8)>,
    base_offset: u64,
    base_count: u64,
    nuc_skip: u64,
    needs_separator: bool,
    // Reusable scratch
    text_buf: Vec<u8>,
}

impl<'a> FastqTextSink<'a> {
    pub fn new(
        output: &'a mut dyn StreamWrite,
        alphabet: AlphabetMode,
        case_mode: CaseMode,
    ) -> Self {
        Self {
            output,
            alphabet,
            case_mode,
            case_dec: StreamingCaseDecoder::new(),
            extras: Vec::new(),
            base_offset: 0,
            base_count: 0,
            nuc_skip: 0,
            needs_separator: false,
            text_buf: Vec::new(),
        }
    }
}

impl<'a> Handler for FastqTextSink<'a> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        self.case_dec.reset();
        self.extras.clear();
        self.base_offset = 0;
        self.base_count = 0;
        self.nuc_skip = 0;
        self.needs_separator = false;
        self.output.write_all(b"@")?;
        self.output.write_all(header)?;
        self.output.write_all(b"\n")
    }

    fn record_meta(&mut self, info: &RecordInfo) -> io::Result<()> {
        self.base_count = info.base_count;
        self.nuc_skip = info.nuc_skip;
        Ok(())
    }

    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        let bases_per_byte: u64 = match self.alphabet {
            AlphabetMode::TwoBit => 4,
            AlphabetMode::FourBit => 2,
            AlphabetMode::EightBit => 1,
        };
        let max_bases = data.len() as u64 * bases_per_byte;
        let remaining_bases = self.base_count - self.base_offset;
        let chunk_bases = max_bases.min(remaining_bases);

        self.text_buf.clear();
        unpack_bases(self.alphabet, data, chunk_bases, &mut self.text_buf);

        // Apply extras
        let chunk_end = self.base_offset + chunk_bases;
        let mut i = 0;
        while i < self.extras.len() {
            let (pos, ch) = self.extras[i];
            if pos >= self.base_offset && pos < chunk_end {
                self.text_buf[(pos - self.base_offset) as usize] = ch;
                self.extras.swap_remove(i);
            } else {
                i += 1;
            }
        }

        // Apply case
        if self.case_mode == CaseMode::Include {
            self.case_dec.apply_case(&mut self.text_buf);
        }

        // Strip nuc_skip padding bases from the front of the first chunk
        let skip = self.nuc_skip.saturating_sub(self.base_offset) as usize;
        self.output.write_all(&self.text_buf[skip..])?;
        self.base_offset = chunk_end;
        self.needs_separator = true;
        Ok(())
    }

    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        self.case_dec.feed(data);
        Ok(())
    }

    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        self.extras.push((position, ch));
        Ok(())
    }

    fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        if self.needs_separator {
            self.output.write_all(b"\n+\n")?;
            self.needs_separator = false;
        }
        self.output.write_all(data)
    }

    fn end_record(&mut self, _info: &RecordInfo) -> io::Result<()> {
        self.output.write_all(b"\n")
    }

    fn eof(&mut self) -> io::Result<()> {
        Ok(())
    }
}
