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

use crate::codec::common::{CaseEncoder, StreamingCaseDecoder};
use crate::container::AlphabetMode;
use crate::pipeline::handler::{Handler, RecordInfo};
use regex::Regex;
use std::io;

/// Specification for a sequence slice: header matcher + optional base range.
#[derive(Debug, Clone)]
pub struct SliceSpec {
    pub matcher: HeaderMatcher,
    /// 0-based inclusive start, 0-based exclusive end.
    pub start: Option<u64>,
    pub end: Option<u64>,
}

impl SliceSpec {
    /// Parse a slice spec from a CLI string.
    /// Formats: "name", "name:start:end", "/regex/", "/regex/:start:end",
    /// "#N", "#N-M", "#N:start:end", "#N-M:start:end"
    /// For name/regex: start and end are 1-based inclusive, converted to 0-based [start, end).
    /// For #N: N is 0-based ordinal. Ranges are half-open [N, M).
    pub fn parse(s: &str) -> io::Result<Self> {
        // Check for ordinal access: #N, #N-M, #N:start:end, #N-M:start:end
        if let Some(after_hash) = s.strip_prefix('#') {
            return Self::parse_ordinal(after_hash);
        }

        // Check for regex pattern: /pattern/ or /pattern/:start:end
        if let Some(after_slash) = s.strip_prefix('/') {
            if let Some(end_slash) = after_slash.find('/') {
                let pattern = &after_slash[..end_slash];
                let rest = &after_slash[end_slash + 1..];
                let regex = Regex::new(pattern).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidInput, format!("invalid regex: {}", e))
                })?;
                let (start, end) = parse_range(rest)?;
                return Ok(Self {
                    matcher: HeaderMatcher::Regex(regex),
                    start,
                    end,
                });
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unterminated regex pattern",
            ));
        }

        // Plain name or name:start:end
        let parts: Vec<&str> = s.splitn(3, ':').collect();
        match parts.len() {
            1 => Ok(Self {
                matcher: HeaderMatcher::Exact(parts[0].to_string()),
                start: None,
                end: None,
            }),
            3 => {
                let start: u64 = parts[1].parse().map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidInput, format!("invalid start: {}", e))
                })?;
                let end: u64 = parts[2].parse().map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidInput, format!("invalid end: {}", e))
                })?;
                if start == 0 || end == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "start and end must be >= 1 (1-based)",
                    ));
                }
                if start > end {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "start must be <= end",
                    ));
                }
                // Convert from 1-based inclusive to 0-based [start, end)
                Ok(Self {
                    matcher: HeaderMatcher::Exact(parts[0].to_string()),
                    start: Some(start - 1),
                    end: Some(end), // 1-based inclusive end == 0-based exclusive
                })
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid slice spec: {}", s),
            )),
        }
    }

    pub fn matches(&self, header: &[u8]) -> bool {
        self.matcher.matches(header)
    }

    /// Match by header content or ordinal position.
    pub fn matches_with_ordinal(&self, header: &[u8], ordinal: usize) -> bool {
        self.matcher.matches(header) || self.matcher.matches_ordinal(ordinal)
    }

    /// Parse ordinal spec after stripping the '#' prefix.
    /// Formats: "N", "N-M", "N:start:end", "N-M:start:end"
    fn parse_ordinal(s: &str) -> io::Result<Self> {
        // Split off the base range part (after the ordinal spec)
        // First, find where the ordinal spec ends: it's either at a ':' or end of string
        // But we also need to handle '#N-M:S:E' where '-' is part of the ordinal
        let (ordinal_part, range_part) = if let Some(colon_pos) = s.find(':') {
            (&s[..colon_pos], &s[colon_pos..])
        } else {
            (s, "")
        };

        // Parse the ordinal part: N or N-M
        let matcher = if let Some(dash_pos) = ordinal_part.find('-') {
            let start: usize = ordinal_part[..dash_pos].parse().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid ordinal start: {}", e),
                )
            })?;
            let end: usize = ordinal_part[dash_pos + 1..].parse().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid ordinal end: {}", e),
                )
            })?;
            if start >= end {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "ordinal range start must be < end",
                ));
            }
            HeaderMatcher::OrdinalRange(start, end)
        } else {
            let n: usize = ordinal_part.parse().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid ordinal: {}", e),
                )
            })?;
            HeaderMatcher::Ordinal(n)
        };

        // Parse the optional base range ":start:end" (1-based inclusive)
        let (start, end) = if range_part.is_empty() {
            (None, None)
        } else {
            parse_range(range_part)?
        };

        Ok(Self {
            matcher,
            start,
            end,
        })
    }

    pub fn base_range(&self) -> Option<(u64, u64)> {
        match (self.start, self.end) {
            (Some(s), Some(e)) => Some((s, e)),
            _ => None,
        }
    }
}

fn parse_range(s: &str) -> io::Result<(Option<u64>, Option<u64>)> {
    if s.is_empty() {
        return Ok((None, None));
    }
    if !s.starts_with(':') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected ':' after regex",
        ));
    }
    let rest = &s[1..];
    let parts: Vec<&str> = rest.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected start:end after regex",
        ));
    }
    let start: u64 = parts[0].parse().map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("invalid start: {}", e))
    })?;
    let end: u64 = parts[1]
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("invalid end: {}", e)))?;
    if start == 0 || end == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "start and end must be >= 1",
        ));
    }
    if start > end {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "start must be <= end",
        ));
    }
    Ok((Some(start - 1), Some(end)))
}

/// Matches a sequence header.
#[derive(Debug, Clone)]
pub enum HeaderMatcher {
    /// Exact match against the full header text.
    Exact(String),
    /// Regex match against the header text.
    Regex(Regex),
    /// Match by 0-based ordinal index.
    Ordinal(usize),
    /// Match by 0-based ordinal range [start, end) (half-open).
    OrdinalRange(usize, usize),
}

impl HeaderMatcher {
    fn matches(&self, header: &[u8]) -> bool {
        match self {
            HeaderMatcher::Exact(name) => header == name.as_bytes(),
            HeaderMatcher::Regex(re) => {
                if let Ok(s) = std::str::from_utf8(header) {
                    re.is_match(s)
                } else {
                    false
                }
            }
            // Ordinals never match by header content
            HeaderMatcher::Ordinal(_) | HeaderMatcher::OrdinalRange(_, _) => false,
        }
    }

    /// Check if this matcher matches by ordinal position.
    fn matches_ordinal(&self, ordinal: usize) -> bool {
        match self {
            HeaderMatcher::Ordinal(n) => ordinal == *n,
            HeaderMatcher::OrdinalRange(start, end) => ordinal >= *start && ordinal < *end,
            _ => false,
        }
    }

    /// Returns true if this is an ordinal-based matcher.
    pub fn is_ordinal(&self) -> bool {
        matches!(
            self,
            HeaderMatcher::Ordinal(_) | HeaderMatcher::OrdinalRange(_, _)
        )
    }

    /// Yield the ordinal indices this matcher covers, capped by seq_count.
    pub fn ordinal_indices(&self, seq_count: usize) -> Vec<usize> {
        match self {
            HeaderMatcher::Ordinal(n) => {
                if *n < seq_count {
                    vec![*n]
                } else {
                    vec![]
                }
            }
            HeaderMatcher::OrdinalRange(start, end) => {
                let s = (*start).min(seq_count);
                let e = (*end).min(seq_count);
                (s..e).collect()
            }
            _ => vec![],
        }
    }
}

/// Filter state per record.
enum SliceState {
    /// Record does not match any spec.
    Suppressed,
    /// Record matches, pass through everything.
    PassAll,
    /// Record matches with a base sub-range.
    SubSlice {
        start: u64,
        end: u64,
        /// Number of padding bases prepended to align nuc data to byte boundary.
        nuc_skip: u64,
        // Nucleotide tracking (in bases, not bytes)
        nuc_base_pos: u64,
        // Case tracking
        case_decoder: StreamingCaseDecoder,
        case_encoder: CaseEncoder,
        case_base_pos: u64,
        /// Whether case data has been flushed to downstream before nuc data.
        case_flushed: bool,
        // Quality tracking
        qual_base_pos: u64,
        // Accumulated metadata for sliced output
        case_bytes_emitted: u32,
        quality_bytes_emitted: u64,
        extra_count_emitted: u32,
    },
}

/// Filters records by header and optionally sub-slices to a base range.
pub struct SliceFilter<'a> {
    downstream: Box<dyn Handler + 'a>,
    specs: Vec<SliceSpec>,
    state: SliceState,
    bases_per_byte: u64,
    /// Current sequence ordinal (0-based), incremented in begin_record.
    seq_ordinal: usize,
}

impl<'a> SliceFilter<'a> {
    pub fn new(
        downstream: Box<dyn Handler + 'a>,
        specs: Vec<SliceSpec>,
        alphabet: AlphabetMode,
    ) -> Self {
        let bases_per_byte = match alphabet {
            AlphabetMode::TwoBit => 4,
            AlphabetMode::FourBit => 2,
            AlphabetMode::EightBit => 1,
        };
        Self {
            downstream,
            specs,
            state: SliceState::Suppressed,
            bases_per_byte,
            seq_ordinal: 0,
        }
    }

    fn find_matching_spec(&self, header: &[u8]) -> Option<&SliceSpec> {
        self.specs
            .iter()
            .find(|s| s.matches_with_ordinal(header, self.seq_ordinal))
    }

    /// Set the current ordinal for the next begin_record call.
    /// Used by the indexed driver to set the ordinal before each record.
    pub fn set_ordinal(&mut self, ordinal: usize) {
        self.seq_ordinal = ordinal;
    }

    /// Apply a case checkpoint to fast-forward the case decoder.
    /// Must be called after `begin_record` and before `case_chunk`.
    /// Skips decoding bases 0..base_offset by initializing the decoder
    /// from the checkpoint state.
    pub fn apply_case_checkpoint(&mut self, base_offset: u64, run_remainder: u16, is_upper: bool) {
        if let SliceState::SubSlice {
            case_decoder,
            case_base_pos,
            ..
        } = &mut self.state
        {
            *case_base_pos = base_offset;
            case_decoder.seek_to_checkpoint(run_remainder, is_upper);
        }
    }
}

impl Handler for SliceFilter<'_> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        // Match using current ordinal, then increment for next record.
        let result = self.find_matching_spec(header).cloned();
        self.seq_ordinal += 1;
        match result.as_ref() {
            None => {
                self.state = SliceState::Suppressed;
                Ok(())
            }
            Some(spec) => match spec.base_range() {
                None => {
                    self.state = SliceState::PassAll;
                    self.downstream.begin_record(header)
                }
                Some((start, end)) => {
                    let bpb = self.bases_per_byte;
                    let nuc_skip = if bpb > 1 { start % bpb } else { 0 };

                    let mut case_encoder = CaseEncoder::new();
                    // Pre-pad with nuc_skip uppercase bits so case data aligns
                    // with the byte-aligned packed nuc data.
                    for _ in 0..nuc_skip {
                        case_encoder.push_bit(1);
                    }

                    self.state = SliceState::SubSlice {
                        start,
                        end,
                        nuc_skip,
                        nuc_base_pos: 0,
                        case_decoder: StreamingCaseDecoder::new(),
                        case_encoder,
                        case_base_pos: 0,
                        case_flushed: false,
                        qual_base_pos: 0,
                        case_bytes_emitted: 0,
                        quality_bytes_emitted: 0,
                        extra_count_emitted: 0,
                    };
                    self.downstream.begin_record(header)
                }
            },
        }
    }

    fn record_meta(&mut self, info: &RecordInfo) -> io::Result<()> {
        match &self.state {
            SliceState::Suppressed => Ok(()),
            SliceState::PassAll => self.downstream.record_meta(info),
            SliceState::SubSlice {
                start,
                end,
                nuc_skip,
                ..
            } => {
                let sliced_bases = end.saturating_sub(*start);
                let adjusted = RecordInfo {
                    base_count: nuc_skip + sliced_bases,
                    nuc_skip: *nuc_skip,
                    ..info.clone()
                };
                self.downstream.record_meta(&adjusted)
            }
        }
    }

    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        match &mut self.state {
            SliceState::Suppressed => Ok(()),
            SliceState::PassAll => self.downstream.nuc_chunk(data),
            SliceState::SubSlice {
                start,
                end,
                nuc_skip,
                nuc_base_pos,
                case_encoder,
                case_flushed,
                case_bytes_emitted,
                ..
            } => {
                let bpb = self.bases_per_byte;

                // Flush case data before the first nuc_chunk so downstream
                // sinks have case info when they process nuc data.
                if !*case_flushed {
                    let case_data = case_encoder.finish().to_vec();
                    *case_bytes_emitted = case_data.len() as u32;
                    if !case_data.is_empty() {
                        self.downstream.case_chunk(&case_data)?;
                    }
                    *case_flushed = true;
                }

                let chunk_base_start = *nuc_base_pos;
                let chunk_base_end = chunk_base_start + data.len() as u64 * bpb;
                *nuc_base_pos = chunk_base_end;

                // Aligned start: round down to byte boundary
                let aligned_start = *start - *nuc_skip;

                if chunk_base_end <= aligned_start || chunk_base_start >= *end {
                    return Ok(());
                }

                if bpb == 1 {
                    // 8-bit: 1 byte = 1 base, slice directly
                    let lo = (aligned_start.max(chunk_base_start) - chunk_base_start) as usize;
                    let hi = ((*end).min(chunk_base_end) - chunk_base_start) as usize;
                    self.downstream.nuc_chunk(&data[lo..hi])
                } else {
                    // Packed mode: convert base coordinates to byte coordinates
                    let first_base = aligned_start.max(chunk_base_start);
                    let last_base = (*end).min(chunk_base_end);
                    let byte_lo = ((first_base - chunk_base_start) / bpb) as usize;
                    let byte_hi = (last_base - chunk_base_start).div_ceil(bpb) as usize;
                    self.downstream.nuc_chunk(&data[byte_lo..byte_hi])
                }
            }
        }
    }

    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        match &mut self.state {
            SliceState::Suppressed => Ok(()),
            SliceState::PassAll => self.downstream.case_chunk(data),
            SliceState::SubSlice {
                start,
                end,
                case_decoder,
                case_encoder,
                case_base_pos,
                ..
            } => {
                case_decoder.feed(data);
                while case_decoder.has_remaining() {
                    let is_upper = case_decoder.next_is_upper();
                    if *case_base_pos >= *start && *case_base_pos < *end {
                        case_encoder.push_bit(if is_upper { 1 } else { 0 });
                    }
                    *case_base_pos += 1;
                }
                Ok(())
            }
        }
    }

    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        match &mut self.state {
            SliceState::Suppressed => Ok(()),
            SliceState::PassAll => self.downstream.extra(position, ch),
            SliceState::SubSlice {
                start,
                end,
                nuc_skip,
                extra_count_emitted,
                ..
            } => {
                if position >= *start && position < *end {
                    *extra_count_emitted += 1;
                    self.downstream.extra(position - *start + *nuc_skip, ch)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        match &mut self.state {
            SliceState::Suppressed => Ok(()),
            SliceState::PassAll => self.downstream.quality_chunk(data),
            SliceState::SubSlice {
                start,
                end,
                qual_base_pos,
                quality_bytes_emitted,
                ..
            } => {
                let chunk_start = *qual_base_pos;
                let chunk_end = chunk_start + data.len() as u64;
                *qual_base_pos = chunk_end;

                if chunk_end <= *start || chunk_start >= *end {
                    return Ok(());
                }

                let lo = ((*start).max(chunk_start) - chunk_start) as usize;
                let hi = ((*end).min(chunk_end) - chunk_start) as usize;
                *quality_bytes_emitted += (hi - lo) as u64;
                self.downstream.quality_chunk(&data[lo..hi])
            }
        }
    }

    fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
        match &mut self.state {
            SliceState::Suppressed => Ok(()),
            SliceState::PassAll => self.downstream.end_record(info),
            SliceState::SubSlice {
                start,
                end,
                nuc_skip,
                case_encoder,
                case_flushed,
                case_bytes_emitted,
                quality_bytes_emitted,
                extra_count_emitted,
                ..
            } => {
                // Flush case data if no nuc_chunk was received (edge case)
                if !*case_flushed {
                    let case_data = case_encoder.finish().to_vec();
                    *case_bytes_emitted = case_data.len() as u32;
                    if !case_data.is_empty() {
                        self.downstream.case_chunk(&case_data)?;
                    }
                }
                let sliced_bases = end.saturating_sub(*start);
                let bpb = self.bases_per_byte;
                let total_bases = *nuc_skip + sliced_bases;
                let packed = if bpb > 1 {
                    total_bases.div_ceil(bpb)
                } else {
                    total_bases
                };
                let sliced_info = RecordInfo {
                    base_count: total_bases,
                    packed_bytes: packed,
                    nuc_skip: *nuc_skip,
                    extra_count: *extra_count_emitted,
                    case_bytes: *case_bytes_emitted,
                    quality_bytes: *quality_bytes_emitted,
                };
                self.downstream.end_record(&sliced_info)
            }
        }
    }

    fn eof(&mut self) -> io::Result<()> {
        self.downstream.eof()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::common::{
        try_pack_2_bases_4bit, try_pack_4_bases_2bit, unpack_bases, CaseDecoder, CaseEncoder,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Test handler that records all events, including ordering.
    struct EventRecorder {
        headers: Rc<RefCell<Vec<String>>>,
        nuc_data: Rc<RefCell<Vec<u8>>>,
        case_data: Rc<RefCell<Vec<u8>>>,
        quality_data: Rc<RefCell<Vec<u8>>>,
        extras: Rc<RefCell<Vec<(u64, u8)>>>,
        end_base_counts: Rc<RefCell<Vec<u64>>>,
        end_nuc_skips: Rc<RefCell<Vec<u64>>>,
        eof_called: Rc<RefCell<bool>>,
        /// Tracks event ordering: 'C' = case_chunk, 'N' = nuc_chunk
        event_order: Rc<RefCell<Vec<char>>>,
    }

    impl EventRecorder {
        fn new() -> (Self, RecorderState) {
            let headers = Rc::new(RefCell::new(Vec::new()));
            let nuc_data = Rc::new(RefCell::new(Vec::new()));
            let case_data = Rc::new(RefCell::new(Vec::new()));
            let quality_data = Rc::new(RefCell::new(Vec::new()));
            let extras = Rc::new(RefCell::new(Vec::new()));
            let end_base_counts = Rc::new(RefCell::new(Vec::new()));
            let end_nuc_skips = Rc::new(RefCell::new(Vec::new()));
            let eof_called = Rc::new(RefCell::new(false));
            let event_order = Rc::new(RefCell::new(Vec::new()));
            let state = RecorderState {
                headers: headers.clone(),
                nuc_data: nuc_data.clone(),
                case_data: case_data.clone(),
                quality_data: quality_data.clone(),
                extras: extras.clone(),
                end_base_counts: end_base_counts.clone(),
                end_nuc_skips: end_nuc_skips.clone(),
                eof_called: eof_called.clone(),
                event_order: event_order.clone(),
            };
            (
                Self {
                    headers,
                    nuc_data,
                    case_data,
                    quality_data,
                    extras,
                    end_base_counts,
                    end_nuc_skips,
                    eof_called,
                    event_order,
                },
                state,
            )
        }
    }

    #[derive(Clone)]
    struct RecorderState {
        headers: Rc<RefCell<Vec<String>>>,
        nuc_data: Rc<RefCell<Vec<u8>>>,
        case_data: Rc<RefCell<Vec<u8>>>,
        quality_data: Rc<RefCell<Vec<u8>>>,
        extras: Rc<RefCell<Vec<(u64, u8)>>>,
        end_base_counts: Rc<RefCell<Vec<u64>>>,
        end_nuc_skips: Rc<RefCell<Vec<u64>>>,
        eof_called: Rc<RefCell<bool>>,
        event_order: Rc<RefCell<Vec<char>>>,
    }

    impl Handler for EventRecorder {
        fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
            self.headers
                .borrow_mut()
                .push(String::from_utf8_lossy(header).to_string());
            self.nuc_data.borrow_mut().clear();
            self.case_data.borrow_mut().clear();
            self.quality_data.borrow_mut().clear();
            self.extras.borrow_mut().clear();
            self.event_order.borrow_mut().clear();
            Ok(())
        }
        fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
            self.nuc_data.borrow_mut().extend_from_slice(data);
            self.event_order.borrow_mut().push('N');
            Ok(())
        }
        fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
            self.case_data.borrow_mut().extend_from_slice(data);
            self.event_order.borrow_mut().push('C');
            Ok(())
        }
        fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
            self.extras.borrow_mut().push((position, ch));
            Ok(())
        }
        fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()> {
            self.quality_data.borrow_mut().extend_from_slice(data);
            Ok(())
        }
        fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
            self.end_base_counts.borrow_mut().push(info.base_count);
            self.end_nuc_skips.borrow_mut().push(info.nuc_skip);
            Ok(())
        }
        fn eof(&mut self) -> io::Result<()> {
            *self.eof_called.borrow_mut() = true;
            Ok(())
        }
    }

    /// Helper: pack bases into 2-bit format.
    fn pack_2bit(bases: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in bases.chunks(4) {
            if chunk.len() == 4 {
                out.push(try_pack_4_bases_2bit(chunk).unwrap());
            } else {
                // Partial byte: pack available bases, pad with 0
                let mut byte = 0u8;
                for (i, &b) in chunk.iter().enumerate() {
                    let code = crate::codec::common::TWO_BIT_ENCODE[b as usize];
                    byte |= code << (i * 2);
                }
                out.push(byte);
            }
        }
        out
    }

    /// Helper: pack bases into 4-bit format.
    fn pack_4bit(bases: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in bases.chunks(2) {
            if chunk.len() == 2 {
                out.push(try_pack_2_bases_4bit(chunk).unwrap());
            } else {
                let code = crate::codec::common::FOUR_BIT_ENCODE[chunk[0] as usize];
                out.push(code);
            }
        }
        out
    }

    #[test]
    fn test_slice_spec_parse_exact() {
        let spec = SliceSpec::parse("chr1").unwrap();
        assert!(matches!(spec.matcher, HeaderMatcher::Exact(ref s) if s == "chr1"));
        assert!(spec.start.is_none());
        assert!(spec.end.is_none());
    }

    #[test]
    fn test_slice_spec_parse_range() {
        let spec = SliceSpec::parse("chr1:100:200").unwrap();
        assert!(matches!(spec.matcher, HeaderMatcher::Exact(ref s) if s == "chr1"));
        assert_eq!(spec.start, Some(99)); // 1-based 100 -> 0-based 99
        assert_eq!(spec.end, Some(200)); // 1-based inclusive 200 -> 0-based exclusive 200
    }

    #[test]
    fn test_slice_spec_parse_regex() {
        let spec = SliceSpec::parse("/^chr[0-9]+/").unwrap();
        assert!(matches!(spec.matcher, HeaderMatcher::Regex(_)));
        assert!(spec.matches(b"chr1"));
        assert!(spec.matches(b"chr22"));
        assert!(!spec.matches(b"scaffold_1"));
    }

    #[test]
    fn test_slice_spec_parse_regex_with_range() {
        let spec = SliceSpec::parse("/^chr/:1:50").unwrap();
        assert!(spec.matches(b"chr1"));
        assert_eq!(spec.start, Some(0));
        assert_eq!(spec.end, Some(50));
    }

    #[test]
    fn test_slice_filter_pass_all() {
        let (recorder, state) = EventRecorder::new();
        let specs = vec![SliceSpec::parse("chr1").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::EightBit);

        // chr1 should pass
        filter.begin_record(b"chr1").unwrap();
        filter.nuc_chunk(b"ACGT").unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 4,
                ..Default::default()
            })
            .unwrap();

        // chr2 should be suppressed
        filter.begin_record(b"chr2").unwrap();
        filter.nuc_chunk(b"TTTT").unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 4,
                ..Default::default()
            })
            .unwrap();

        filter.eof().unwrap();

        assert_eq!(&*state.headers.borrow(), &["chr1"]);
        assert_eq!(&*state.nuc_data.borrow(), b"ACGT");
        assert_eq!(*state.end_base_counts.borrow(), vec![4]);
        assert!(*state.eof_called.borrow());
    }

    #[test]
    fn test_slice_filter_regex() {
        let (recorder, state) = EventRecorder::new();
        let specs = vec![SliceSpec::parse("/^chr[0-9]+/").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::EightBit);

        filter.begin_record(b"chr1").unwrap();
        filter.nuc_chunk(b"AA").unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 2,
                ..Default::default()
            })
            .unwrap();

        filter.begin_record(b"scaffold_1").unwrap();
        filter.nuc_chunk(b"CC").unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 2,
                ..Default::default()
            })
            .unwrap();

        filter.begin_record(b"chr22").unwrap();
        filter.nuc_chunk(b"GG").unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 2,
                ..Default::default()
            })
            .unwrap();

        filter.eof().unwrap();

        let headers = state.headers.borrow();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0], "chr1");
        assert_eq!(headers[1], "chr22");
    }

    #[test]
    fn test_slice_filter_sub_slice_quality() {
        let (recorder, state) = EventRecorder::new();
        // Slice bases 3-7 (1-based) = bytes 2..7 (0-based [2,7))
        let specs = vec![SliceSpec::parse("read1:3:7").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::EightBit);

        filter.begin_record(b"read1").unwrap();
        // Quality: 10 bytes
        filter.quality_chunk(b"ABCDEFGHIJ").unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 10,
                quality_bytes: 10,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // Should have quality bytes 2..7 = "CDEFG"
        assert_eq!(&*state.quality_data.borrow(), b"CDEFG");
        assert_eq!(*state.end_base_counts.borrow(), vec![5]);
    }

    #[test]
    fn test_slice_filter_sub_slice_extras() {
        let (recorder, state) = EventRecorder::new();
        // Slice bases 5-10 (1-based) = [4, 10) (0-based)
        let specs = vec![SliceSpec::parse("seq:5:10").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::EightBit);

        filter.begin_record(b"seq").unwrap();
        // Extras at positions 2, 6, 9, 15 (0-based)
        filter.extra(2, b'N').unwrap(); // outside range
        filter.extra(6, b'R').unwrap(); // inside -> adjusted to 2
        filter.extra(9, b'Y').unwrap(); // inside -> adjusted to 5
        filter.extra(15, b'N').unwrap(); // outside range
        filter
            .end_record(&RecordInfo {
                base_count: 20,
                extra_count: 4,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        let extras = state.extras.borrow();
        assert_eq!(extras.len(), 2);
        assert_eq!(extras[0], (2, b'R')); // 6 - 4 = 2
        assert_eq!(extras[1], (5, b'Y')); // 9 - 4 = 5
    }

    #[test]
    fn test_slice_filter_sub_slice_case() {
        let (recorder, state) = EventRecorder::new();
        // Slice bases 3-6 (1-based) = [2, 6) (0-based)
        let specs = vec![SliceSpec::parse("seq:3:6").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::EightBit);

        // Encode case for "AAaaBBbb" = [2, 2, 2, 2] (uppercase AA, lowercase aa, upper BB, lower bb)
        let mut enc = CaseEncoder::new();
        for _ in 0..2 {
            enc.push_bit(1);
        } // AA
        for _ in 0..2 {
            enc.push_bit(0);
        } // aa
        for _ in 0..2 {
            enc.push_bit(1);
        } // BB
        for _ in 0..2 {
            enc.push_bit(0);
        } // bb
        let rle = enc.finish().to_vec();

        filter.begin_record(b"seq").unwrap();
        filter.case_chunk(&rle).unwrap();
        filter.nuc_chunk(b"AACCGGTT").unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                case_bytes: rle.len() as u32,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // Bases [2,6) = positions 2,3,4,5 = "aaBB" (lowercase, lowercase, uppercase, uppercase)
        // Re-encoded as case RLE: [0, 2, 2]
        let case = state.case_data.borrow().clone();
        let mut dec = CaseDecoder::new(&case);
        assert!(!dec.next_is_upper()); // a (pos 2)
        assert!(!dec.next_is_upper()); // a (pos 3)
        assert!(dec.next_is_upper()); // B (pos 4)
        assert!(dec.next_is_upper()); // B (pos 5)
    }

    #[test]
    fn test_slice_filter_suppressed_record() {
        let (recorder, state) = EventRecorder::new();
        let specs = vec![SliceSpec::parse("wanted").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::EightBit);

        // Unwanted record
        filter.begin_record(b"unwanted").unwrap();
        filter.nuc_chunk(b"AAAA").unwrap();
        filter.case_chunk(&[4]).unwrap();
        filter.quality_chunk(b"IIII").unwrap();
        filter.extra(0, b'N').unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 4,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        assert!(state.headers.borrow().is_empty());
        assert!(state.nuc_data.borrow().is_empty());
        assert!(*state.eof_called.borrow());
    }

    #[test]
    fn test_slice_spec_parse_errors() {
        assert!(SliceSpec::parse("chr1:0:10").is_err()); // 0 is invalid
        assert!(SliceSpec::parse("chr1:10:5").is_err()); // start > end
        assert!(SliceSpec::parse("/unclosed").is_err()); // unclosed regex
    }

    #[test]
    fn test_slice_filter_sub_slice_nuc() {
        let (recorder, state) = EventRecorder::new();
        // Slice bytes [2, 5) from nucleotide data
        let specs = vec![SliceSpec::parse("seq:3:5").unwrap()]; // 1-based -> [2, 5)
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::EightBit);

        filter.begin_record(b"seq").unwrap();
        filter.nuc_chunk(b"ABCDEFGH").unwrap(); // 8 bytes
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 8,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // Should get bytes at positions 2,3,4 = "CDE"
        assert_eq!(&*state.nuc_data.borrow(), b"CDE");
    }

    // --- Packed nucleotide tests ---

    #[test]
    fn test_sub_slice_nuc_2bit() {
        // ACGTACGT = 8 bases, 2 bytes in 2-bit mode
        let bases = b"ACGTACGT";
        let packed = pack_2bit(bases);
        assert_eq!(packed.len(), 2);

        let (recorder, state) = EventRecorder::new();
        // Slice bases 2-6 (0-based [2,6)). 1-based = 3:6
        let specs = vec![SliceSpec::parse("seq:3:6").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter
            .record_meta(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // start=2, bpb=4, nuc_skip = 2 % 4 = 2
        // aligned_start = 2 - 2 = 0 (byte 0)
        // end = 6, ceil(6/4) = 2 bytes
        // Should pass both packed bytes to downstream
        let nuc = state.nuc_data.borrow().clone();
        assert_eq!(nuc.len(), 2); // both bytes needed

        // nuc_skip should be 2, base_count should be 2 + 4 = 6
        assert_eq!(*state.end_nuc_skips.borrow(), vec![2]);
        assert_eq!(*state.end_base_counts.borrow(), vec![6]);

        // Verify: unpack and skip first 2 bases -> should get GTAC
        let mut unpacked = Vec::new();
        unpack_bases(AlphabetMode::TwoBit, &nuc, 6, &mut unpacked);
        assert_eq!(&unpacked[2..], b"GTAC");
    }

    #[test]
    fn test_sub_slice_nuc_4bit() {
        // ACGA = 4 bases, 2 bytes in 4-bit mode
        let bases = b"ACGA";
        let packed = pack_4bit(bases);
        assert_eq!(packed.len(), 2);

        let (recorder, state) = EventRecorder::new();
        // Slice bases 1-3 (0-based [1,3)). 1-based = 2:3
        let specs = vec![SliceSpec::parse("seq:2:3").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::FourBit);

        filter.begin_record(b"seq").unwrap();
        filter
            .record_meta(&RecordInfo {
                base_count: 4,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 4,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // start=1, bpb=2, nuc_skip = 1 % 2 = 1
        // aligned_start = 1 - 1 = 0
        // end = 3, ceil(3/2) = 2 bytes
        let nuc = state.nuc_data.borrow().clone();
        assert_eq!(nuc.len(), 2);

        assert_eq!(*state.end_nuc_skips.borrow(), vec![1]);
        // base_count = nuc_skip + (end - start) = 1 + 2 = 3
        assert_eq!(*state.end_base_counts.borrow(), vec![3]);

        // Unpack and skip 1 -> should get CG
        let mut unpacked = Vec::new();
        unpack_bases(AlphabetMode::FourBit, &nuc, 3, &mut unpacked);
        assert_eq!(&unpacked[1..], b"CG");
    }

    #[test]
    fn test_sub_slice_nuc_2bit_byte_aligned() {
        // Slice bases 0-4 (byte-aligned, nuc_skip=0)
        let bases = b"ACGTACGT";
        let packed = pack_2bit(bases);

        let (recorder, state) = EventRecorder::new();
        let specs = vec![SliceSpec::parse("seq:1:4").unwrap()]; // 0-based [0, 4)
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // nuc_skip = 0 % 4 = 0, only need 1 byte (4 bases)
        let nuc = state.nuc_data.borrow().clone();
        assert_eq!(nuc.len(), 1);
        assert_eq!(*state.end_nuc_skips.borrow(), vec![0]);
        assert_eq!(*state.end_base_counts.borrow(), vec![4]);

        let mut unpacked = Vec::new();
        unpack_bases(AlphabetMode::TwoBit, &nuc, 4, &mut unpacked);
        assert_eq!(&unpacked, b"ACGT");
    }

    #[test]
    fn test_sub_slice_nuc_2bit_max_skip() {
        // Start at base 3 in 2-bit (nuc_skip=3, maximum possible)
        let bases = b"ACGTACGT";
        let packed = pack_2bit(bases);

        let (recorder, state) = EventRecorder::new();
        let specs = vec![SliceSpec::parse("seq:4:6").unwrap()]; // 0-based [3, 6)
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // nuc_skip = 3 % 4 = 3
        assert_eq!(*state.end_nuc_skips.borrow(), vec![3]);
        // base_count = 3 + 3 = 6
        assert_eq!(*state.end_base_counts.borrow(), vec![6]);

        let nuc = state.nuc_data.borrow().clone();
        // aligned_start = 3 - 3 = 0, end = 6, ceil(6/4) = 2 bytes
        assert_eq!(nuc.len(), 2);

        let mut unpacked = Vec::new();
        unpack_bases(AlphabetMode::TwoBit, &nuc, 6, &mut unpacked);
        assert_eq!(&unpacked[3..], b"TAC");
    }

    #[test]
    fn test_sub_slice_nuc_2bit_single_base() {
        // Slice exactly 1 base from the middle of a packed byte
        let bases = b"ACGT";
        let packed = pack_2bit(bases); // 1 byte

        let (recorder, state) = EventRecorder::new();
        // Slice base 2 only (0-based [1, 2)). 1-based = 2:2
        let specs = vec![SliceSpec::parse("seq:2:2").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 4,
                packed_bytes: 1,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // nuc_skip = 1 % 4 = 1
        assert_eq!(*state.end_nuc_skips.borrow(), vec![1]);
        // base_count = 1 + 1 = 2
        assert_eq!(*state.end_base_counts.borrow(), vec![2]);

        let nuc = state.nuc_data.borrow().clone();
        assert_eq!(nuc.len(), 1);

        let mut unpacked = Vec::new();
        unpack_bases(AlphabetMode::TwoBit, &nuc, 2, &mut unpacked);
        assert_eq!(unpacked[1], b'C'); // base at position 1 is C
    }

    #[test]
    fn test_sub_slice_nuc_2bit_with_case() {
        // 8 bases: ACGTacgt (upper 4, lower 4)
        let bases = b"ACGTACGT"; // packed uppercase
        let packed = pack_2bit(bases);

        // Case: 4 upper, 4 lower
        let mut case_enc = CaseEncoder::new();
        for _ in 0..4 {
            case_enc.push_bit(1);
        }
        for _ in 0..4 {
            case_enc.push_bit(0);
        }
        let case_rle = case_enc.finish().to_vec();

        let (recorder, state) = EventRecorder::new();
        // Slice bases [2, 6) (0-based). 1-based = 3:6
        let specs = vec![SliceSpec::parse("seq:3:6").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.case_chunk(&case_rle).unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // Case data should arrive before nuc data
        let order = state.event_order.borrow().clone();
        assert!(!order.is_empty());
        let first_c = order.iter().position(|&c| c == 'C');
        let first_n = order.iter().position(|&c| c == 'N');
        assert!(first_c.is_some() && first_n.is_some());
        assert!(
            first_c.unwrap() < first_n.unwrap(),
            "case must arrive before nuc"
        );

        // nuc_skip = 2, so case should have 2 padding uppercase bits + 4 sliced bits
        // Sliced case: positions 2,3 = upper, positions 4,5 = lower
        // Total case: [upper, upper] (padding) + [upper, upper, lower, lower]
        let case = state.case_data.borrow().clone();
        let mut dec = CaseDecoder::new(&case);
        assert!(dec.next_is_upper()); // padding
        assert!(dec.next_is_upper()); // padding
        assert!(dec.next_is_upper()); // pos 2 (G) - uppercase
        assert!(dec.next_is_upper()); // pos 3 (T) - uppercase
        assert!(!dec.next_is_upper()); // pos 4 (a) - lowercase
        assert!(!dec.next_is_upper()); // pos 5 (c) - lowercase
    }

    #[test]
    fn test_sub_slice_nuc_2bit_with_extras() {
        let bases = b"ACGTACGT";
        let packed = pack_2bit(bases);

        let (recorder, state) = EventRecorder::new();
        // Slice [2, 6). 1-based = 3:6
        let specs = vec![SliceSpec::parse("seq:3:6").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.extra(1, b'N').unwrap(); // outside [2,6)
        filter.extra(3, b'R').unwrap(); // inside: adjusted to 3 - 2 + 2(nuc_skip) = 3
        filter.extra(5, b'Y').unwrap(); // inside: adjusted to 5 - 2 + 2 = 5
        filter.extra(7, b'N').unwrap(); // outside
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        let extras = state.extras.borrow();
        assert_eq!(extras.len(), 2);
        assert_eq!(extras[0], (3, b'R')); // (3 - 2) + 2 = 3
        assert_eq!(extras[1], (5, b'Y')); // (5 - 2) + 2 = 5
    }

    #[test]
    fn test_sub_slice_nuc_2bit_multi_chunk() {
        // 16 bases across 2 nuc_chunks of 4 bytes (8 bases) each
        let bases1 = b"ACGTACGT";
        let bases2 = b"TGCATGCA";
        let packed1 = pack_2bit(bases1);
        let packed2 = pack_2bit(bases2);

        let (recorder, state) = EventRecorder::new();
        // Slice [6, 10). 1-based = 7:10
        let specs = vec![SliceSpec::parse("seq:7:10").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.nuc_chunk(&packed1).unwrap(); // bases 0-7
        filter.nuc_chunk(&packed2).unwrap(); // bases 8-15
        filter
            .end_record(&RecordInfo {
                base_count: 16,
                packed_bytes: 4,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // nuc_skip = 6 % 4 = 2
        assert_eq!(*state.end_nuc_skips.borrow(), vec![2]);
        // base_count = 2 + 4 = 6
        assert_eq!(*state.end_base_counts.borrow(), vec![6]);

        let nuc = state.nuc_data.borrow().clone();
        let mut unpacked = Vec::new();
        unpack_bases(AlphabetMode::TwoBit, &nuc, 6, &mut unpacked);
        // Skip 2: remaining = bases 6,7,8,9 = GT (from first) + TG (from second)
        assert_eq!(&unpacked[2..], b"GTTG");
    }

    #[test]
    fn test_sub_slice_nuc_2bit_end_of_sequence() {
        // 6 bases -> 2 bytes (last byte has 2 unused bits)
        let bases = b"ACGTAC";
        let packed = pack_2bit(bases);
        assert_eq!(packed.len(), 2);

        let (recorder, state) = EventRecorder::new();
        // Slice [4, 6). 1-based = 5:6
        let specs = vec![SliceSpec::parse("seq:5:6").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter
            .record_meta(&RecordInfo {
                base_count: 6,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 6,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // start=4, nuc_skip = 4 % 4 = 0, byte-aligned
        assert_eq!(*state.end_nuc_skips.borrow(), vec![0]);
        assert_eq!(*state.end_base_counts.borrow(), vec![2]);

        let nuc = state.nuc_data.borrow().clone();
        assert_eq!(nuc.len(), 1); // only the second packed byte
        let mut unpacked = Vec::new();
        unpack_bases(AlphabetMode::TwoBit, &nuc, 2, &mut unpacked);
        assert_eq!(&unpacked, b"AC");
    }

    #[test]
    fn test_sub_slice_nuc_2bit_entire_sequence() {
        let bases = b"ACGTACGT";
        let packed = pack_2bit(bases);

        let (recorder, state) = EventRecorder::new();
        // Slice entire sequence [0, 8). 1-based = 1:8
        let specs = vec![SliceSpec::parse("seq:1:8").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        assert_eq!(*state.end_nuc_skips.borrow(), vec![0]);
        assert_eq!(*state.end_base_counts.borrow(), vec![8]);
        assert_eq!(&*state.nuc_data.borrow(), &packed);
    }

    #[test]
    fn test_sub_slice_nuc_with_quality() {
        // FASTQ-like: packed nuc + quality
        let bases = b"ACGTACGT";
        let packed = pack_2bit(bases);

        let (recorder, state) = EventRecorder::new();
        // Slice [2, 6). 1-based = 3:6
        let specs = vec![SliceSpec::parse("seq:3:6").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.nuc_chunk(&packed).unwrap();
        filter.quality_chunk(b"ABCDEFGH").unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                quality_bytes: 8,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // Quality slicing is byte-based (1 byte = 1 base), unchanged
        assert_eq!(&*state.quality_data.borrow(), b"CDEF");

        // Nuc should have nuc_skip = 2
        assert_eq!(*state.end_nuc_skips.borrow(), vec![2]);
    }

    #[test]
    fn test_sub_slice_nuc_2bit_case_and_extras() {
        // Full integration: 2-bit packed nuc + case + extras
        let bases = b"ACGTACGT";
        let packed = pack_2bit(bases);

        // Case: all uppercase
        let mut case_enc = CaseEncoder::new();
        for _ in 0..8 {
            case_enc.push_bit(1);
        }
        let case_rle = case_enc.finish().to_vec();

        let (recorder, state) = EventRecorder::new();
        // Slice [1, 5). 1-based = 2:5
        let specs = vec![SliceSpec::parse("seq:2:5").unwrap()];
        let mut filter = SliceFilter::new(Box::new(recorder), specs, AlphabetMode::TwoBit);

        filter.begin_record(b"seq").unwrap();
        filter.case_chunk(&case_rle).unwrap();
        filter.extra(2, b'N').unwrap(); // inside [1,5), adjusted to (2-1)+1 = 2
        filter.nuc_chunk(&packed).unwrap();
        filter
            .end_record(&RecordInfo {
                base_count: 8,
                packed_bytes: 2,
                ..Default::default()
            })
            .unwrap();
        filter.eof().unwrap();

        // nuc_skip = 1 % 4 = 1
        assert_eq!(*state.end_nuc_skips.borrow(), vec![1]);
        // base_count = 1 + 4 = 5
        assert_eq!(*state.end_base_counts.borrow(), vec![5]);

        // Extras adjusted by +nuc_skip
        let extras = state.extras.borrow();
        assert_eq!(extras.len(), 1);
        assert_eq!(extras[0], (2, b'N')); // (2 - 1) + 1 = 2

        // Case should arrive before nuc
        let order = state.event_order.borrow();
        let first_c = order.iter().position(|&c| c == 'C');
        let first_n = order.iter().position(|&c| c == 'N');
        assert!(first_c.unwrap() < first_n.unwrap());

        // Case: 1 padding upper + 4 sliced uppers = 5 uppercase bits
        let case = state.case_data.borrow().clone();
        let mut dec = CaseDecoder::new(&case);
        for _ in 0..5 {
            assert!(dec.next_is_upper());
        }
    }
}
