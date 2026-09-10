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
use crate::index::MuxIndex;
use crate::mux::block::BlockHeader;
use crate::stream::traits::{StreamRead, StreamSeek};
use std::io;
use std::sync::Arc;

const CACHE_SLOTS: usize = 4;

struct CacheEntry {
    block_id: usize,
    streams: Vec<Vec<u8>>,
    /// Bitmask of which streams were decompressed in this entry.
    loaded_mask: u32,
    /// LRU counter: higher = more recently used.
    last_used: u64,
}

/// Seekable block reader that reads individual blocks by ID using the index.
pub struct IndexedMuxReader<R: StreamRead + StreamSeek> {
    input: R,
    stream_count: usize,
    decompressors: Vec<Option<Box<dyn Decompressor>>>,
    index: Arc<MuxIndex>,
    cache: Vec<CacheEntry>,
    access_counter: u64,
    blocks_read: usize,
    // Reusable read buffer
    read_buf: Vec<u8>,
    /// Bitmask of streams to skip during block decompression.
    /// Skipped streams are consumed from input but not decompressed.
    skip_mask: u32,
}

impl<R: StreamRead + StreamSeek> IndexedMuxReader<R> {
    pub fn new(
        input: R,
        stream_count: usize,
        decompressors: Vec<Option<Box<dyn Decompressor>>>,
        index: MuxIndex,
    ) -> Self {
        assert_eq!(decompressors.len(), stream_count);
        Self {
            input,
            stream_count,
            decompressors,
            index: Arc::new(index),
            cache: Vec::with_capacity(CACHE_SLOTS),
            access_counter: 0,
            blocks_read: 0,
            read_buf: Vec::new(),
            skip_mask: 0,
        }
    }

    /// Find a cached entry for the given block_id that has the needed
    /// stream loaded. Mark it as recently used if found.
    fn cache_lookup(&mut self, block_id: usize, needed_stream: usize) -> Option<usize> {
        for (i, entry) in self.cache.iter_mut().enumerate() {
            if entry.block_id == block_id && (entry.loaded_mask >> needed_stream) & 1 != 0 {
                self.access_counter += 1;
                entry.last_used = self.access_counter;
                return Some(i);
            }
        }
        None
    }

    /// Evict the least recently used cache entry, returning its slot index.
    /// Reuses the evicted entry's stream Vecs to avoid reallocation.
    fn evict_lru(&mut self) -> usize {
        let mut min_idx = 0;
        let mut min_used = u64::MAX;
        for (i, entry) in self.cache.iter().enumerate() {
            if entry.last_used < min_used {
                min_used = entry.last_used;
                min_idx = i;
            }
        }
        min_idx
    }

    /// Read and decompress a specific block by ID, storing results in cache.
    /// Only the `needed_stream` is guaranteed to be decompressed; other
    /// streams may be skipped based on `self.skip_mask`.
    fn read_block(&mut self, block_id: usize, needed_stream: usize) -> io::Result<()> {
        if self.cache_lookup(block_id, needed_stream).is_some() {
            return Ok(());
        }

        let offset = self.index.block_entries[block_id].file_offset;
        self.input.seek(io::SeekFrom::Start(offset))?;

        let hdr = BlockHeader::read_from(&mut self.input, self.stream_count)?;

        // Get or create a cache slot, reusing existing Vec allocations
        let slot = if self.cache.len() < CACHE_SLOTS {
            self.cache.push(CacheEntry {
                block_id,
                streams: (0..self.stream_count).map(|_| Vec::new()).collect(),
                loaded_mask: 0,
                last_used: 0,
            });
            self.cache.len() - 1
        } else {
            self.evict_lru()
        };

        // Read and decompress each stream, validate CRC.
        // When skip_mask is set, skip decompression for masked streams
        // (except the needed_stream which is always decompressed).
        // CRC validation is disabled when any streams are skipped.
        let validate_crc = self.skip_mask == 0;
        let mut crc_val = 0u32;
        let mut loaded_mask = 0u32;
        for s in 0..self.stream_count {
            self.cache[slot].streams[s].clear();
            let comp_size = hdr.compressed_sizes[s] as usize;
            if comp_size == 0 {
                loaded_mask |= 1 << s;
                continue;
            }

            let skip = s != needed_stream && (self.skip_mask >> s) & 1 != 0;
            if skip {
                // Read and discard compressed data
                self.read_buf.clear();
                self.read_buf.resize(comp_size, 0);
                self.input.read_exact(&mut self.read_buf)?;
                continue;
            }

            loaded_mask |= 1 << s;
            let is_raw = hdr.is_raw(s);
            if is_raw {
                self.cache[slot].streams[s].resize(comp_size, 0);
                self.input.read_exact(&mut self.cache[slot].streams[s])?;
                if validate_crc {
                    crc_val = crc32c::crc32c_append(crc_val, &self.cache[slot].streams[s]);
                }
            } else {
                self.read_buf.clear();
                self.read_buf.resize(comp_size, 0);
                self.input.read_exact(&mut self.read_buf)?;

                if let Some(decomp) = &mut self.decompressors[s] {
                    decomp.reset();
                    decomp.decompress(&self.read_buf, &mut self.cache[slot].streams[s])?;
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("stream {} has compressed data but no decompressor", s),
                    ));
                }
                if validate_crc {
                    crc_val = crc32c::crc32c_append(crc_val, &self.cache[slot].streams[s]);
                }
            }
        }

        if validate_crc && crc_val != hdr.crc32 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} CRC32C mismatch: expected 0x{:08X}, got 0x{:08X}",
                    block_id, hdr.crc32, crc_val
                ),
            ));
        }

        self.access_counter += 1;
        self.cache[slot].block_id = block_id;
        self.cache[slot].loaded_mask = loaded_mask;
        self.cache[slot].last_used = self.access_counter;
        self.blocks_read += 1;
        Ok(())
    }

    /// Get cached stream data for a block (must have been read already).
    fn cached_stream(&self, block_id: usize, stream_id: usize) -> &[u8] {
        for entry in &self.cache {
            if entry.block_id == block_id {
                return &entry.streams[stream_id];
            }
        }
        unreachable!(
            "cached_stream called for block {} which is not in cache",
            block_id
        );
    }

    /// Read a byte range [start, end) from a specific stream into a
    /// caller-supplied buffer, spanning multiple blocks if necessary.
    /// The buffer is cleared first, then filled with the requested data.
    pub fn read_stream_range_into(
        &mut self,
        stream_id: usize,
        start: u64,
        end: u64,
        result: &mut Vec<u8>,
    ) -> io::Result<()> {
        result.clear();
        if start >= end {
            return Ok(());
        }

        let block_count = self.index.block_count();
        let first_block = self.index.find_block(stream_id, start);
        let last_block = self.index.find_block(stream_id, end - 1);

        result.reserve((end - start) as usize);

        for block_id in first_block..=last_block {
            if block_id >= block_count {
                break;
            }

            self.read_block(block_id, stream_id)?;
            let stream_data = self.cached_stream(block_id, stream_id);

            let block_start = self.index.cumulative_size(stream_id, block_id);
            let block_end = if block_id + 1 < block_count {
                self.index.cumulative_size(stream_id, block_id + 1)
            } else {
                block_start + stream_data.len() as u64
            };

            // Compute the slice within this block's stream data
            let slice_start = if start > block_start {
                (start - block_start) as usize
            } else {
                0
            };
            let slice_end = if end < block_end {
                (end - block_start) as usize
            } else {
                stream_data.len()
            };

            if slice_start < slice_end && slice_end <= stream_data.len() {
                result.extend_from_slice(&stream_data[slice_start..slice_end]);
            }
        }

        Ok(())
    }

    /// Read a byte range [start, end) from a specific stream, spanning
    /// multiple blocks if necessary. Returns the concatenated data.
    pub fn read_stream_range(
        &mut self,
        stream_id: usize,
        start: u64,
        end: u64,
    ) -> io::Result<Vec<u8>> {
        let mut result = Vec::new();
        self.read_stream_range_into(stream_id, start, end, &mut result)?;
        Ok(result)
    }

    /// Number of blocks actually read and decompressed (for testing).
    pub fn blocks_read(&self) -> usize {
        self.blocks_read
    }

    /// Set a bitmask of streams to skip during block decompression.
    /// Skipped streams are consumed from the input but not decompressed
    /// or cached, saving CPU. Useful for scanning headers without
    /// decompressing large NUC data.
    pub fn set_skip_mask(&mut self, mask: u32) {
        self.skip_mask = mask;
    }

    /// Clear all cached blocks (e.g. after changing skip_mask).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Access the index. Returns Arc for cheap cloning when the caller
    /// needs to hold a reference while mutating the reader.
    pub fn index(&self) -> &Arc<MuxIndex> {
        &self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compress::traits::Compressor;
    use crate::index::IndexTier;
    use crate::mux::writer::MuxWriter;
    use crate::stream::traits::StreamWrite;

    struct VecIO {
        data: Vec<u8>,
        pos: usize,
    }

    impl VecIO {
        fn new() -> Self {
            Self {
                data: Vec::new(),
                pos: 0,
            }
        }
    }

    impl StreamWrite for VecIO {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl StreamRead for VecIO {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let avail = self.data.len() - self.pos;
            let n = buf.len().min(avail);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    impl StreamSeek for VecIO {
        fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
            let new_pos = match pos {
                io::SeekFrom::Start(p) => p as i64,
                io::SeekFrom::Current(p) => self.pos as i64 + p,
                io::SeekFrom::End(p) => self.data.len() as i64 + p,
            };
            if new_pos < 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "seek to negative position",
                ));
            }
            self.pos = new_pos as usize;
            Ok(self.pos as u64)
        }
    }

    fn make_raw_compressors(n: usize) -> Vec<Option<Box<dyn Compressor>>> {
        (0..n).map(|_| None).collect()
    }

    fn make_raw_decompressors(n: usize) -> Vec<Option<Box<dyn Decompressor>>> {
        (0..n).map(|_| None).collect()
    }

    #[test]
    fn test_indexed_read_single_block() {
        let n_streams = 3;
        let budget = 1024 * 1024;
        let mut out = VecIO::new();

        let (index_builder, _offset) = {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                Some(IndexTier::BlockOnly),
                0,
            );
            mux.write_to_stream(0, b"Hello").unwrap();
            mux.write_to_stream(1, b"World").unwrap();
            mux.write_to_stream(2, b"Test!").unwrap();
            let (_w, ib, offset) = mux.finalize().unwrap();
            (ib.unwrap(), offset)
        };

        let index = index_builder.build();

        let reader_io = VecIO {
            data: out.data.clone(),
            pos: 0,
        };
        let mut reader = IndexedMuxReader::new(
            reader_io,
            n_streams,
            make_raw_decompressors(n_streams),
            index,
        );

        let data0 = reader.read_stream_range(0, 0, 5).unwrap();
        assert_eq!(&data0, b"Hello");

        let data1 = reader.read_stream_range(1, 0, 5).unwrap();
        assert_eq!(&data1, b"World");

        let data2 = reader.read_stream_range(2, 0, 5).unwrap();
        assert_eq!(&data2, b"Test!");

        // Only 1 block should have been read (cache hit for subsequent reads)
        assert_eq!(reader.blocks_read(), 1);
    }

    #[test]
    fn test_indexed_read_multi_block() {
        let n_streams = 2;
        let budget = 50; // Small budget to force multiple blocks
        let mut out = VecIO::new();

        let index_builder = {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                Some(IndexTier::BlockOnly),
                0,
            );
            // Write enough to span multiple blocks
            mux.write_to_stream(0, &vec![b'A'; 30]).unwrap();
            mux.write_to_stream(1, &vec![b'B'; 30]).unwrap();
            // This should trigger a block flush (60 > 50)
            mux.write_to_stream(0, &vec![b'C'; 30]).unwrap();
            mux.write_to_stream(1, &vec![b'D'; 30]).unwrap();
            let (_w, ib, _offset) = mux.finalize().unwrap();
            ib.unwrap()
        };

        let index = index_builder.build();
        assert!(index.block_count() >= 2);

        let reader_io = VecIO {
            data: out.data.clone(),
            pos: 0,
        };
        let mut reader = IndexedMuxReader::new(
            reader_io,
            n_streams,
            make_raw_decompressors(n_streams),
            index,
        );

        // Read spanning all blocks for stream 0
        let data0 = reader.read_stream_range(0, 0, 60).unwrap();
        assert_eq!(data0.len(), 60);
        assert!(data0[..30].iter().all(|&b| b == b'A'));
        assert!(data0[30..].iter().all(|&b| b == b'C'));
    }

    #[test]
    fn test_indexed_read_partial_range() {
        let n_streams = 1;
        let budget = 1024 * 1024;
        let mut out = VecIO::new();

        let index_builder = {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                Some(IndexTier::BlockOnly),
                0,
            );
            mux.write_to_stream(0, b"Hello, World!").unwrap();
            let (_w, ib, _offset) = mux.finalize().unwrap();
            ib.unwrap()
        };

        let index = index_builder.build();

        let reader_io = VecIO {
            data: out.data.clone(),
            pos: 0,
        };
        let mut reader = IndexedMuxReader::new(
            reader_io,
            n_streams,
            make_raw_decompressors(n_streams),
            index,
        );

        // Read a sub-range
        let data = reader.read_stream_range(0, 7, 12).unwrap();
        assert_eq!(&data, b"World");
    }

    #[test]
    fn test_indexed_read_block_skipping() {
        let n_streams = 1;
        let budget = 20; // Very small: forces many blocks
        let mut out = VecIO::new();

        let index_builder = {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                Some(IndexTier::BlockOnly),
                0,
            );
            // Write 200 bytes in small chunks to force many blocks
            for i in 0..10u8 {
                mux.write_to_stream(0, &vec![b'A' + i; 20]).unwrap();
            }
            let (_w, ib, _offset) = mux.finalize().unwrap();
            ib.unwrap()
        };

        let index = index_builder.build();
        let total_blocks = index.block_count();
        assert!(
            total_blocks >= 5,
            "should have many blocks, got {}",
            total_blocks
        );

        let reader_io = VecIO {
            data: out.data.clone(),
            pos: 0,
        };
        let mut reader = IndexedMuxReader::new(
            reader_io,
            n_streams,
            make_raw_decompressors(n_streams),
            index,
        );

        // Read just a small range from the middle
        let data = reader.read_stream_range(0, 50, 70).unwrap();
        assert_eq!(data.len(), 20);

        // Should have read fewer blocks than total
        assert!(
            reader.blocks_read() < total_blocks,
            "read {} blocks but total is {} -- should skip blocks",
            reader.blocks_read(),
            total_blocks,
        );
    }

    #[test]
    fn test_indexed_empty_range() {
        let n_streams = 1;
        let budget = 1024;
        let mut out = VecIO::new();

        let index_builder = {
            let mut mux = MuxWriter::new(
                &mut out,
                n_streams,
                budget,
                make_raw_compressors(n_streams),
                vec![None; n_streams],
                Some(IndexTier::BlockOnly),
                0,
            );
            mux.write_to_stream(0, b"data").unwrap();
            let (_w, ib, _offset) = mux.finalize().unwrap();
            ib.unwrap()
        };

        let index = index_builder.build();

        let reader_io = VecIO {
            data: out.data.clone(),
            pos: 0,
        };
        let mut reader = IndexedMuxReader::new(
            reader_io,
            n_streams,
            make_raw_decompressors(n_streams),
            index,
        );

        let data = reader.read_stream_range(0, 5, 5).unwrap();
        assert!(data.is_empty());
        assert_eq!(reader.blocks_read(), 0);
    }
}
