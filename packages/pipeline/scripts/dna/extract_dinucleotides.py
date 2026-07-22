#!/usr/bin/env python3
"""
Genome Guides - Dinucleotide Frequency & Odds Ratio Extractor
Calculates all 16 dinucleotide frequencies and odds ratios using fast FASTA splitting and NumPy.
"""

import sys
import argparse
from pathlib import Path
import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

BASES = list("ACGT")
DINUCLEOTIDES = [b1 + b2 for b1 in BASES for b2 in BASES]

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Dinucleotide Frequencies & Odds Ratios.")
    parser.add_argument("--fasta", "-f", required=True, help="Input FASTA file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    return parser.parse_args()

def process_dinucleotides(fasta_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    single_counts = np.zeros(4, dtype=np.uint64)
    dinucl_counts = np.zeros(16, dtype=np.uint64)

    # Mapping characters to 0 (A/a), 1 (C/c), 2 (G/g), 3 (T/t)
    mapping = np.zeros(256, dtype=np.uint8)
    mapping[ord('A')] = 0; mapping[ord('a')] = 0
    mapping[ord('C')] = 1; mapping[ord('c')] = 1
    mapping[ord('G')] = 2; mapping[ord('g')] = 2
    mapping[ord('T')] = 3; mapping[ord('t')] = 3

    def analyze_seq(seq_str):
        nonlocal single_counts, dinucl_counts
        if len(seq_str) < 2:
            if len(seq_str) == 1 and seq_str[0].upper() in BASES:
                idx = "ACGT".index(seq_str[0].upper())
                single_counts[idx] += 1
            return

        arr = np.frombuffer(seq_str.encode('ascii', errors='ignore'), dtype=np.uint8)
        
        is_valid_char = (arr == ord('A')) | (arr == ord('a')) | \
                         (arr == ord('C')) | (arr == ord('c')) | \
                         (arr == ord('G')) | (arr == ord('g')) | \
                         (arr == ord('T')) | (arr == ord('t'))
        
        valid_singles = arr[is_valid_char]
        if len(valid_singles) > 0:
            mapped_singles = mapping[valid_singles]
            single_counts += np.bincount(mapped_singles, minlength=4).astype(np.uint64)
            
        from numpy.lib.stride_tricks import sliding_window_view
        valid_windows = sliding_window_view(is_valid_char, 2).all(axis=-1)
        
        if np.any(valid_windows):
            mapped_all = mapping[arr]
            windows = sliding_window_view(mapped_all, 2)
            valid_mapped = windows[valid_windows]
            
            encoded = valid_mapped[:, 0].astype(np.uint64) * 4 + valid_mapped[:, 1].astype(np.uint64)
            dinucl_counts += np.bincount(encoded, minlength=16).astype(np.uint64)

    # Read the whole FASTA file at once and split by '>' (extremely fast I/O)
    with open(fasta_path, "r") as fa_f:
        data = fa_f.read()

    parts = data.split(">")
    for part in parts:
        if not part:
            continue
        lines = part.split("\n", 1)
        if len(lines) < 2:
            continue
        seq_str = lines[1].replace("\n", "")
        analyze_seq(seq_str)

    total_single = np.sum(single_counts) or 1
    total_dinucl = np.sum(dinucl_counts) or 1

    rows = []
    for i, din in enumerate(DINUCLEOTIDES):
        b1_idx = i // 4
        b2_idx = i % 4
        
        f_din = dinucl_counts[i] / total_dinucl
        f_b1 = single_counts[b1_idx] / total_single
        f_b2 = single_counts[b2_idx] / total_single
        
        odds_ratio = (f_din / (f_b1 * f_b2)) if (f_b1 * f_b2) > 0 else 0.0
        pct = f_din * 100.0
        
        rows.append((din, int(dinucl_counts[i]), float(pct), float(odds_ratio)))

    schema = pa.schema([
        ("dinucleotide", pa.string()),
        ("count", pa.uint64()),
        ("frequency_pct", pa.float32()),
        ("odds_ratio", pa.float32())
    ])

    table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in rows], type=pa.string()),
            pa.array([r[1] for r in rows], type=pa.uint64()),
            pa.array([r[2] for r in rows], type=pa.float32()),
            pa.array([r[3] for r in rows], type=pa.float32()),
        ], schema=schema)
    ])

    pq.write_table(table, out_p / "dinucleotide_frequencies.parquet", compression="ZSTD")
    print(f"Created dinucleotide_frequencies.parquet for all 16 dinucleotides.")

if __name__ == "__main__":
    args = parse_args()
    process_dinucleotides(args.fasta, args.out_dir)
