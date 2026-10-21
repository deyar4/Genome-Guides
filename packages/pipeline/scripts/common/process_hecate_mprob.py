#!/usr/bin/env python3
import argparse
from pathlib import Path
import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq
import sys
import warnings
sys.path.insert(0, str(Path(__file__).resolve().parent))
from resource_args import add_resource_args, effective_memory_mb

def expected_model_class(first_block_len):
    """EXPECTED model machine from the first block length, using the dispatch in upstream 76b4836 (markov_codec.rs `dispatch`:
    < 200,000 -> vmarkov; < 3/4 * 1 MiB block budget -> bmarkov; else markov). The unit and the exact first-block length
    that hecate sees are not verified here (audit 03 S3): this is a log line for humans, not a measurement of the run."""
    if first_block_len < 200_000:
        return "vmarkov"
    if first_block_len < (1 << 20) * 3 // 4:
        return "bmarkov"
    return "markov"


def check_model_class(contigs, pin):
    """Fail BEFORE the long run when the expected model class differs from the pinned one (audit 03 S3, D40).
    pin None or "any" disables the check (small test genomes). The expectation is a proxy (first record, capped at one block
    budget), not a measurement of what hecate chose; test_hecate_model_context.py-style evidence: the same 100 kb gives
    different probabilities alone and as the head of a 1.5 Mb file (2026-10-21, see LOGBOOK), so classes are not comparable."""
    if pin in (None, "any"):
        return
    first_len = contigs[0][1] if contigs else 0
    got = expected_model_class(min(first_len, 1 << 20))
    if got != pin:
        raise SystemExit(
            f"Model class mismatch: pinned '{pin}' but the first record ({first_len} bases) is expected to give '{got}'. "
            f"Probabilities from different classes are not comparable (audit 03 S3). Fix the config pin "
            f"tracks.hecate_bwt.model_class or the input order."
        )


def stream_pad_indices(contigs):
    """Stream indices of the pad entry hecate logs after each odd-length record.

    Four-bit packing stores two bases per byte, so a record of n bases yields
    n + (n mod 2) logged probabilities; the extra one is the last of the record.
    """
    ends = np.cumsum([n + n % 2 for _, n in contigs], dtype=np.int64)
    odd = np.array([n % 2 == 1 for _, n in contigs], dtype=bool)
    return ends[odd] - 1


def load_masks(bed_path):
    """Read 0-based half-open BED intervals of non-ACGT bases into {contig: (starts, ends)}."""
    raw = {}
    if bed_path:
        with open(bed_path) as f:
            for line in f:
                parts = line.split('\t')
                if len(parts) >= 3:
                    raw.setdefault(parts[0], []).append((int(parts[1]), int(parts[2])))
    return {
        c: (np.array([a for a, _ in sorted(v)]), np.array([b for _, b in sorted(v)]))
        for c, v in raw.items()
    }


def mask_nonacgt(values, masks, contig, offset):
    """Return a copy of values (contig positions offset..offset+len) with non-ACGT bases set to NaN."""
    if contig not in masks:
        return values
    starts, ends = masks[contig]
    lo, hi = offset, offset + len(values)
    out = values.copy()
    first = np.searchsorted(ends, lo, side='right')
    for a, b in zip(starts[first:], ends[first:]):
        if a >= hi:
            break
        out[max(a, lo) - lo:min(b, hi) - lo] = np.nan
    return out


def process_mprob(
    fai_path,
    xz_path,
    out_path,
    window_size=1000,
    chunk_windows=1000,
    symbols_per_base=1,
    nonacgt_bed=None,
    model_class_pin=None,
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
    check_model_class(contigs, model_class_pin)

    masks = load_masks(nonacgt_bed)
    stats = {"bases": 0, "masked": 0, "prob_zero": 0}
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
        ("n_valid", pa.uint32()),
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
                "mean_prob": [], "min_prob": [], "max_prob": [], "mean_entropy": [],
                "n_valid": []
            }

            while chunk_pos < chunk_len:
                if contig_idx >= len(contigs):
                    raise ValueError("Probability stream contains more bases than the FASTA index")

                current_contig, current_contig_len = contigs[contig_idx]
                remaining_in_contig = current_contig_len - pos_in_contig
                take = min(chunk_len - chunk_pos, remaining_in_contig)

                p_part = mask_nonacgt(probs[chunk_pos:chunk_pos + take], masks, current_contig, pos_in_contig)
                e_part = mask_nonacgt(entropies[chunk_pos:chunk_pos + take], masks, current_contig, pos_in_contig)
                stats["bases"] += len(p_part)
                stats["masked"] += int(np.isnan(p_part).sum())
                stats["prob_zero"] += int((p_part == 0).sum())
                p_slice = np.concatenate((pending_p, p_part))
                e_slice = np.concatenate((pending_e, e_part))
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
                with warnings.catch_warnings():
                    # Windows with no valid base are NaN by design (counted in n_valid = 0).
                    warnings.simplefilter("ignore", RuntimeWarning)
                    mean_p = np.nanmean(p_2d, axis=1)
                    min_p = np.nanmin(p_2d, axis=1)
                    max_p = np.nanmax(p_2d, axis=1)
                    mean_e = np.nanmean(e_2d, axis=1)
                n_valid = (~np.isnan(p_2d)).sum(axis=1)

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
                records["n_valid"].extend(n_valid)

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
                # NaN -> NULL so that DuckDB aggregates in the API skip missing windows instead of returning NaN.
                columns = [
                    pa.array(records[f.name], type=f.type, from_pandas=True)
                    for f in arrow_schema
                ]
                writer.write_table(pa.table(columns, schema=arrow_schema))
    finally:
        writer.close()
    if contig_idx != len(contigs) or pos_in_contig:
        raise ValueError(
            f"Probability stream ended early: {contig_idx} of {len(contigs)} contigs complete"
        )
    # The model class changes what the numbers mean (audit 03 S3), so every run records which one to expect. The first block
    # is at most one block budget (1 MiB) of the first record; whether the muxer cuts exactly there is NOT verified.
    first_len = contigs[0][1] if contigs else 0
    print(
        f"model class expected from the first record ({first_len} bases): {expected_model_class(min(first_len, 1 << 20))} "
        f"(T2T must say markov; vmarkov/bmarkov streams are not comparable to the full run)",
        file=sys.stderr,
    )
    valid = stats["bases"] - stats["masked"]
    print(
        f"hecate probabilities: bases={stats['bases']} masked_nonacgt={stats['masked']} valid={valid} "
        f"prob_zero={stats['prob_zero']} (printed 0.00000, i.e. p < 5e-6; surprisal clipped to 20 bits) "
        f"fraction_zero={stats['prob_zero'] / max(valid, 1):.3e}",
        file=sys.stderr,
    )
    return stats

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
    parser.add_argument(
        "--nonacgt-bed",
        help="BED (0-based, half-open) of non-ACGT runs; those bases get NaN and are excluded from window statistics",
    )
    parser.add_argument(
        "--model-class-pin",
        default="any",
        help="Required expected model class (markov, bmarkov, vmarkov) or 'any' to skip the check (default: any)",
    )
    parser.add_argument("--check-only", action="store_true", help="Only run the model-class check on the .fai, then exit")
    add_resource_args(parser)
    args = parser.parse_args()
    if args.check_only:
        with open(args.fai) as handle:
            first = [line.split("\t") for line in handle if line.strip()][:1]
        check_model_class([(r[0], int(r[1])) for r in first], args.model_class_pin)
        sys.exit(0)
    chunk_windows = min(args.chunk_windows, max(100, effective_memory_mb(args.memory_mb) // 4))
    process_mprob(
        args.fai,
        args.input,
        args.output,
        args.window,
        chunk_windows,
        args.symbols_per_base,
        args.nonacgt_bed,
        args.model_class_pin,
    )
