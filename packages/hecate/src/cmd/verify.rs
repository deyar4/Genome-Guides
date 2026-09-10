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

use super::{build_decompressors, cmp_header_size, make_decompressor, open_input, SliceCursor};
use crate::Cli;
use hecate::container::*;
use hecate::mux::block::BlockHeader;
use hecate::stream::file::FileInput;
use hecate::stream::mmap::MmapInput;
use hecate::stream::traits::{StreamRead, StreamSeek};
use std::io;

pub fn cmd_verify(cli: &Cli) -> io::Result<()> {
    if let Some(ref path) = cli.file {
        if !cli.no_mmap {
            if let Ok(mmap) = MmapInput::open(path) {
                return verify_mmap(cli, &mmap);
            }
        }
    }
    verify_sequential(cli)
}

fn verify_block_crc(
    bh: &BlockHeader,
    payload: &[u8],
    stream_count: usize,
    indexed: bool,
    methods: &[MethodSpec],
    pattern: &[u8],
) -> io::Result<()> {
    let mut crc = 0u32;
    let mut offset = 0;
    for s in 0..stream_count {
        let size = bh.compressed_sizes[s] as usize;
        let data = &payload[offset..offset + size];
        offset += size;
        if data.is_empty() {
            continue;
        }
        if !indexed || bh.is_raw(s) || pattern[s] == 0xFF {
            crc = crc32c::crc32c_append(crc, data);
        } else {
            let mut decomp = make_decompressor(methods, pattern, s)?;
            let mut buf = Vec::new();
            decomp.decompress(data, &mut buf)?;
            crc = crc32c::crc32c_append(crc, &buf);
        }
    }
    if crc != bh.crc32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CRC mismatch: expected 0x{:08X}, got 0x{:08X}",
                bh.crc32, crc
            ),
        ));
    }
    Ok(())
}

fn verify_block_sequential(
    input: &mut dyn StreamRead,
    bh: &BlockHeader,
    stream_count: usize,
    indexed: bool,
    decompressors: &mut [Option<Box<dyn hecate::compress::traits::Decompressor>>],
    stream_buf: &mut Vec<u8>,
    decomp_buf: &mut Vec<u8>,
) -> io::Result<()> {
    let mut crc = 0u32;
    for s in 0..stream_count {
        let size = bh.compressed_sizes[s] as usize;
        if size == 0 {
            continue;
        }
        stream_buf.resize(size, 0);
        input.read_exact(&mut stream_buf[..size])?;
        if !indexed || bh.is_raw(s) {
            crc = crc32c::crc32c_append(crc, &stream_buf[..size]);
        } else if let Some(ref mut decomp) = decompressors[s] {
            decomp_buf.clear();
            decomp.decompress(&stream_buf[..size], decomp_buf)?;
            decomp.reset();
            crc = crc32c::crc32c_append(crc, decomp_buf);
        } else {
            crc = crc32c::crc32c_append(crc, &stream_buf[..size]);
        }
    }
    if crc != bh.crc32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CRC mismatch: expected 0x{:08X}, got 0x{:08X}",
                bh.crc32, crc
            ),
        ));
    }
    Ok(())
}

fn verify_mmap(cli: &Cli, mmap: &MmapInput) -> io::Result<()> {
    use rayon::prelude::*;

    let data = mmap.as_slice();
    let mut cursor = SliceCursor::new(data);

    let file_header = FileHeader::read_from(&mut cursor)?;
    let cmp_header = CompressionHeader::read_from(&mut cursor)?;
    cmp_header.validate(file_header.stream_count())?;

    let stream_count = file_header.stream_count() as usize;
    let header_size = BlockHeader::header_size(stream_count);

    let block_data_end = if file_header.indexed {
        if data.len() < Trailer::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "file too small for trailer",
            ));
        }
        let trailer_start = data.len() - Trailer::SIZE;
        let mut tc = SliceCursor::new(&data[trailer_start..]);
        let trailer = Trailer::read_from(&mut tc)?;
        trailer.index_offset as usize
    } else {
        data.len()
    };

    struct BlockInfo {
        bh: BlockHeader,
        payload_offset: usize,
    }

    let mut blocks = Vec::new();
    let mut pos = cursor.pos;
    while pos + header_size <= block_data_end {
        let mut hc = SliceCursor::new(&data[pos..]);
        let bh = match BlockHeader::read_from(&mut hc, stream_count) {
            Ok(bh) => bh,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };
        let payload_offset = pos + header_size;
        let payload_size = bh.total_compressed_data_size() as usize;
        if payload_offset + payload_size > block_data_end {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "block payload extends beyond data region",
            ));
        }
        blocks.push(BlockInfo { bh, payload_offset });
        pos = payload_offset + payload_size;
    }

    let block_count = blocks.len();

    blocks.par_iter().enumerate().try_for_each(|(idx, info)| {
        let payload = &data[info.payload_offset
            ..info.payload_offset + info.bh.total_compressed_data_size() as usize];
        verify_block_crc(
            &info.bh,
            payload,
            stream_count,
            file_header.indexed,
            &cmp_header.methods,
            &cmp_header.pattern,
        )
        .map_err(|e| io::Error::new(e.kind(), format!("block {}: {}", idx, e)))
    })?;

    if !cli.quiet {
        eprintln!("OK: {} blocks verified", block_count);
    }
    Ok(())
}

fn verify_sequential(cli: &Cli) -> io::Result<()> {
    let mut input = open_input(cli)?;

    let file_header = FileHeader::read_from(&mut input)?;
    let cmp_header = CompressionHeader::read_from(&mut input)?;
    cmp_header.validate(file_header.stream_count())?;

    let stream_count = file_header.stream_count() as usize;

    let block_data_end = if file_header.indexed {
        if let Some(ref path) = cli.file {
            let file_len = std::fs::metadata(path)?.len();
            let trailer_pos = file_len - Trailer::SIZE as u64;
            let mut f = FileInput::open(path)?;
            f.seek(io::SeekFrom::Start(trailer_pos))?;
            let trailer = Trailer::read_from(&mut f)?;
            Some(trailer.index_offset)
        } else {
            None
        }
    } else {
        None
    };

    let mut decompressors = if file_header.indexed {
        build_decompressors(&cmp_header.methods, &cmp_header.pattern)?
    } else {
        (0..stream_count).map(|_| None).collect()
    };

    let mut block_idx = 0u64;
    let mut file_pos = 8 + cmp_header_size(&cmp_header.methods, stream_count);
    let mut stream_buf = Vec::new();
    let mut decomp_buf = Vec::new();

    loop {
        if let Some(end) = block_data_end {
            if file_pos >= end {
                break;
            }
        }

        let bh = match BlockHeader::read_from(&mut input, stream_count) {
            Ok(bh) => bh,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };

        file_pos +=
            BlockHeader::header_size(stream_count) as u64 + bh.total_compressed_data_size() as u64;

        verify_block_sequential(
            &mut input,
            &bh,
            stream_count,
            file_header.indexed,
            &mut decompressors,
            &mut stream_buf,
            &mut decomp_buf,
        )
        .map_err(|e| io::Error::new(e.kind(), format!("block {}: {}", block_idx, e)))?;

        block_idx += 1;
    }

    if !cli.quiet {
        eprintln!("OK: {} blocks verified", block_idx);
    }
    Ok(())
}
