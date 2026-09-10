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

use super::{cmp_header_size, make_decompressor, open_output, SliceCursor};
use hecate::container::*;
use hecate::mux::block::BlockHeader;
use hecate::stream::file::FileInput;
use hecate::stream::mmap::MmapInput;
use hecate::stream::traits::{StreamRead, StreamSeek, StreamWrite};
use std::io;
use std::path::PathBuf;

pub fn cmd_cat(
    output_path: Option<&PathBuf>,
    force: bool,
    verbose: bool,
    no_mmap: bool,
    inputs: Vec<PathBuf>,
) -> io::Result<()> {
    if inputs.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cat requires at least 2 input files",
        ));
    }
    let output_path = output_path
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cat requires -o <output>"))?;

    if !no_mmap {
        let mmaps: Result<Vec<_>, _> = inputs.iter().map(MmapInput::open).collect();
        if let Ok(mmaps) = mmaps {
            return cat_mmap(output_path, force, verbose, &inputs, &mmaps);
        }
    }

    cat_sequential(output_path, force, verbose, &inputs)
}

/// Validate headers from all inputs, returning (first_fh, first_ch, header_sizes).
/// Non-indexed files with streaming compression (zstd) are rejected;
/// non-indexed uncompressed files are allowed (TAG_EOF stripping handles them).
fn validate_headers(
    inputs: &[PathBuf],
    read_header: &mut dyn FnMut(&PathBuf) -> io::Result<(FileHeader, CompressionHeader)>,
) -> io::Result<(FileHeader, CompressionHeader, Vec<u64>)> {
    let mut first_file_header: Option<FileHeader> = None;
    let mut first_cmp_header: Option<CompressionHeader> = None;
    let mut header_sizes = Vec::new();

    for path in inputs {
        let (fh, ch) = read_header(path)?;
        ch.validate(fh.stream_count())?;

        // Non-indexed files with compressed streams use streaming frames
        // that span blocks -- blocks are not independent, so concatenation
        // would break decompression at file boundaries.
        if !fh.indexed && ch.pattern.iter().any(|&p| p != 0xFF) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{}: cannot concatenate non-indexed compressed files \
                     (streaming frames span blocks; encode with -I or without -M)",
                    path.display()
                ),
            ));
        }

        if fh.referential {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{}: cannot concatenate referentially compressed files",
                    path.display()
                ),
            ));
        }

        if let Some(ref first_fh) = first_file_header {
            if fh.alphabet != first_fh.alphabet
                || fh.file_type != first_fh.file_type
                || fh.case_mode != first_fh.case_mode
                || fh.indexed != first_fh.indexed
                || fh.illumina != first_fh.illumina
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{}: file header does not match first file", path.display()),
                ));
            }
        }

        if let Some(ref first_ch) = first_cmp_header {
            if ch.methods != first_ch.methods || ch.pattern != first_ch.pattern {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "{}: compression header does not match first file",
                        path.display()
                    ),
                ));
            }
        }

        let stream_count = fh.stream_count() as usize;
        let hs = 8 + cmp_header_size(&ch.methods, stream_count);
        header_sizes.push(hs);

        if first_file_header.is_none() {
            first_file_header = Some(fh);
            first_cmp_header = Some(ch);
        }
    }

    Ok((
        first_file_header.unwrap(),
        first_cmp_header.unwrap(),
        header_sizes,
    ))
}

/// Merge an index from one input file into the accumulated merged index.
fn merge_index(
    merged_index: &mut Option<hecate::index::MuxIndex>,
    idx: hecate::index::MuxIndex,
    stream_count: usize,
    data_start: u64,
    output_data_offset: u64,
) {
    match merged_index.take() {
        None => {
            let delta = output_data_offset as i64 - data_start as i64;
            let mut adjusted = idx;
            for entry in &mut adjusted.block_entries {
                entry.file_offset = (entry.file_offset as i64 + delta) as u64;
            }
            *merged_index = Some(adjusted);
        }
        Some(mut prev) => {
            let offset_delta = output_data_offset as i64 - data_start as i64;
            let stream_deltas: Vec<u64> = prev.total_stream_sizes.clone();

            for mut entry in idx.block_entries {
                entry.file_offset = (entry.file_offset as i64 + offset_delta) as u64;
                for s in 0..stream_count {
                    entry.cumulative_uncompressed[s] += stream_deltas[s];
                }
                prev.block_entries.push(entry);
            }

            for mut seq in idx.sequence_entries {
                seq.header_offset += stream_deltas[STREAM_HDR];
                seq.nuc_offset += stream_deltas[STREAM_NUC];
                seq.case_offset += stream_deltas[STREAM_CASE];
                seq.ctrl_offset += stream_deltas[STREAM_CTRL];
                seq.extra_offset += stream_deltas[STREAM_EXTRA];
                if stream_count > 5 {
                    seq.quality_offset += stream_deltas[STREAM_QUALITY];
                }
                prev.sequence_entries.push(seq);
            }

            for s in 0..stream_count {
                prev.total_stream_sizes[s] += idx.total_stream_sizes[s];
            }

            *merged_index = Some(prev);
        }
    }
}

/// Write the merged index and trailer to the output.
fn write_footer(
    output: &mut dyn StreamWrite,
    merged_index: &Option<hecate::index::MuxIndex>,
    output_data_offset: u64,
) -> io::Result<()> {
    let index_offset = if let Some(ref idx) = merged_index {
        let out_pos = output_data_offset;
        idx.write_to(output)?;
        out_pos
    } else {
        0
    };
    Trailer { index_offset }.write_to(output)?;
    output.flush()
}

// ---- Non-indexed helpers: strip TAG_EOF from intermediate files ----

/// Write a block with the last byte of the CTRL stream (TAG_EOF) removed.
/// `payload` is the raw stream data after the block header.
/// All streams must be raw (no compression) -- CRC is over stored data.
fn write_block_strip_eof(
    output: &mut dyn StreamWrite,
    bh: &BlockHeader,
    payload: &[u8],
    stream_count: usize,
) -> io::Result<usize> {
    let ctrl_size = bh.compressed_sizes[STREAM_CTRL] as usize;
    if ctrl_size == 0 {
        // No CTRL data in this block -- nothing to strip. Write verbatim.
        bh.write_to(output)?;
        output.write_all(payload)?;
        return Ok(BlockHeader::header_size(stream_count) + payload.len());
    }

    // Split payload into per-stream slices.
    let mut offset = 0;
    let mut stream_slices = Vec::with_capacity(stream_count);
    for s in 0..stream_count {
        let sz = bh.compressed_sizes[s] as usize;
        stream_slices.push(&payload[offset..offset + sz]);
        offset += sz;
    }

    // CTRL stream: strip the trailing TAG_EOF byte.
    let ctrl_data = stream_slices[STREAM_CTRL];
    debug_assert_eq!(*ctrl_data.last().unwrap(), TAG_EOF);
    let trimmed_ctrl = &ctrl_data[..ctrl_data.len() - 1];

    // Recompute CRC over all stored (raw) stream data.
    let mut crc = 0u32;
    for s in 0..stream_count {
        let data = if s == STREAM_CTRL {
            trimmed_ctrl
        } else {
            stream_slices[s]
        };
        if !data.is_empty() {
            crc = crc32c::crc32c_append(crc, data);
        }
    }

    // Write modified block header.
    let mut new_bh = bh.clone();
    new_bh.compressed_sizes[STREAM_CTRL] -= 1;
    new_bh.uncompressed_sizes[STREAM_CTRL] -= 1;
    new_bh.crc32 = crc;
    new_bh.write_to(output)?;

    // Write stream data (trimmed CTRL, others verbatim).
    let mut data_written = 0;
    for s in 0..stream_count {
        let data = if s == STREAM_CTRL {
            trimmed_ctrl
        } else {
            stream_slices[s]
        };
        if !data.is_empty() {
            output.write_all(data)?;
            data_written += data.len();
        }
    }

    Ok(BlockHeader::header_size(stream_count) + data_written)
}

/// Write a block with the last byte of the CTRL stream (TAG_EOF) removed.
/// For indexed files: CRC is over uncompressed data, so we must decompress
/// CTRL (if compressed), strip TAG_EOF, recompress, and recompute CRC.
fn write_block_strip_eof_indexed(
    output: &mut dyn StreamWrite,
    bh: &BlockHeader,
    payload: &[u8],
    stream_count: usize,
    methods: &[MethodSpec],
    pattern: &[u8],
) -> io::Result<usize> {
    let ctrl_size = bh.compressed_sizes[STREAM_CTRL] as usize;
    if ctrl_size == 0 {
        bh.write_to(output)?;
        output.write_all(payload)?;
        return Ok(BlockHeader::header_size(stream_count) + payload.len());
    }

    // Split payload into per-stream slices.
    let mut offset = 0;
    let mut stream_slices: Vec<&[u8]> = Vec::with_capacity(stream_count);
    for s in 0..stream_count {
        let sz = bh.compressed_sizes[s] as usize;
        stream_slices.push(&payload[offset..offset + sz]);
        offset += sz;
    }

    let ctrl_compressed = stream_slices[STREAM_CTRL];
    let ctrl_is_raw = bh.is_raw(STREAM_CTRL) || pattern[STREAM_CTRL] == 0xFF;

    // Get uncompressed CTRL, strip TAG_EOF, recompress if needed.
    let (new_ctrl_compressed, new_ctrl_uncompressed_len) = if ctrl_is_raw {
        // Raw: just strip the last byte.
        debug_assert_eq!(*ctrl_compressed.last().unwrap(), TAG_EOF);
        let trimmed = &ctrl_compressed[..ctrl_compressed.len() - 1];
        (trimmed.to_vec(), bh.uncompressed_sizes[STREAM_CTRL] - 1)
    } else {
        // Compressed: decompress, strip, recompress.
        let mut decomp = make_decompressor(methods, pattern, STREAM_CTRL)?;
        let mut uncompressed = Vec::new();
        decomp.decompress(ctrl_compressed, &mut uncompressed)?;
        debug_assert_eq!(*uncompressed.last().unwrap(), TAG_EOF);
        uncompressed.pop();
        let new_uncomp_len = uncompressed.len() as u64;

        let mut compressors =
            super::build_compressors(methods, pattern, super::compute_block_budget(methods), true)?;
        let comp = compressors[STREAM_CTRL].as_mut().unwrap();
        let mut recompressed = Vec::new();
        comp.compress(&uncompressed, &mut recompressed)?;
        comp.finish(&mut recompressed)?;
        (recompressed, new_uncomp_len)
    };

    // Recompute CRC over all uncompressed data.
    let mut crc = 0u32;
    for s in 0..stream_count {
        if bh.compressed_sizes[s] == 0 {
            continue;
        }
        if s == STREAM_CTRL {
            // Use the trimmed uncompressed CTRL data.
            if ctrl_is_raw {
                crc = crc32c::crc32c_append(crc, &new_ctrl_compressed);
            } else {
                let mut decomp = make_decompressor(methods, pattern, STREAM_CTRL)?;
                let mut buf = Vec::new();
                decomp.decompress(&new_ctrl_compressed, &mut buf)?;
                crc = crc32c::crc32c_append(crc, &buf);
            }
        } else if bh.is_raw(s) || pattern[s] == 0xFF {
            crc = crc32c::crc32c_append(crc, stream_slices[s]);
        } else {
            let mut decomp = make_decompressor(methods, pattern, s)?;
            let mut buf = Vec::new();
            decomp.decompress(stream_slices[s], &mut buf)?;
            crc = crc32c::crc32c_append(crc, &buf);
        }
    }

    // Write modified block header.
    let mut new_bh = bh.clone();
    new_bh.compressed_sizes[STREAM_CTRL] = new_ctrl_compressed.len() as u64;
    new_bh.uncompressed_sizes[STREAM_CTRL] = new_ctrl_uncompressed_len;
    new_bh.crc32 = crc;
    new_bh.write_to(output)?;

    // Write stream data.
    let mut data_written = 0;
    for s in 0..stream_count {
        let data = if s == STREAM_CTRL {
            &new_ctrl_compressed[..]
        } else {
            stream_slices[s]
        };
        if !data.is_empty() {
            output.write_all(data)?;
            data_written += data.len();
        }
    }

    Ok(BlockHeader::header_size(stream_count) + data_written)
}

/// Scan block headers in a slice, returning the offset of each block start
/// relative to `data[0]`.
fn scan_block_offsets(data: &[u8], stream_count: usize) -> io::Result<Vec<usize>> {
    let header_size = BlockHeader::header_size(stream_count);
    let mut offsets = Vec::new();
    let mut pos = 0;
    while pos + header_size <= data.len() {
        let mut hc = SliceCursor::new(&data[pos..]);
        let bh = match BlockHeader::read_from(&mut hc, stream_count) {
            Ok(bh) => bh,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };
        offsets.push(pos);
        pos += header_size + bh.total_compressed_data_size() as usize;
    }
    Ok(offsets)
}

// ---- Mmap cat ----

fn cat_mmap(
    output_path: &PathBuf,
    force: bool,
    verbose: bool,
    inputs: &[PathBuf],
    mmaps: &[MmapInput],
) -> io::Result<()> {
    let (first_fh, first_ch, header_sizes) = validate_headers(inputs, &mut |path| {
        let i = inputs.iter().position(|p| p == path).unwrap();
        let data = mmaps[i].as_slice();
        let mut cursor = SliceCursor::new(data);
        let fh = FileHeader::read_from(&mut cursor)?;
        let ch = CompressionHeader::read_from(&mut cursor)?;
        Ok((fh, ch))
    })?;

    let stream_count = first_fh.stream_count() as usize;

    if first_fh.indexed {
        cat_mmap_indexed(
            output_path,
            force,
            verbose,
            inputs,
            mmaps,
            &first_fh,
            &first_ch,
            &header_sizes,
            stream_count,
        )
    } else {
        cat_mmap_nonindexed(
            output_path,
            force,
            verbose,
            inputs,
            mmaps,
            &first_fh,
            &first_ch,
            &header_sizes,
            stream_count,
        )
    }
}

fn cat_mmap_indexed(
    output_path: &PathBuf,
    force: bool,
    verbose: bool,
    inputs: &[PathBuf],
    mmaps: &[MmapInput],
    first_fh: &FileHeader,
    first_ch: &CompressionHeader,
    header_sizes: &[u64],
    stream_count: usize,
) -> io::Result<()> {
    let mut output = open_output(Some(output_path), force)?;
    first_fh.write_to(&mut *output)?;
    first_ch.write_to(&mut *output)?;

    let mut merged_index: Option<hecate::index::MuxIndex> = None;
    let mut output_data_offset = header_sizes[0];
    let header_size = BlockHeader::header_size(stream_count);
    let last_file = mmaps.len() - 1;

    for (file_idx, mmap) in mmaps.iter().enumerate() {
        let data = mmap.as_slice();
        let data_start = header_sizes[file_idx] as usize;

        let trailer_start = data.len() - Trailer::SIZE;
        let mut tc = SliceCursor::new(&data[trailer_start..]);
        let trailer = Trailer::read_from(&mut tc)?;
        let data_end = trailer.index_offset as usize;
        let block_region = &data[data_start..data_end];

        if file_idx == last_file {
            // Last file: copy verbatim (keep TAG_EOF).
            output.write_all(block_region)?;
            output_data_offset += block_region.len() as u64;
        } else {
            // Intermediate file: strip TAG_EOF from last block.
            let offsets = scan_block_offsets(block_region, stream_count)?;
            if offsets.is_empty() {
                // No blocks -- skip.
            } else {
                let last_block_rel = *offsets.last().unwrap();

                // Copy all blocks before the last one verbatim.
                if last_block_rel > 0 {
                    output.write_all(&block_region[..last_block_rel])?;
                    output_data_offset += last_block_rel as u64;
                }

                // Parse and rewrite the last block with TAG_EOF stripped.
                let last_block_data = &block_region[last_block_rel..];
                let mut hc = SliceCursor::new(last_block_data);
                let bh = BlockHeader::read_from(&mut hc, stream_count)?;
                let payload = &last_block_data
                    [header_size..header_size + bh.total_compressed_data_size() as usize];

                let written = write_block_strip_eof_indexed(
                    &mut *output,
                    &bh,
                    payload,
                    stream_count,
                    &first_ch.methods,
                    &first_ch.pattern,
                )?;
                output_data_offset += written as u64;
            }
        }

        if trailer.index_offset > 0 {
            let mut ic = SliceCursor::new(&data[trailer.index_offset as usize..trailer_start]);
            let idx = hecate::index::reader::read_index(&mut ic)?;
            merge_index(
                &mut merged_index,
                idx,
                stream_count,
                data_start as u64,
                output_data_offset,
            );
        }
    }

    write_footer(&mut *output, &merged_index, output_data_offset)?;

    if verbose {
        eprintln!("cat: {} files merged (mmap)", inputs.len());
    }
    Ok(())
}

fn cat_mmap_nonindexed(
    output_path: &PathBuf,
    force: bool,
    verbose: bool,
    _inputs: &[PathBuf],
    mmaps: &[MmapInput],
    first_fh: &FileHeader,
    first_ch: &CompressionHeader,
    header_sizes: &[u64],
    stream_count: usize,
) -> io::Result<()> {
    let mut output = open_output(Some(output_path), force)?;
    first_fh.write_to(&mut *output)?;
    first_ch.write_to(&mut *output)?;

    let mut output_data_offset = header_sizes[0];
    let header_size = BlockHeader::header_size(stream_count);
    let last_file = mmaps.len() - 1;

    for (file_idx, mmap) in mmaps.iter().enumerate() {
        let data = mmap.as_slice();
        let data_start = header_sizes[file_idx] as usize;
        if data.len() < data_start + Trailer::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("file {} is too small to contain valid data", file_idx),
            ));
        }
        let data_end = data.len() - Trailer::SIZE;
        let block_region = &data[data_start..data_end];

        if file_idx == last_file {
            // Last file: copy verbatim (keep TAG_EOF).
            output.write_all(block_region)?;
            output_data_offset += block_region.len() as u64;
        } else {
            // Intermediate file: strip TAG_EOF from last block's CTRL stream.
            let offsets = scan_block_offsets(block_region, stream_count)?;
            if offsets.is_empty() {
                continue;
            }

            let last_block_rel = *offsets.last().unwrap();

            // Copy all blocks before the last one verbatim.
            if last_block_rel > 0 {
                output.write_all(&block_region[..last_block_rel])?;
                output_data_offset += last_block_rel as u64;
            }

            // Parse and rewrite the last block with TAG_EOF stripped.
            let last_block_data = &block_region[last_block_rel..];
            let mut hc = SliceCursor::new(last_block_data);
            let bh = BlockHeader::read_from(&mut hc, stream_count)?;
            let payload = &last_block_data
                [header_size..header_size + bh.total_compressed_data_size() as usize];

            let written = write_block_strip_eof(&mut *output, &bh, payload, stream_count)?;
            output_data_offset += written as u64;
        }
    }

    // Non-indexed: no index to write, just trailer with offset 0.
    write_footer(&mut *output, &None, output_data_offset)?;

    if verbose {
        eprintln!("cat: {} files merged (mmap, non-indexed)", mmaps.len());
    }
    Ok(())
}

// ---- Sequential cat ----

fn cat_sequential(
    output_path: &PathBuf,
    force: bool,
    verbose: bool,
    inputs: &[PathBuf],
) -> io::Result<()> {
    let (first_fh, first_ch, header_sizes) = validate_headers(inputs, &mut |path| {
        let mut f = FileInput::open(path)?;
        let fh = FileHeader::read_from(&mut f)?;
        let ch = CompressionHeader::read_from(&mut f)?;
        Ok((fh, ch))
    })?;

    let stream_count = first_fh.stream_count() as usize;

    if first_fh.indexed {
        cat_sequential_indexed(
            output_path,
            force,
            verbose,
            inputs,
            &first_fh,
            &first_ch,
            &header_sizes,
            stream_count,
        )
    } else {
        cat_sequential_nonindexed(
            output_path,
            force,
            verbose,
            inputs,
            &first_fh,
            &first_ch,
            &header_sizes,
            stream_count,
        )
    }
}

fn cat_sequential_indexed(
    output_path: &PathBuf,
    force: bool,
    verbose: bool,
    inputs: &[PathBuf],
    first_fh: &FileHeader,
    first_ch: &CompressionHeader,
    header_sizes: &[u64],
    stream_count: usize,
) -> io::Result<()> {
    let mut output = open_output(Some(output_path), force)?;
    first_fh.write_to(&mut *output)?;
    first_ch.write_to(&mut *output)?;

    let mut merged_index: Option<hecate::index::MuxIndex> = None;
    let mut output_data_offset = header_sizes[0];
    let mut copy_buf = vec![0u8; 256 * 1024];
    let header_size = BlockHeader::header_size(stream_count);
    let last_file = inputs.len() - 1;

    for (file_idx, path) in inputs.iter().enumerate() {
        let data_start = header_sizes[file_idx];
        let mut f = FileInput::open(path)?;

        f.seek(io::SeekFrom::End(-(Trailer::SIZE as i64)))?;
        let trailer = Trailer::read_from(&mut f)?;
        let data_end = trailer.index_offset;

        if file_idx == last_file {
            // Last file: copy block data verbatim.
            f.seek(io::SeekFrom::Start(data_start))?;
            let mut remaining = data_end - data_start;
            while remaining > 0 {
                let to_read = (remaining as usize).min(copy_buf.len());
                f.read_exact(&mut copy_buf[..to_read])?;
                output.write_all(&copy_buf[..to_read])?;
                remaining -= to_read as u64;
            }
            output_data_offset += data_end - data_start;
        } else {
            // Intermediate file: scan for last block, strip TAG_EOF.
            f.seek(io::SeekFrom::Start(data_start))?;
            let mut block_offsets = Vec::new();
            let mut pos = data_start;
            while pos < data_end {
                let bh = match BlockHeader::read_from(&mut f, stream_count) {
                    Ok(bh) => bh,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e),
                };
                block_offsets.push(pos);
                let skip = bh.total_compressed_data_size();
                super::skip_bytes(&mut f, skip)?;
                pos += header_size as u64 + skip;
            }

            if !block_offsets.is_empty() {
                let last_block_offset = *block_offsets.last().unwrap();

                // Copy blocks before the last one.
                if last_block_offset > data_start {
                    f.seek(io::SeekFrom::Start(data_start))?;
                    let mut remaining = last_block_offset - data_start;
                    while remaining > 0 {
                        let to_read = (remaining as usize).min(copy_buf.len());
                        f.read_exact(&mut copy_buf[..to_read])?;
                        output.write_all(&copy_buf[..to_read])?;
                        remaining -= to_read as u64;
                    }
                    output_data_offset += last_block_offset - data_start;
                }

                // Read the last block, strip TAG_EOF, write modified.
                f.seek(io::SeekFrom::Start(last_block_offset))?;
                let bh = BlockHeader::read_from(&mut f, stream_count)?;
                let payload_size = bh.total_compressed_data_size();
                let mut payload = vec![0u8; payload_size as usize];
                f.read_exact(&mut payload)?;

                let written = write_block_strip_eof_indexed(
                    &mut *output,
                    &bh,
                    &payload,
                    stream_count,
                    &first_ch.methods,
                    &first_ch.pattern,
                )?;
                output_data_offset += written as u64;
            }
        }

        if trailer.index_offset > 0 {
            f.seek(io::SeekFrom::Start(trailer.index_offset))?;
            let idx = hecate::index::reader::read_index(&mut f)?;
            merge_index(
                &mut merged_index,
                idx,
                stream_count,
                data_start,
                output_data_offset,
            );
        }
    }

    write_footer(&mut *output, &merged_index, output_data_offset)?;

    if verbose {
        eprintln!("cat: {} files merged", inputs.len());
    }
    Ok(())
}

fn cat_sequential_nonindexed(
    output_path: &PathBuf,
    force: bool,
    verbose: bool,
    inputs: &[PathBuf],
    first_fh: &FileHeader,
    first_ch: &CompressionHeader,
    header_sizes: &[u64],
    stream_count: usize,
) -> io::Result<()> {
    let mut output = open_output(Some(output_path), force)?;
    first_fh.write_to(&mut *output)?;
    first_ch.write_to(&mut *output)?;

    let mut output_data_offset = header_sizes[0];
    let mut copy_buf = vec![0u8; 256 * 1024];
    let header_size = BlockHeader::header_size(stream_count);
    let last_file = inputs.len() - 1;

    for (file_idx, path) in inputs.iter().enumerate() {
        let data_start = header_sizes[file_idx];
        let file_len = std::fs::metadata(path)?.len();
        let data_end = file_len - Trailer::SIZE as u64;
        let block_data_len = data_end - data_start;

        let mut f = FileInput::open(path)?;

        if file_idx == last_file {
            // Last file: copy block data verbatim.
            f.seek(io::SeekFrom::Start(data_start))?;
            let mut remaining = block_data_len;
            while remaining > 0 {
                let to_read = (remaining as usize).min(copy_buf.len());
                f.read_exact(&mut copy_buf[..to_read])?;
                output.write_all(&copy_buf[..to_read])?;
                remaining -= to_read as u64;
            }
            output_data_offset += block_data_len;
        } else {
            // Intermediate file: scan for last block, strip TAG_EOF.
            // Pass 1: scan block headers to find the last block offset.
            f.seek(io::SeekFrom::Start(data_start))?;
            let mut block_offsets = Vec::new();
            let mut pos = data_start;
            while pos < data_end {
                let bh = match BlockHeader::read_from(&mut f, stream_count) {
                    Ok(bh) => bh,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e),
                };
                block_offsets.push(pos);
                let skip = bh.total_compressed_data_size();
                super::skip_bytes(&mut f, skip)?;
                pos += header_size as u64 + skip;
            }

            if block_offsets.is_empty() {
                continue;
            }

            let last_block_offset = *block_offsets.last().unwrap();

            // Pass 2: copy blocks before the last one.
            if last_block_offset > data_start {
                f.seek(io::SeekFrom::Start(data_start))?;
                let mut remaining = last_block_offset - data_start;
                while remaining > 0 {
                    let to_read = (remaining as usize).min(copy_buf.len());
                    f.read_exact(&mut copy_buf[..to_read])?;
                    output.write_all(&copy_buf[..to_read])?;
                    remaining -= to_read as u64;
                }
                output_data_offset += last_block_offset - data_start;
            }

            // Read the last block fully, strip TAG_EOF, write modified.
            f.seek(io::SeekFrom::Start(last_block_offset))?;
            let bh = BlockHeader::read_from(&mut f, stream_count)?;
            let payload_size = bh.total_compressed_data_size();
            let mut payload = vec![0u8; payload_size as usize];
            f.read_exact(&mut payload)?;

            let written = write_block_strip_eof(&mut *output, &bh, &payload, stream_count)?;
            output_data_offset += written as u64;
        }
    }

    write_footer(&mut *output, &None, output_data_offset)?;

    if verbose {
        eprintln!("cat: {} files merged (non-indexed)", inputs.len());
    }
    Ok(())
}
