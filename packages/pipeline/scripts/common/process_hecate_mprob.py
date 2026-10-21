#!/usr/bin/env python3
import argparse
from pathlib import Path
import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
from resource_args import add_resource_args, effective_memory_mb

def stream_pad_indices(contigs):
    """Stream indices of the pad entry hecate logs after each odd-length record.

    Four-bit packing stores two bases per byte, so a record of n bases yields
    n + (n mod 2) logged probabilities; the extra one is the last of the record.
    """
    ends = np.cumsum([n + n % 2 for _, n in contigs], dtype=np.int64)
    odd = np.array([n % 2 == 1 for _, n in contigs], dtype=bool)
    return ends[odd] - 1


def process_mprob(
    fai_path,
    xz_path,
    out_path,
    window_size=1000,
    chunk_windows=1000,
    symbols_per_base=1,
):
    if window_size < 1:
        raise ValueError("window size must be at least 1")
    if chunk_windows < 1:
        raise ValueError("chunk windows must be at least 1")
    if symbols_per_base < 1:
        raise ValueError("symbols per base must be at least 1")

    contigs = []
    with open(fai_path, 'r') as f:
        for line in f:
            parts = line.strip().split('\t')
            if parts:
                contigs.append((parts[0], int(parts[1])))
                
    # Polars read_csv_batched does not support .xz files natively.
    # We use pandas C-engine to stream the .xz in chunks, then process the numpy arrays.
    import pandas as pd
    try:
        chunk_iterator = pd.read_csv(
            xz_path, 
            header=None, 
            names=['prob'], 
                chunksize=window_size * chunk_windows,
            engine='c'
        )
    except (OSError, ValueError, pd.errors.ParserError) as exc:
        raise SystemExit(f"Error: Cannot read Hecate probability stream '{xz_path}': {exc}") from exc

    pad_idx = stream_pad_indices(contigs)
    stream_pos = 0
    contig_idx = 0
    pos_in_contig = 0
    pending_p = np.empty(0, dtype=float)
    pending_e = np.empty(0, dtype=float)
    pending_symbols = np.empty(0, dtype=float)
    
    arrow_schema = pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("mean_prob", pa.float32()),
        ("min_prob", pa.float32()),
        ("max_prob", pa.float32()),
        ("mean_entropy", pa.float32()),
    ])
    Path(out_path).parent.mkdir(parents=True, exist_ok=True)
    writer = pq.ParquetWriter(out_path, arrow_schema, compression="zstd")

    try:
        for chunk in chunk_iterator:
            raw_probs = chunk["prob"].to_numpy()
            in_chunk = pad_idx[(pad_idx >= stream_pos) & (pad_idx < stream_pos + len(raw_probs))]
            raw_probs = np.delete(raw_probs, in_chunk - stream_pos)
            stream_pos += len(chunk)
            if symbols_per_base > 1:
                raw_probs = np.concatenate((pending_symbols, raw_probs))
                complete_symbols = (
                    len(raw_probs) // symbols_per_base
                ) * symbols_per_base
                pending_symbols = raw_probs[complete_symbols:]
                if complete_symbols == 0:
                    continue
                probs = raw_probs[:complete_symbols].reshape(
                    -1, symbols_per_base
                ).prod(axis=1)
            else:
                probs = raw_probs

            with np.errstate(divide="ignore"):
                entropies = -np.log2(probs)
            entropies = np.clip(entropies, 0, 20.0)

            chunk_pos = 0
            chunk_len = len(probs)
            records = {
                "contig": [], "start_pos": [], "end_pos": [],
                "mean_prob": [], "min_prob": [], "max_prob": [], "mean_entropy": []
            }

            while chunk_pos < chunk_len:
                if contig_idx >= len(contigs):
                    raise ValueError("Probability stream contains more bases than the FASTA index")

                current_contig, current_contig_len = contigs[contig_idx]
                remaining_in_contig = current_contig_len - pos_in_contig
                take = min(chunk_len - chunk_pos, remaining_in_contig)

                p_slice = np.concatenate((pending_p, probs[chunk_pos:chunk_pos + take]))
                e_slice = np.concatenate((pending_e, entropies[chunk_pos:chunk_pos + take]))
                window_start = pos_in_contig - len(pending_p)
                reaches_contig_end = pos_in_contig + take >= current_contig_len
                window_count = len(p_slice) // window_size
                if reaches_contig_end and len(p_slice) % window_size:
                    window_count += 1
                used = window_count * window_size
                if reaches_contig_end and used > len(p_slice):
                    pad = used - len(p_slice)
                    p_slice = np.pad(p_slice, (0, pad), constant_values=np.nan)
                    e_slice = np.pad(e_slice, (0, pad), constant_values=np.nan)

                p_2d = p_slice[:used].reshape(-1, window_size)
                e_2d = e_slice[:used].reshape(-1, window_size)
                mean_p = np.nanmean(p_2d, axis=1)
                min_p = np.nanmin(p_2d, axis=1)
                max_p = np.nanmax(p_2d, axis=1)
                mean_e = np.nanmean(e_2d, axis=1)

                num_windows = len(mean_p)
                starts = window_start + np.arange(num_windows) * window_size
                ends = starts + window_size
                if reaches_contig_end and num_windows:
                    ends[-1] = current_contig_len

                records["contig"].extend([current_contig] * num_windows)
                records["start_pos"].extend(starts)
                records["end_pos"].extend(ends)
                records["mean_prob"].extend(mean_p)
                records["min_prob"].extend(min_p)
                records["max_prob"].extend(max_p)
                records["mean_entropy"].extend(mean_e)

                chunk_pos += take
                pos_in_contig += take
                if reaches_contig_end:
                    pending_p = np.empty(0, dtype=float)
                    pending_e = np.empty(0, dtype=float)
                    contig_idx += 1
                    pos_in_contig = 0
                else:
                    pending_p = p_slice[used:]
                    pending_e = e_slice[used:]

            if records["contig"]:
                writer.write_table(pa.table(records, schema=arrow_schema))
    finally:
        writer.close()
    if contig_idx != len(contigs) or pos_in_contig:
        raise ValueError(
            f"Probability stream ended early: {contig_idx} of {len(contigs)} contigs complete"
        )

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-f", "--fai", required=True)
    parser.add_argument("-i", "--input", required=True)
    parser.add_argument("-o", "--output", required=True)
    parser.add_argument("-w", "--window", type=int, default=1000)
    parser.add_argument(
        "--symbols-per-base",
        type=int,
        default=1,
        help="Logged Hecate probabilities per nucleotide (default: 1, see audit S1)",
    )
    parser.add_argument(
        "--chunk-windows",
        type=int,
        default=1000,
        help="Maximum windows buffered per input batch (default: 1000)",
    )
    add_resource_args(parser)
    args = parser.parse_args()
    chunk_windows = min(args.chunk_windows, max(100, effective_memory_mb(args.memory_mb) // 4))
    process_mprob(
        args.fai,
        args.input,
        args.output,
        args.window,
        chunk_windows,
        args.symbols_per_base,
    )
