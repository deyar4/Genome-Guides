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

/// Readable byte stream.
pub trait StreamRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let mut offset = 0;
        while offset < buf.len() {
            let n = self.read(&mut buf[offset..])?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "unexpected EOF in read_exact",
                ));
            }
            offset += n;
        }
        Ok(())
    }
}

/// Writable byte stream.
pub trait StreamWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        let mut offset = 0;
        while offset < buf.len() {
            let n = self.write(&buf[offset..])?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "failed to write in write_all",
                ));
            }
            offset += n;
        }
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()>;
}

/// Seekable stream.
pub trait StreamSeek {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64>;
    fn position(&mut self) -> io::Result<u64> {
        self.seek(io::SeekFrom::Current(0))
    }
}

/// Zero-copy borrowing reader: fill a buffer and return a reference to it.
pub trait BorrowRead {
    /// Fill the internal buffer so that at least `min_bytes` are available
    /// (or EOF is reached). Returns the available data without consuming it.
    fn fill_buf(&mut self, min_bytes: usize) -> io::Result<&[u8]>;

    /// Consume `n` bytes from the front of the buffer.
    fn consume(&mut self, n: usize);
}

// --- Helper free functions for reading/writing integers ---

pub fn read_u8(r: &mut dyn StreamRead) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

pub fn read_u16le(r: &mut dyn StreamRead) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

pub fn read_u32le(r: &mut dyn StreamRead) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn read_u64le(r: &mut dyn StreamRead) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

pub fn read_i64le(r: &mut dyn StreamRead) -> io::Result<i64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(i64::from_le_bytes(buf))
}

pub fn write_u8(w: &mut dyn StreamWrite, v: u8) -> io::Result<()> {
    w.write_all(&[v])
}

pub fn write_u16le(w: &mut dyn StreamWrite, v: u16) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

pub fn write_u32le(w: &mut dyn StreamWrite, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

pub fn write_u64le(w: &mut dyn StreamWrite, v: u64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

pub fn write_i64le(w: &mut dyn StreamWrite, v: i64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

// --- Blanket impls for Box<T> and &mut T ---

impl<T: StreamRead + ?Sized> StreamRead for Box<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }
}

impl<T: StreamRead + ?Sized> StreamRead for &mut T {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }
}

impl<T: StreamWrite + ?Sized> StreamWrite for Box<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (**self).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }
}

impl<T: StreamWrite + ?Sized> StreamWrite for &mut T {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (**self).write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }
}

impl<T: StreamSeek + ?Sized> StreamSeek for Box<T> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        (**self).seek(pos)
    }
}

impl<T: StreamSeek + ?Sized> StreamSeek for &mut T {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        (**self).seek(pos)
    }
}

impl<T: BorrowRead + ?Sized> BorrowRead for Box<T> {
    fn fill_buf(&mut self, min_bytes: usize) -> io::Result<&[u8]> {
        (**self).fill_buf(min_bytes)
    }
    fn consume(&mut self, n: usize) {
        (**self).consume(n)
    }
}

impl<T: BorrowRead + ?Sized> BorrowRead for &mut T {
    fn fill_buf(&mut self, min_bytes: usize) -> io::Result<&[u8]> {
        (**self).fill_buf(min_bytes)
    }
    fn consume(&mut self, n: usize) {
        (**self).consume(n)
    }
}
