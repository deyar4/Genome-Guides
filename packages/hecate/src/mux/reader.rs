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

use crate::compress::traits::Decompressor;
use crate::stream::traits::StreamRead;
use std::io;

/// Streaming demultiplexer: reads blocks sequentially, buffers per-stream data.
pub struct StreamingMuxReader<R: StreamRead> {
    input: R,
    stream_count: usize,
    decompressors: Vec<Option<Box<dyn Decompressor>>>,
    channel_buffers: Vec<Vec<u8>>,
    channel_positions: Vec<usize>,
    eof: bool,
    read_buf: Vec<u8>,
    compressed_sizes: Vec<u64>,
    uncompressed_sizes: Vec<u64>,
    indexed: bool,
    /// Bitmask of streams to skip during block decompression.
    /// Skipped streams are read from the input but not decompressed.
    /// Must be set before reading begins and not changed afterwards.
    skip_mask: u32,
}

impl<R: StreamRead> StreamingMuxReader<R> {
    pub fn new(
        input: R,
        stream_count: usize,
        decompressors: Vec<Option<Box<dyn Decompressor>>>,
        indexed: bool,
    ) -> Self {
        assert_eq!(decompressors.len(), stream_count);
        let channel_buffers = (0..stream_count).map(|_| Vec::new()).collect();
        let channel_positions = vec![0; stream_count];
        Self {
            input,
            stream_count,
            decompressors,
            channel_buffers,
            channel_positions,
            eof: false,
            read_buf: Vec::new(),
            compressed_sizes: Vec::with_capacity(stream_count),
            uncompressed_sizes: Vec::with_capacity(stream_count),
            indexed,
            skip_mask: 0,
        }
    }

    fn available(&self, stream_id: usize) -> usize {
        self.channel_buffers[stream_id].len() - self.channel_positions[stream_id]
    }

    /// Read from a specific stream. Pumps blocks as needed.
    pub fn read_stream(&mut self, stream_id: usize, buf: &mut [u8]) -> io::Result<usize> {
        while self.available(stream_id) < buf.len() && !self.eof {
            self.pump_next_block()?;
        }

        let avail = self.available(stream_id);
        let n = buf.len().min(avail);
        if n > 0 {
            let pos = self.channel_positions[stream_id];
            buf[..n].copy_from_slice(&self.channel_buffers[stream_id][pos..pos + n]);
            self.channel_positions[stream_id] += n;

            // Fast clear when fully consumed (O(1), no memmove)
            if self.channel_positions[stream_id] == self.channel_buffers[stream_id].len() {
                self.channel_buffers[stream_id].clear();
                self.channel_positions[stream_id] = 0;
            }
        }

        Ok(n)
    }

    /// Read exactly `len` bytes from a stream, or fail.
    pub fn read_stream_exact(&mut self, stream_id: usize, buf: &mut [u8]) -> io::Result<()> {
        let mut offset = 0;
        while offset < buf.len() {
            let n = self.read_stream(stream_id, &mut buf[offset..])?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "unexpected EOF reading stream",
                ));
            }
            offset += n;
        }
        Ok(())
    }

    /// Read a u8 from a specific stream.
    pub fn read_stream_u8(&mut self, stream_id: usize) -> io::Result<u8> {
        let mut buf = [0u8; 1];
        self.read_stream_exact(stream_id, &mut buf)?;
        Ok(buf[0])
    }

    /// Read a u32le from a specific stream.
    pub fn read_stream_u32le(&mut self, stream_id: usize) -> io::Result<u32> {
        let mut buf = [0u8; 4];
        self.read_stream_exact(stream_id, &mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    /// Read a u64le from a specific stream.
    pub fn read_stream_u64le(&mut self, stream_id: usize) -> io::Result<u64> {
        let mut buf = [0u8; 8];
        self.read_stream_exact(stream_id, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }

    /// Ensure at least `len` bytes are available for stream `stream_id`.
    /// Pumps blocks as needed. Returns actual available bytes.
    pub fn ensure_available(&mut self, stream_id: usize, len: usize) -> io::Result<usize> {
        while self.available(stream_id) < len && !self.eof {
            self.pump_next_block()?;
        }
        Ok(self.available(stream_id))
    }

    /// Borrow available data from a stream (zero-copy).
    pub fn borrow_stream(&self, stream_id: usize) -> &[u8] {
        let pos = self.channel_positions[stream_id];
        &self.channel_buffers[stream_id][pos..]
    }

    /// Advance the position of a stream after borrowing.
    pub fn advance_stream(&mut self, stream_id: usize, n: usize) {
        self.channel_positions[stream_id] += n;
        // Fast clear when fully consumed
        if self.channel_positions[stream_id] == self.channel_buffers[stream_id].len() {
            self.channel_buffers[stream_id].clear();
            self.channel_positions[stream_id] = 0;
        }
    }

    #[allow(clippy::uninit_vec)]
    fn pump_next_block(&mut self) -> io::Result<()> {
        // Try to read the block header. If we get EOF on the first byte, set eof flag.
        let mut flags_buf = [0u8; 1];
        let n = self.input.read(&mut flags_buf)?;
        if n == 0 {
            self.eof = true;
            return Ok(());
        }
        let flags = flags_buf[0];

        // Read compressed and uncompressed sizes (reuse allocations)
        self.compressed_sizes.clear();
        self.uncompressed_sizes.clear();
        for _ in 0..self.stream_count {
            let mut buf = [0u8; 8];
            self.input.read_exact(&mut buf)?;
            self.compressed_sizes.push(u64::from_le_bytes(buf));
        }
        for _ in 0..self.stream_count {
            let mut buf = [0u8; 8];
            self.input.read_exact(&mut buf)?;
            self.uncompressed_sizes.push(u64::from_le_bytes(buf));
        }
        let mut crc_buf = [0u8; 4];
        self.input.read_exact(&mut crc_buf)?;
        let expected_crc = u32::from_le_bytes(crc_buf);

        // Pre-size buffers to the sizes we'll need this block. This eliminates
        // mid-decompress reallocations whose first-touch zero-fill (THP-backed)
        // shows up as `clear_page_erms` in the kernel profile.
        let max_comp = self.compressed_sizes.iter().copied().max().unwrap_or(0) as usize;
        if max_comp > self.read_buf.capacity() {
            self.read_buf.reserve(max_comp - self.read_buf.len());
        }
        for s in 0..self.stream_count {
            let need = self.uncompressed_sizes[s] as usize;
            if need > 0 {
                let buf = &mut self.channel_buffers[s];
                let pos = self.channel_positions[s];
                let live = buf.len() - pos;
                let want = live + need;
                if want > buf.capacity() {
                    buf.reserve(want - buf.len());
                }
            }
        }

        // In indexed mode each block is independently compressed (the writer
        // calls finish()+reset() per block), so mirror that by resetting
        // decompressors before decompressing each block.  This is required
        // for codecs like Markov where the model state is external to the
        // frame data.  For Zstd this is harmless since its frames are
        // self-delimited and reinit() between them is a no-op.
        if self.indexed {
            for s in 0..self.stream_count {
                if let Some(decomp) = &mut self.decompressors[s] {
                    decomp.reset();
                }
            }
        }

        // Read and decompress each stream's data, validate CRC.
        // In indexed mode: CRC covers uncompressed (decompressed) data.
        // In non-indexed mode: CRC covers stored (compressed) data, since
        // streaming frames don't produce per-block-aligned decompressed output.
        // When skip_mask is set, CRC validation is disabled because we may
        // not decompress all streams needed for the checksum.
        let validate_crc = self.skip_mask == 0;
        let mut crc_val = 0u32;
        for s in 0..self.stream_count {
            let comp_size = self.compressed_sizes[s] as usize;
            let is_raw = flags & (1 << s) != 0;
            let skip = (self.skip_mask >> s) & 1 != 0;

            // Skip only when there is truly nothing to do: no data AND either
            // the stream is raw, indexed (independent frames), or skipped.
            // In non-indexed streaming mode, we must still call the decompressor
            // even with zero bytes so it can maintain frame state -- unless skipped.
            if comp_size == 0 && (is_raw || self.indexed || skip) {
                continue;
            }

            // Skipped stream: consume bytes from input without decompressing.
            if skip {
                if comp_size > 0 {
                    self.read_buf.clear();
                    self.read_buf.reserve(comp_size);
                    // SAFETY: reserve guarantees capacity, read_exact fills all bytes.
                    unsafe {
                        self.read_buf.set_len(comp_size);
                    }
                    if let Err(e) = self.input.read_exact(&mut self.read_buf) {
                        unsafe {
                            self.read_buf.set_len(0);
                        }
                        return Err(e);
                    }
                }
                continue;
            }

            // Compact consumed prefix before appending (once per block per stream)
            let pos = self.channel_positions[s];
            if pos > 0 {
                let buf = &mut self.channel_buffers[s];
                let remaining = buf.len() - pos;
                buf.copy_within(pos.., 0);
                buf.truncate(remaining);
                self.channel_positions[s] = 0;
            }

            if comp_size == 0 {
                // Non-indexed compressed stream with zero stored bytes: feed
                // empty input to the decompressor to maintain frame state.
                if let Some(decomp) = &mut self.decompressors[s] {
                    decomp.decompress(&[], &mut self.channel_buffers[s])?;
                }
                continue;
            }

            if is_raw {
                // Read directly into channel_buffers tail (no intermediary, no zeroing)
                let old_len = self.channel_buffers[s].len();
                self.channel_buffers[s].reserve(comp_size);
                // SAFETY: reserve guarantees capacity >= old_len + comp_size.
                // read_exact fills all bytes or returns Err (we restore len on Err).
                unsafe {
                    self.channel_buffers[s].set_len(old_len + comp_size);
                }
                if let Err(e) = self
                    .input
                    .read_exact(&mut self.channel_buffers[s][old_len..])
                {
                    unsafe {
                        self.channel_buffers[s].set_len(old_len);
                    }
                    return Err(e);
                }
                // Raw data is both stored and uncompressed -- CRC is the same either way.
                if validate_crc {
                    crc_val = crc32c::crc32c_append(crc_val, &self.channel_buffers[s][old_len..]);
                }
            } else {
                // Compressed: need read_buf for contiguous input to decompressor
                self.read_buf.clear();
                self.read_buf.reserve(comp_size);
                // SAFETY: same pattern -- reserve guarantees capacity, read_exact fills all bytes.
                unsafe {
                    self.read_buf.set_len(comp_size);
                }
                if let Err(e) = self.input.read_exact(&mut self.read_buf) {
                    unsafe {
                        self.read_buf.set_len(0);
                    }
                    return Err(e);
                }

                if self.indexed {
                    // Indexed: decompress first, CRC over decompressed data.
                    let old_len = self.channel_buffers[s].len();
                    if let Some(decomp) = &mut self.decompressors[s] {
                        decomp.decompress(&self.read_buf, &mut self.channel_buffers[s])?;
                    } else {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("stream {} has compressed data but no decompressor", s),
                        ));
                    }
                    if validate_crc {
                        crc_val =
                            crc32c::crc32c_append(crc_val, &self.channel_buffers[s][old_len..]);
                    }
                } else {
                    // Non-indexed: CRC over compressed (stored) data, then decompress.
                    if validate_crc {
                        crc_val = crc32c::crc32c_append(crc_val, &self.read_buf);
                    }
                    if let Some(decomp) = &mut self.decompressors[s] {
                        decomp.decompress(&self.read_buf, &mut self.channel_buffers[s])?;
                    } else {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("stream {} has compressed data but no decompressor", s),
                        ));
                    }
                }
            }
        }

        if validate_crc && crc_val != expected_crc {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block CRC32C mismatch: expected 0x{:08X}, got 0x{:08X}",
                    expected_crc, crc_val
                ),
            ));
        }

        Ok(())
    }

    pub fn is_eof(&self) -> bool {
        self.eof
    }

    /// Set a bitmask of streams to skip during block decompression.
    /// Skipped streams are consumed from the input but not decompressed
    /// or buffered, saving CPU and memory. Must be set before reading
    /// begins. Skipped streams must never be read from.
    pub fn set_skip_mask(&mut self, mask: u32) {
        self.skip_mask = mask;
    }

    pub fn into_inner(self) -> R {
        self.input
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compress::traits::Compressor;
    use crate::compress::zstd_codec::{ZstdCompressor, ZstdDecompressor};
    use crate::mux::block::BlockHeader;
    use crate::mux::writer::MuxWriter;
    use crate::stream::traits::StreamWrite;

    struct VecOutput {
        data: Vec<u8>,
    }

    impl VecOutput {
        fn new() -> Self {
            Self { data: Vec::new() }
        }
    }

    impl StreamWrite for VecOutput {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct SliceInput<'a> {
        data: &'a [u8],
        pos: usize,
    }

    impl<'a> SliceInput<'a> {
        fn new(data: &'a [u8]) -> Self {
            Self { data, pos: 0 }
        }
    }

    impl<'a> StreamRead for SliceInput<'a> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let avail = self.data.len() - self.pos;
            let n = buf.len().min(avail);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    fn make_raw_compressors(n: usize) -> Vec<Option<Box<dyn Compressor>>> {
        (0..n).map(|_| None).collect()
    }

    fn make_raw_decompressors(n: usize) -> Vec<Option<Box<dyn Decompressor>>> {
        (0..n).map(|_| None).collect()
    }

    fn make_zstd_compressors(n: usize) -> Vec<Option<Box<dyn Compressor>>> {
        (0..n)
            .map(|_| Some(Box::new(ZstdCompressor::new(1, 0, 1)) as Box<dyn Compressor>))
            .collect()
    }

    fn make_zstd_decompressors(n: usize) -> Vec<Option<Box<dyn Decompressor>>> {
        (0..n)
            .map(|_| Some(Box::new(ZstdDecompressor::new()) as Box<dyn Decompressor>))
            .collect()
    }

    #[test]
    fn test_single_stream_raw_round_trip() {
        let n_streams = 1;
        let mut out = VecOutput::new();
        let budget = 1024;

        {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                None,
                0,
            );
            mux.write_to_stream(0, b"Hello, World!").unwrap();
            let _ = mux.finalize().unwrap();
        }

        let mut reader = StreamingMuxReader::new(
            SliceInput::new(&out.data),
            n_streams,
            make_raw_decompressors(n_streams),
            false,
        );
        let mut buf = vec![0u8; 100];
        let n = reader.read_stream(0, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"Hello, World!");
    }

    #[test]
    fn test_multi_stream_raw_round_trip() {
        let n_streams = 5;
        let budget = 256;
        let mut out = VecOutput::new();

        {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                Some(crate::index::IndexTier::BlockOnly),
                0,
            );
            mux.write_to_stream(0, b"CTRL DATA").unwrap();
            mux.write_to_stream(1, b"NUC DATA HERE").unwrap();
            mux.write_to_stream(2, b"CASE").unwrap();
            mux.write_to_stream(3, b"HEADER INFO").unwrap();
            mux.write_to_stream(4, b"EXTRA").unwrap();
            let _ = mux.finalize().unwrap();
        }

        let mut reader = StreamingMuxReader::new(
            SliceInput::new(&out.data),
            n_streams,
            make_raw_decompressors(n_streams),
            true,
        );

        let expected = [
            b"CTRL DATA".to_vec(),
            b"NUC DATA HERE".to_vec(),
            b"CASE".to_vec(),
            b"HEADER INFO".to_vec(),
            b"EXTRA".to_vec(),
        ];

        for (s, exp) in expected.iter().enumerate() {
            let mut buf = vec![0u8; 100];
            let n = reader.read_stream(s, &mut buf).unwrap();
            assert_eq!(&buf[..n], exp.as_slice(), "stream {}", s);
        }
    }

    #[test]
    fn test_block_boundaries() {
        let n_streams = 2;
        let budget = 100;
        let mut out = VecOutput::new();

        {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                Some(crate::index::IndexTier::BlockOnly),
                0,
            );
            // Write exactly budget bytes total
            mux.write_to_stream(0, &vec![b'A'; 50]).unwrap();
            mux.write_to_stream(1, &vec![b'B'; 50]).unwrap();
            assert_eq!(mux.block_count(), 1);

            // Write one more byte to trigger second block
            mux.write_to_stream(0, &vec![b'C'; 1]).unwrap();
            let _ = mux.finalize().unwrap();
        }

        let mut reader = StreamingMuxReader::new(
            SliceInput::new(&out.data),
            n_streams,
            make_raw_decompressors(n_streams),
            true,
        );
        let mut buf = vec![0u8; 200];
        let n = reader.read_stream(0, &mut buf).unwrap();
        assert_eq!(n, 51);
        assert_eq!(&buf[..50], &vec![b'A'; 50]);
        assert_eq!(buf[50], b'C');
    }

    #[test]
    fn test_crc_corruption() {
        let n_streams = 1;
        let budget = 1024;
        let mut out = VecOutput::new();

        {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                None,
                0,
            );
            mux.write_to_stream(0, b"test data for crc").unwrap();
            let _ = mux.finalize().unwrap();
        }

        // Corrupt one byte in the data payload
        let data_start = BlockHeader::header_size(n_streams);
        if data_start < out.data.len() {
            out.data[data_start] ^= 0xFF;
        }

        let mut reader = StreamingMuxReader::new(
            SliceInput::new(&out.data),
            n_streams,
            make_raw_decompressors(n_streams),
            false,
        );
        let mut buf = vec![0u8; 100];
        let result = reader.read_stream(0, &mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_streams() {
        let n_streams = 5;
        let budget = 1024;
        let mut out = VecOutput::new();

        {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                None,
                0,
            );
            // Only write to streams 0, 1, 3
            mux.write_to_stream(0, b"ctrl").unwrap();
            mux.write_to_stream(1, b"nuc").unwrap();
            mux.write_to_stream(3, b"hdr").unwrap();
            let _ = mux.finalize().unwrap();
        }

        let mut reader = StreamingMuxReader::new(
            SliceInput::new(&out.data),
            n_streams,
            make_raw_decompressors(n_streams),
            false,
        );
        let mut buf = vec![0u8; 100];

        // Read from streams with data
        let n = reader.read_stream(0, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"ctrl");

        let n = reader.read_stream(1, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"nuc");

        // Stream 2 should be empty
        let n = reader.read_stream(2, &mut buf).unwrap();
        assert_eq!(n, 0);

        let n = reader.read_stream(3, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"hdr");

        let n = reader.read_stream(4, &mut buf).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn test_zstd_compressed_round_trip() {
        let n_streams = 3;
        let budget = 1024;
        let mut out = VecOutput::new();

        {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_zstd_compressors(n_streams),
                vec![None; n_streams],
                Some(crate::index::IndexTier::BlockOnly),
                0,
            );
            let data0 = vec![b'A'; 500];
            let data1 = vec![b'C'; 300];
            let data2 = vec![b'G'; 200];
            mux.write_to_stream(0, &data0).unwrap();
            mux.write_to_stream(1, &data1).unwrap();
            mux.write_to_stream(2, &data2).unwrap();
            let _ = mux.finalize().unwrap();
        }

        let mut reader = StreamingMuxReader::new(
            SliceInput::new(&out.data),
            n_streams,
            make_zstd_decompressors(n_streams),
            true,
        );

        let mut buf = vec![0u8; 600];
        let n = reader.read_stream(0, &mut buf).unwrap();
        assert_eq!(n, 500);
        assert!(buf[..n].iter().all(|&b| b == b'A'));

        let n = reader.read_stream(1, &mut buf).unwrap();
        assert_eq!(n, 300);
        assert!(buf[..n].iter().all(|&b| b == b'C'));

        let n = reader.read_stream(2, &mut buf).unwrap();
        assert_eq!(n, 200);
        assert!(buf[..n].iter().all(|&b| b == b'G'));
    }

    #[test]
    fn test_large_data_multi_blocks() {
        let n_streams = 3;
        let budget = 4096;
        let mut out = VecOutput::new();

        // Generate unique data for each stream
        let data0: Vec<u8> = (0..50000u32).map(|i| (i % 4) as u8 + b'A').collect();
        let data1: Vec<u8> = (0..30000u32).map(|i| ((i * 7) % 4) as u8 + b'a').collect();
        let data2: Vec<u8> = (0..20000u32)
            .map(|i| ((i * 13) % 10) as u8 + b'0')
            .collect();

        {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                Some(crate::index::IndexTier::BlockOnly),
                0,
            );
            // Write in chunks to simulate real usage
            for chunk in data0.chunks(1000) {
                mux.write_to_stream(0, chunk).unwrap();
            }
            for chunk in data1.chunks(1000) {
                mux.write_to_stream(1, chunk).unwrap();
            }
            for chunk in data2.chunks(1000) {
                mux.write_to_stream(2, chunk).unwrap();
            }
            let _ = mux.finalize().unwrap();
        }

        let mut reader = StreamingMuxReader::new(
            SliceInput::new(&out.data),
            n_streams,
            make_raw_decompressors(n_streams),
            true,
        );

        let mut result0 = Vec::new();
        let mut result1 = Vec::new();
        let mut result2 = Vec::new();
        let mut buf = vec![0u8; 8192];

        loop {
            let n = reader.read_stream(0, &mut buf).unwrap();
            if n == 0 {
                break;
            }
            result0.extend_from_slice(&buf[..n]);
        }
        loop {
            let n = reader.read_stream(1, &mut buf).unwrap();
            if n == 0 {
                break;
            }
            result1.extend_from_slice(&buf[..n]);
        }
        loop {
            let n = reader.read_stream(2, &mut buf).unwrap();
            if n == 0 {
                break;
            }
            result2.extend_from_slice(&buf[..n]);
        }

        assert_eq!(result0, data0);
        assert_eq!(result1, data1);
        assert_eq!(result2, data2);
    }

    #[test]
    fn test_incompressible_raw_fallback() {
        let n_streams = 1;
        let budget = 1024;
        let mut out = VecOutput::new();

        // Pseudo-random data that won't compress
        let mut data = Vec::with_capacity(500);
        let mut x: u32 = 999;
        for _ in 0..500 {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((x >> 16) as u8);
        }

        {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_zstd_compressors(n_streams),
                vec![None; n_streams],
                Some(crate::index::IndexTier::BlockOnly),
                0,
            );
            mux.write_to_stream(0, &data).unwrap();
            let _ = mux.finalize().unwrap();
        }

        let mut reader = StreamingMuxReader::new(
            SliceInput::new(&out.data),
            n_streams,
            make_zstd_decompressors(n_streams),
            true,
        );
        let mut result = vec![0u8; 600];
        let n = reader.read_stream(0, &mut result).unwrap();
        assert_eq!(&result[..n], data.as_slice());
    }
}
