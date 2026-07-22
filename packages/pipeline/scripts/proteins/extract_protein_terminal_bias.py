#!/usr/bin/env python3
"""
Genome Guides - Protein Terminal Bias Extractor
Analyzes N-terminal (first 20 AA) vs C-terminal (last 20 AA) amino acid composition bias.
"""

import sys
import argparse
import gzip
from pathlib import Path
from collections import Counter
import pyarrow as pa
import pyarrow.parquet as pq

AMINO_ACIDS = list("ACDEFGHIKLMNPQRSTVWY")

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Protein Terminal Bias.")
    parser.add_argument("--input", "-i", required=True, help="Input Protein FASTA (.faa, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    return parser.parse_args()

def analyze_terminal_bias(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    n_term_counts = Counter()
    c_term_counts = Counter()

    open_fn = gzip.open if input_path.endswith(".gz") else open

    current_id = None
    seq_chunks = []

    def process_sequence(seq):
        if len(seq) >= 40:
            n_term_counts.update(seq[:20])
            c_term_counts.update(seq[-20:])

    with open_fn(input_path, "rt") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_id and seq_chunks:
                    process_sequence("".join(seq_chunks).upper())
                    seq_chunks = []
                current_id = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_id and seq_chunks:
            process_sequence("".join(seq_chunks).upper())

    total_n = sum(n_term_counts.values()) or 1
    total_c = sum(c_term_counts.values()) or 1

    rows = []
    for aa in AMINO_ACIDS:
        n_pct = float(n_term_counts[aa] / total_n * 100.0)
        c_pct = float(c_term_counts[aa] / total_c * 100.0)
        bias_ratio = float(n_pct / c_pct) if c_pct > 0 else 0.0
        rows.append((aa, n_term_counts[aa], float(n_pct), c_term_counts[aa], float(c_pct), float(bias_ratio)))

    schema = pa.schema([
        ("amino_acid", pa.string()),
        ("n_terminal_count", pa.uint64()),
        ("n_terminal_pct", pa.float32()),
        ("c_terminal_count", pa.uint64()),
        ("c_terminal_pct", pa.float32()),
        ("n_to_c_bias_ratio", pa.float32())
    ])

    table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in rows], type=pa.string()),
            pa.array([r[1] for r in rows], type=pa.uint64()),
            pa.array([r[2] for r in rows], type=pa.float32()),
            pa.array([r[3] for r in rows], type=pa.uint64()),
            pa.array([r[4] for r in rows], type=pa.float32()),
            pa.array([r[5] for r in rows], type=pa.float32()),
        ], schema=schema)
    ])

    pq.write_table(table, out_p / "protein_terminal_bias.parquet", compression="ZSTD")
    print(f"Created protein_terminal_bias.parquet with {len(rows)} amino acids.")

if __name__ == "__main__":
    args = parse_args()
    analyze_terminal_bias(args.input, args.out_dir)
