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

use super::{
    cmp_header_size, format_number, open_input, print_stats, read_index_stats, skip_bytes,
    SliceCursor,
};
use crate::Cli;
use hecate::container::*;
use hecate::mux::block::BlockHeader;
use hecate::stream::file::FileInput;
use hecate::stream::mmap::MmapInput;
use hecate::stream::traits::StreamSeek;
use std::io;

pub fn cmd_stats(cli: &Cli) -> io::Result<()> {
    if let Some(ref path) = cli.file {
        if !cli.no_mmap {
            if let Ok(mmap) = MmapInput::open(path) {
                return stats_mmap(cli, &mmap);
            }
        }
    }
    stats_sequential(cli)
}

fn stats_mmap(cli: &Cli, mmap: &MmapInput) -> io::Result<()> {
    let data = mmap.as_slice();
    let mut cursor = SliceCursor::new(data);

    let file_header = FileHeader::read_from(&mut cursor)?;
    let cmp_header = CompressionHeader::read_from(&mut cursor)?;
    cmp_header.validate(file_header.stream_count())?;

    if file_header.referential {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stats is not supported for referentially compressed files",
        ));
    }

    let stream_count = file_header.stream_count() as usize;
    let header_size = BlockHeader::header_size(stream_count);

    let (block_data_end, seq_count_str, total_bases_str) = if file_header.indexed {
        if data.len() < Trailer::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "file too small for trailer",
            ));
        }
        let trailer_start = data.len() - Trailer::SIZE;
        let mut tc = SliceCursor::new(&data[trailer_start..]);
        let trailer = Trailer::read_from(&mut tc)?;

        let (sc, tb) = if trailer.index_offset > 0 {
            if trailer.index_offset as usize >= data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trailer index_offset exceeds file size",
                ));
            }
            let mut ic = SliceCursor::new(&data[trailer.index_offset as usize..]);
            let index = hecate::index::reader::read_index(&mut ic)?;
            let sc = format!("{}", index.sequence_count());
            let tb = if index.sequence_entries.is_empty() {
                "-".into()
            } else {
                format_number(index.sequence_entries.iter().map(|e| e.total_bases).sum())
            };
            (sc, tb)
        } else {
            ("-".into(), "-".into())
        };

        (trailer.index_offset as usize, sc, tb)
    } else {
        (data.len(), "-".into(), "-".into())
    };

    let mut total_compressed = vec![0u64; stream_count];
    let mut total_uncompressed = vec![0u64; stream_count];
    let mut block_count = 0u64;
    let mut pos = cursor.pos;

    while pos + header_size <= block_data_end {
        let mut hc = SliceCursor::new(&data[pos..]);
        let bh = match BlockHeader::read_from(&mut hc, stream_count) {
            Ok(bh) => bh,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };
        let payload_size = bh.total_compressed_data_size() as usize;
        let next = pos + header_size + payload_size;
        if next > block_data_end {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "block payload extends beyond data region",
            ));
        }
        for s in 0..stream_count {
            total_compressed[s] += bh.compressed_sizes[s];
            total_uncompressed[s] += bh.uncompressed_sizes[s];
        }
        block_count += 1;
        pos = next;
    }

    print_stats(
        cli,
        &file_header,
        &cmp_header,
        block_count,
        &total_compressed,
        &total_uncompressed,
        &seq_count_str,
        &total_bases_str,
    );
    Ok(())
}

fn stats_sequential(cli: &Cli) -> io::Result<()> {
    let mut input = open_input(cli)?;

    let file_header = FileHeader::read_from(&mut input)?;
    let cmp_header = CompressionHeader::read_from(&mut input)?;
    cmp_header.validate(file_header.stream_count())?;

    if file_header.referential {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stats is not supported for referentially compressed files",
        ));
    }

    let stream_count = file_header.stream_count() as usize;

    let (block_data_end, seq_count_str, total_bases_str) = if file_header.indexed {
        if let Some(ref path) = cli.file {
            let (sc, tb) = read_index_stats(path)?;
            let file_len = std::fs::metadata(path)?.len();
            let trailer_pos = file_len - Trailer::SIZE as u64;
            let mut f = FileInput::open(path)?;
            f.seek(io::SeekFrom::Start(trailer_pos))?;
            let trailer = Trailer::read_from(&mut f)?;
            (Some(trailer.index_offset), sc, tb)
        } else {
            (None, "-".into(), "-".into())
        }
    } else {
        (None, "-".into(), "-".into())
    };

    let mut total_compressed = vec![0u64; stream_count];
    let mut total_uncompressed = vec![0u64; stream_count];
    let mut block_count = 0u64;
    let mut file_pos = 8 + cmp_header_size(&cmp_header.methods, stream_count);

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

        let payload_size = bh.total_compressed_data_size();
        skip_bytes(&mut input, payload_size)?;

        file_pos += BlockHeader::header_size(stream_count) as u64 + payload_size;

        for s in 0..stream_count {
            total_compressed[s] += bh.compressed_sizes[s];
            total_uncompressed[s] += bh.uncompressed_sizes[s];
        }
        block_count += 1;
    }

    print_stats(
        cli,
        &file_header,
        &cmp_header,
        block_count,
        &total_compressed,
        &total_uncompressed,
        &seq_count_str,
        &total_bases_str,
    );
    Ok(())
}
