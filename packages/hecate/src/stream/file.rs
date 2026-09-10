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

use super::traits::{StreamRead, StreamSeek, StreamWrite};
use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::path::Path;

/// Thin wrapper around `std::fs::File` for reading.
pub struct FileInput {
    file: File,
}

impl FileInput {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self { file })
    }

    pub fn from_file(file: File) -> Self {
        Self { file }
    }
}

impl StreamRead for FileInput {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl StreamSeek for FileInput {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}

/// Thin wrapper around `std::fs::File` for writing.
pub struct FileOutput {
    file: File,
}

impl FileOutput {
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::create(path)?;
        Ok(Self { file })
    }

    pub fn from_file(file: File) -> Self {
        Self { file }
    }
}

impl StreamWrite for FileOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl StreamSeek for FileOutput {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}
