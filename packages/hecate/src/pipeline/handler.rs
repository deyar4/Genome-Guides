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

use std::io;

/// Metadata about a completed record.
#[derive(Clone, Debug, Default)]
pub struct RecordInfo {
    pub base_count: u64,
    pub packed_bytes: u64,
    pub extra_count: u32,
    pub case_bytes: u32,
    pub quality_bytes: u64,
    /// Number of padding bases prepended to packed nuc data by SliceFilter.
    /// Downstream sinks must skip these bases from output.
    pub nuc_skip: u64,
}

/// Push-based handler for streaming genomic data.
pub trait Handler {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()>;
    /// Called after begin_record with record metadata (base_count, etc.)
    /// before any data events. Default is a no-op.
    fn record_meta(&mut self, _info: &RecordInfo) -> io::Result<()> {
        Ok(())
    }
    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()>;
    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()>;
    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()>;
    fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()>;
    fn end_record(&mut self, info: &RecordInfo) -> io::Result<()>;
    fn eof(&mut self) -> io::Result<()>;

    /// Whether this handler needs stream data (NUC, CASE, EXTRA, QUALITY).
    /// Return false for metadata-only handlers (e.g. listing).
    /// The source driver will skip reading data streams entirely, and
    /// the mux reader will skip decompressing those streams.
    fn needs_stream_data(&self) -> bool {
        true
    }
}

impl<T: Handler + ?Sized> Handler for Box<T> {
    fn begin_record(&mut self, header: &[u8]) -> io::Result<()> {
        (**self).begin_record(header)
    }
    fn record_meta(&mut self, info: &RecordInfo) -> io::Result<()> {
        (**self).record_meta(info)
    }
    fn nuc_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        (**self).nuc_chunk(data)
    }
    fn case_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        (**self).case_chunk(data)
    }
    fn extra(&mut self, position: u64, ch: u8) -> io::Result<()> {
        (**self).extra(position, ch)
    }
    fn quality_chunk(&mut self, data: &[u8]) -> io::Result<()> {
        (**self).quality_chunk(data)
    }
    fn end_record(&mut self, info: &RecordInfo) -> io::Result<()> {
        (**self).end_record(info)
    }
    fn eof(&mut self) -> io::Result<()> {
        (**self).eof()
    }
    fn needs_stream_data(&self) -> bool {
        (**self).needs_stream_data()
    }
}
