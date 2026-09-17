#!/usr/bin/env python3
"""Extract amino-acid composition bias at protein termini."""

import sys
import argparse
import gzip
from pathlib import Path
import polars as pl
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args

AMINO_ACIDS = list("ACDEFGHIKLMNPQRSTVWY")

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Protein Terminal Bias.")
    parser.add_argument("--input", "-i", required=True, help="Input Protein FASTA (.faa, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    add_resource_args(parser)
    return parser.parse_args()

def analyze_terminal_bias(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    n_term_counts = {aa: 0 for aa in AMINO_ACIDS}
    c_term_counts = {aa: 0 for aa in AMINO_ACIDS}

    open_fn = gzip.open if input_path.endswith(".gz") else open

    current_id = None
    seq_chunks = []

    def process_sequence(seq):
        if len(seq) >= 40:
            n_seq = seq[:20]
            c_seq = seq[-20:]
            for aa in AMINO_ACIDS:
                n_term_counts[aa] += n_seq.count(aa)
                c_term_counts[aa] += c_seq.count(aa)

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

    schema = {
        "amino_acid": pl.Utf8,
        "n_terminal_count": pl.UInt64,
        "n_terminal_pct": pl.Float32,
        "c_terminal_count": pl.UInt64,
        "c_terminal_pct": pl.Float32,
        "n_to_c_bias_ratio": pl.Float32
    }

    df = pl.DataFrame(rows, schema=schema, orient="row")
    df.write_parquet(out_p / "protein_terminal_bias.parquet", compression="zstd")
    print(f"Created protein_terminal_bias.parquet with {len(rows)} amino acids.")

if __name__ == "__main__":
    args = parse_args()
    analyze_terminal_bias(args.input, args.out_dir)
