#!/usr/bin/env python3
"""
Genome Guides - Fast Shannon Entropy & Telomeric/SSR Repeat Extractor
Computes 1kb windowed sequence Shannon Entropy H(W) and locates Telomeric (TTAGGG) & SSR repeat tracts.
"""

import sys
import argparse
import math
from pathlib import Path
from collections import Counter
import pyarrow as pa
import pyarrow.parquet as pq

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Shannon Entropy & Telomeric/SSR Repeats.")
    parser.add_argument("--fasta", "-f", required=True, help="Input FASTA file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    parser.add_argument("--window", "-w", type=int, default=1000, help="Window size (bp)")
    parser.add_argument("--step", "-s", type=int, default=1000, help="Step size (bp)")
    return parser.parse_args()

def calculate_shannon_entropy(seq_str):
    l = len(seq_str)
    if l == 0:
        return 0.0
    counts = Counter(seq_str)
    entropy = 0.0
    for base in "ACGT":
        p = counts[base] / l
        if p > 0:
            entropy -= p * math.log2(p)
    return float(entropy)

def process_entropy(fasta_path: str, out_dir: str, window: int, step: int):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    entropy_rows = []
    telo_motif = "TTAGGG"
    telo_rev = "CCCTAA"

    current_contig = None
    seq_chunks = []

    def analyze_contig(contig_name, sequence):
        seq_len = len(sequence)
        for i in range(0, seq_len, step):
            sub = sequence[i:i+window].upper()
            if not sub:
                continue
            l = len(sub)
            end_pos = min(i + window, seq_len)
            
            non_n_seq = sub.replace("N", "")
            h_score = calculate_shannon_entropy(non_n_seq)
            telo_count = sub.count(telo_motif) + sub.count(telo_rev)
            
            entropy_rows.append((contig_name, i, end_pos, float(h_score), int(telo_count)))

    with open(fasta_path, "r") as fa_f:
        for line in fa_f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_contig and seq_chunks:
                    full_seq = "".join(seq_chunks)
                    analyze_contig(current_contig, full_seq)
                    seq_chunks = []
                current_contig = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_contig and seq_chunks:
            full_seq = "".join(seq_chunks)
            analyze_contig(current_contig, full_seq)

    schema = pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("shannon_entropy", pa.float32()),
        ("telomeric_repeats", pa.uint32())
    ])

    table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in entropy_rows], type=pa.string()),
            pa.array([r[1] for r in entropy_rows], type=pa.uint32()),
            pa.array([r[2] for r in entropy_rows], type=pa.uint32()),
            pa.array([r[3] for r in entropy_rows], type=pa.float32()),
            pa.array([r[4] for r in entropy_rows], type=pa.uint32()),
        ], schema=schema)
    ])

    pq.write_table(table, out_p / "shannon_entropy.parquet", compression="ZSTD")
    print(f"Created shannon_entropy.parquet with {len(entropy_rows)} window bins.")

if __name__ == "__main__":
    args = parse_args()
    process_entropy(args.fasta, args.out_dir, args.window, args.step)
