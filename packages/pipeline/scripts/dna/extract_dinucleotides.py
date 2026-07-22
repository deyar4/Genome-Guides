#!/usr/bin/env python3
"""
Genome Guides - Dinucleotide Frequency & Odds Ratio Extractor
Calculates all 16 dinucleotide frequencies and odds ratios rho_XY = f_XY / (f_X * f_Y).
"""

import sys
import argparse
from pathlib import Path
from collections import Counter
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

    single_counts = Counter()
    dinucl_counts = Counter()

    current_contig = None
    seq_chunks = []

    def analyze_seq(seq):
        seq_u = seq.upper()
        l = len(seq_u)
        for i in range(l - 1):
            b1 = seq_u[i]
            b2 = seq_u[i+1]
            if b1 in BASES:
                single_counts[b1] += 1
            din = b1 + b2
            if din in DINUCLEOTIDES:
                dinucl_counts[din] += 1
        if l > 0 and seq_u[-1] in BASES:
            single_counts[seq_u[-1]] += 1

    with open(fasta_path, "r") as fa_f:
        for line in fa_f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_contig and seq_chunks:
                    analyze_seq("".join(seq_chunks))
                    seq_chunks = []
                current_contig = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_contig and seq_chunks:
            analyze_seq("".join(seq_chunks))

    total_single = sum(single_counts.values()) or 1
    total_dinucl = sum(dinucl_counts.values()) or 1

    rows = []
    for din in DINUCLEOTIDES:
        b1, b2 = din[0], din[1]
        f_din = dinucl_counts[din] / total_dinucl
        f_b1 = single_counts[b1] / total_single
        f_b2 = single_counts[b2] / total_single
        
        odds_ratio = (f_din / (f_b1 * f_b2)) if (f_b1 * f_b2) > 0 else 0.0
        pct = f_din * 100.0
        
        rows.append((din, dinucl_counts[din], float(pct), float(odds_ratio)))

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
