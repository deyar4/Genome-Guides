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

use super::traits::{BorrowRead, StreamRead, StreamSeek, StreamWrite};
use std::io;

pub const DEFAULT_BUF_SIZE: usize = 65536; // 64 KiB

/// Buffered reader that implements both `StreamRead` and `BorrowRead`.
pub struct BufferedReader<S: StreamRead> {
    inner: S,
    buf: Vec<u8>,
    pos: usize,
    len: usize,
    capacity: usize,
}

impl<S: StreamRead> BufferedReader<S> {
    pub fn new(inner: S) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    pub fn with_capacity(capacity: usize, inner: S) -> Self {
        let capacity = capacity.max(4096);
        let buf = vec![0; capacity];
        Self {
            inner,
            buf,
            pos: 0,
            len: 0,
            capacity,
        }
    }

    pub fn into_inner(self) -> S {
        self.inner
    }

    fn compact(&mut self) {
        if self.pos > 0 {
            self.buf.copy_within(self.pos..self.len, 0);
            self.len -= self.pos;
            self.pos = 0;
        }
    }
}

impl<S: StreamRead> StreamRead for BufferedReader<S> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        if dst.is_empty() {
            return Ok(0);
        }

        // If buffer has data, return from buffer
        if self.pos < self.len {
            let avail = self.len - self.pos;
            let n = avail.min(dst.len());
            dst[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }

        // Buffer empty - if request is larger than buffer, bypass
        if dst.len() >= self.capacity {
            return self.inner.read(dst);
        }

        // Refill buffer
        self.pos = 0;
        self.len = 0;
        let n = self.inner.read(&mut self.buf[..self.capacity])?;
        self.len = n;
        if n == 0 {
            return Ok(0);
        }

        let to_copy = n.min(dst.len());
        dst[..to_copy].copy_from_slice(&self.buf[..to_copy]);
        self.pos = to_copy;
        Ok(to_copy)
    }
}

impl<S: StreamRead> BorrowRead for BufferedReader<S> {
    fn fill_buf(&mut self, min_bytes: usize) -> io::Result<&[u8]> {
        // If we already have enough, return immediately
        let available = self.len - self.pos;
        if available >= min_bytes {
            return Ok(&self.buf[self.pos..self.len]);
        }

        // Compact to make room
        self.compact();

        // Grow buffer if needed
        if min_bytes > self.buf.len() {
            self.buf.resize(min_bytes, 0);
            self.capacity = self.buf.len();
        }

        // Read until we have min_bytes or hit EOF
        while self.len < min_bytes {
            let n = self.inner.read(&mut self.buf[self.len..self.capacity])?;
            if n == 0 {
                break; // EOF
            }
            self.len += n;
        }

        Ok(&self.buf[self.pos..self.len])
    }

    fn consume(&mut self, n: usize) {
        self.pos += n;
        debug_assert!(self.pos <= self.len);
    }
}

impl<S: StreamRead + StreamSeek> StreamSeek for BufferedReader<S> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        // Account for buffered data when seeking
        let buffered_remaining = (self.len - self.pos) as i64;
        if buffered_remaining > 0 {
            self.inner
                .seek(io::SeekFrom::Current(-buffered_remaining))?;
        }
        self.pos = 0;
        self.len = 0;
        self.inner.seek(pos)
    }
}

/// Buffered writer.
pub struct BufferedWriter<S: StreamWrite> {
    inner: Option<S>,
    buf: Vec<u8>,
    len: usize,
    capacity: usize,
}

impl<S: StreamWrite> BufferedWriter<S> {
    pub fn new(inner: S) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    pub fn with_capacity(capacity: usize, inner: S) -> Self {
        let capacity = capacity.max(4096);
        let buf = vec![0; capacity];
        Self {
            inner: Some(inner),
            buf,
            len: 0,
            capacity,
        }
    }

    pub fn into_inner(mut self) -> io::Result<S> {
        self.flush_buf()?;
        self.inner
            .take()
            .ok_or_else(|| io::Error::other("inner writer already taken"))
    }

    fn flush_buf(&mut self) -> io::Result<()> {
        if self.len > 0 {
            if let Some(inner) = &mut self.inner {
                let mut offset = 0;
                while offset < self.len {
                    let n = inner.write(&self.buf[offset..self.len])?;
                    if n == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to write buffer",
                        ));
                    }
                    offset += n;
                }
            }
            self.len = 0;
        }
        Ok(())
    }
}

impl<S: StreamWrite> StreamWrite for BufferedWriter<S> {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        // If data is larger than buffer, flush and write directly
        if src.len() >= self.capacity {
            self.flush_buf()?;
            if let Some(inner) = &mut self.inner {
                let mut offset = 0;
                while offset < src.len() {
                    let n = inner.write(&src[offset..])?;
                    if n == 0 {
                        return Err(io::Error::new(io::ErrorKind::WriteZero, "failed to write"));
                    }
                    offset += n;
                }
                return Ok(src.len());
            }
            return Ok(0);
        }

        let mut written = 0;
        let mut remaining = src;

        while !remaining.is_empty() {
            let space = self.capacity - self.len;
            if space == 0 {
                self.flush_buf()?;
                continue;
            }

            let to_copy = space.min(remaining.len());
            self.buf[self.len..self.len + to_copy].copy_from_slice(&remaining[..to_copy]);
            self.len += to_copy;
            written += to_copy;
            remaining = &remaining[to_copy..];

            if self.len == self.capacity {
                self.flush_buf()?;
            }
        }

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buf()?;
        if let Some(inner) = &mut self.inner {
            inner.flush()
        } else {
            Ok(())
        }
    }
}

impl<S: StreamWrite> Drop for BufferedWriter<S> {
    fn drop(&mut self) {
        let _ = self.flush_buf();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::file::{FileInput, FileOutput};
    #[test]
    fn test_buffered_reader_basic() {
        let data = b"Hello, World!";
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), data).unwrap();

        let fi = FileInput::open(tmp.path()).unwrap();
        let mut br = BufferedReader::new(fi);
        let mut buf = [0u8; 5];
        let n = br.read(&mut buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf, b"Hello");
    }

    #[test]
    fn test_buffered_reader_borrow_read() {
        let data = b"ACGTACGTACGT";
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), data).unwrap();

        let fi = FileInput::open(tmp.path()).unwrap();
        let mut br = BufferedReader::new(fi);

        let slice = br.fill_buf(4).unwrap();
        assert!(slice.len() >= 4);
        assert_eq!(&slice[..4], b"ACGT");
        br.consume(4);

        let slice = br.fill_buf(4).unwrap();
        assert_eq!(&slice[..4], b"ACGT");
        br.consume(4);

        let slice = br.fill_buf(4).unwrap();
        assert_eq!(&slice[..4], b"ACGT");
        br.consume(4);

        let slice = br.fill_buf(1).unwrap();
        assert!(slice.is_empty()); // EOF
    }

    #[test]
    fn test_buffered_reader_compaction() {
        let mut data = Vec::new();
        for i in 0..10000u32 {
            data.extend_from_slice(&i.to_le_bytes());
        }
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &data).unwrap();

        let fi = FileInput::open(tmp.path()).unwrap();
        let mut br = BufferedReader::with_capacity(128, fi);

        // Read in small chunks that force compaction
        for i in 0..10000u32 {
            let slice = br.fill_buf(4).unwrap();
            assert!(slice.len() >= 4, "failed at i={}", i);
            let val = u32::from_le_bytes(slice[..4].try_into().unwrap());
            assert_eq!(val, i);
            br.consume(4);
        }
    }

    #[test]
    fn test_buffered_writer_round_trip() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write
        {
            let fo = FileOutput::create(&path).unwrap();
            let mut bw = BufferedWriter::new(fo);
            for i in 0..1000u32 {
                bw.write_all(&i.to_le_bytes()).unwrap();
            }
            bw.flush().unwrap();
        }

        // Read back
        let fi = FileInput::open(&path).unwrap();
        let mut br = BufferedReader::new(fi);
        for i in 0..1000u32 {
            let slice = br.fill_buf(4).unwrap();
            assert!(slice.len() >= 4);
            let val = u32::from_le_bytes(slice[..4].try_into().unwrap());
            assert_eq!(val, i);
            br.consume(4);
        }
    }

    #[test]
    fn test_buffered_large_read_bypass() {
        let data: Vec<u8> = (0..200000).map(|i| (i % 256) as u8).collect();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &data).unwrap();

        let fi = FileInput::open(tmp.path()).unwrap();
        let mut br = BufferedReader::with_capacity(4096, fi);

        // Read a chunk larger than buffer capacity
        let mut dst = vec![0u8; 100000];
        let n = br.read(&mut dst).unwrap();
        assert!(n > 0);
        assert_eq!(&dst[..n], &data[..n]);
    }

    #[test]
    fn test_buffered_eof() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"abc").unwrap();

        let fi = FileInput::open(tmp.path()).unwrap();
        let mut br = BufferedReader::new(fi);

        let slice = br.fill_buf(100).unwrap();
        assert_eq!(slice, b"abc"); // Got less than requested (EOF)
        br.consume(3);

        let slice = br.fill_buf(1).unwrap();
        assert!(slice.is_empty());
    }

    #[test]
    fn test_buffered_random_data_round_trip() {
        // Generate pseudo-random data
        let mut data = Vec::with_capacity(100000);
        let mut x: u32 = 12345;
        for _ in 0..100000 {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            data.push((x >> 16) as u8);
        }

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        // Write via BufferedWriter
        {
            let fo = FileOutput::create(&path).unwrap();
            let mut bw = BufferedWriter::with_capacity(8192, fo);
            bw.write_all(&data).unwrap();
            bw.flush().unwrap();
        }

        // Read via BufferedReader
        let fi = FileInput::open(&path).unwrap();
        let mut br = BufferedReader::with_capacity(8192, fi);
        let mut result = Vec::new();
        loop {
            let slice = br.fill_buf(1).unwrap();
            if slice.is_empty() {
                break;
            }
            let n = slice.len();
            result.extend_from_slice(slice);
            br.consume(n);
        }

        assert_eq!(result, data);
    }
}
