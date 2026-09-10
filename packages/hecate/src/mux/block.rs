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

use crate::stream::traits::{
    read_u32le, read_u64le, read_u8, write_u32le, write_u64le, write_u8, StreamRead, StreamWrite,
};
use std::io;

/// Block header: flags, per-stream compressed/uncompressed sizes, CRC32C.
///
/// Sizes are u64 to accommodate huge blocks (e.g. BWT with multi-GB block
/// sizes or per-sequence flush for FASTAs containing very long scaffolds).
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub flags: u8,
    pub compressed_sizes: Vec<u64>,
    pub uncompressed_sizes: Vec<u64>,
    pub crc32: u32,
}

impl BlockHeader {
    pub fn header_size(stream_count: usize) -> usize {
        1 + 2 * stream_count * 8 + 4
    }

    pub fn total_compressed_data_size(&self) -> u64 {
        self.compressed_sizes.iter().copied().sum()
    }

    pub fn is_raw(&self, stream_id: usize) -> bool {
        self.flags & (1 << stream_id) != 0
    }

    pub fn write_to(&self, w: &mut dyn StreamWrite) -> io::Result<()> {
        write_u8(w, self.flags)?;
        for &cs in &self.compressed_sizes {
            write_u64le(w, cs)?;
        }
        for &us in &self.uncompressed_sizes {
            write_u64le(w, us)?;
        }
        write_u32le(w, self.crc32)?;
        Ok(())
    }

    pub fn read_from(r: &mut dyn StreamRead, stream_count: usize) -> io::Result<Self> {
        let flags = read_u8(r)?;
        let mut compressed_sizes = Vec::with_capacity(stream_count);
        for _ in 0..stream_count {
            compressed_sizes.push(read_u64le(r)?);
        }
        let mut uncompressed_sizes = Vec::with_capacity(stream_count);
        for _ in 0..stream_count {
            uncompressed_sizes.push(read_u64le(r)?);
        }
        let crc32 = read_u32le(r)?;
        Ok(BlockHeader {
            flags,
            compressed_sizes,
            uncompressed_sizes,
            crc32,
        })
    }
}
