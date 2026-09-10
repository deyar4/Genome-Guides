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

use super::traits::{BorrowRead, StreamRead, StreamSeek};
use memmap2::MmapOptions;
use std::fs::{self, File};
use std::io;
use std::path::Path;

/// Memory-mapped file input supporting both `StreamRead` (copy) and `BorrowRead` (zero-copy).
pub struct MmapInput {
    mmap: memmap2::Mmap,
    pos: usize,
}

impl MmapInput {
    /// Open a file for mmap reading. Returns `Err` for empty or non-regular files
    /// (caller should fall back to `FileInput`).
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref();
        let meta = fs::metadata(path)?;

        // Reject non-regular files
        if !meta.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "mmap not supported for non-regular files",
            ));
        }

        // Reject empty files
        if meta.len() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot mmap an empty file",
            ));
        }

        let file = File::open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // Advise sequential access by default
        #[cfg(unix)]
        {
            let _ = mmap.advise(memmap2::Advice::Sequential);
        }

        Ok(Self { mmap, pos: 0 })
    }

    /// Advise sequential read pattern (forward scan).
    pub fn advise_sequential(&self) {
        #[cfg(unix)]
        {
            let _ = self.mmap.advise(memmap2::Advice::Sequential);
        }
    }

    /// Advise random access pattern (seeking around).
    pub fn advise_random(&self) {
        #[cfg(unix)]
        {
            let _ = self.mmap.advise(memmap2::Advice::Random);
        }
    }

    /// Total length of the mapped region.
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// Current cursor position.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Remaining bytes.
    pub fn remaining(&self) -> usize {
        self.mmap.len().saturating_sub(self.pos)
    }

    /// Access the full underlying bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap
    }
}

impl StreamRead for MmapInput {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.mmap.len() {
            return Ok(0);
        }
        let remaining = self.mmap.len() - self.pos;
        let to_copy = remaining.min(buf.len());
        buf[..to_copy].copy_from_slice(&self.mmap[self.pos..self.pos + to_copy]);
        self.pos += to_copy;
        Ok(to_copy)
    }
}

impl BorrowRead for MmapInput {
    fn fill_buf(&mut self, _min_bytes: usize) -> io::Result<&[u8]> {
        // Mmap always has all data available - just return the remaining slice.
        Ok(&self.mmap[self.pos..])
    }

    fn consume(&mut self, n: usize) {
        self.pos += n;
        debug_assert!(self.pos <= self.mmap.len());
    }
}

impl StreamSeek for MmapInput {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        let new_pos: i64 = match pos {
            io::SeekFrom::Start(n) => n as i64,
            io::SeekFrom::Current(delta) => (self.pos as i64).saturating_add(delta),
            io::SeekFrom::End(delta) => (self.mmap.len() as i64).saturating_add(delta),
        };
        if new_pos < 0 || new_pos as usize > self.mmap.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "mmap seek out of bounds",
            ));
        }
        self.pos = new_pos as usize;
        Ok(new_pos as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmap_basic() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"ACGTACGT").unwrap();

        let mut m = MmapInput::open(tmp.path()).unwrap();
        assert_eq!(m.len(), 8);

        let slice = m.fill_buf(4).unwrap();
        assert_eq!(&slice[..4], b"ACGT");
        m.consume(4);

        let slice = m.fill_buf(4).unwrap();
        assert_eq!(slice, b"ACGT");
        m.consume(4);

        let slice = m.fill_buf(1).unwrap();
        assert!(slice.is_empty());
    }

    #[test]
    fn test_mmap_stream_read() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"Hello World").unwrap();

        let mut m = MmapInput::open(tmp.path()).unwrap();
        let mut buf = [0u8; 5];
        let n = m.read(&mut buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf, b"Hello");
    }

    #[test]
    fn test_mmap_empty_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // File is empty by default
        let result = MmapInput::open(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_mmap_seek() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"ABCDEFGH").unwrap();

        let mut m = MmapInput::open(tmp.path()).unwrap();
        m.seek(io::SeekFrom::Start(4)).unwrap();
        let slice = m.fill_buf(4).unwrap();
        assert_eq!(slice, b"EFGH");
    }

    #[test]
    fn test_mmap_seek_returns_new_position() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"ABCDEFGHIJ").unwrap();

        let mut m = MmapInput::open(tmp.path()).unwrap();

        // SeekFrom::Start should return the new position
        let pos = m.seek(io::SeekFrom::Start(5)).unwrap();
        assert_eq!(pos, 5, "seek(Start(5)) should return 5");

        // SeekFrom::Current should return the new position
        let pos = m.seek(io::SeekFrom::Current(2)).unwrap();
        assert_eq!(pos, 7, "seek(Current(2)) from 5 should return 7");

        // SeekFrom::End should return the new position
        let pos = m.seek(io::SeekFrom::End(-3)).unwrap();
        assert_eq!(pos, 7, "seek(End(-3)) on 10-byte file should return 7");

        // Current(0) should return the current position (no-op seek)
        let pos = m.seek(io::SeekFrom::Current(0)).unwrap();
        assert_eq!(pos, 7, "seek(Current(0)) should return current position");
    }

    #[test]
    fn test_mmap_seek_out_of_bounds() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"ABCD").unwrap();

        let mut m = MmapInput::open(tmp.path()).unwrap();

        // Past end
        assert!(m.seek(io::SeekFrom::Start(5)).is_err());

        // Before start
        assert!(m.seek(io::SeekFrom::Current(-1)).is_err());
    }
}
