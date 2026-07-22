#!/usr/bin/env python3
"""
Genome Guides - Fast CpG Island & CpG Ratio Track Generator
Computes 200bp windowed CpG ratio (Observed CpG / Expected CpG) and identifies CpG islands.
"""

import sys
import argparse
from pathlib import Path
import pyarrow as pa
import pyarrow.parquet as pq

def parse_args():
    parser = argparse.ArgumentParser(description="Extract CpG Islands & CpG Ratio Track.")
    parser.add_argument("--fasta", "-f", required=True, help="Input FASTA file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    parser.add_argument("--window", "-w", type=int, default=200, help="Window size (bp)")
    parser.add_argument("--step", "-s", type=int, default=200, help="Step size (bp)")
    return parser.parse_args()

def process_cpg(fasta_path: str, out_dir: str, window: int, step: int):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    cpg_rows = []
    current_contig = None
    seq_chunks = []

    def analyze_cpg(contig_name, sequence):
        seq_len = len(sequence)
        for i in range(0, seq_len, step):
            sub = sequence[i:i+window].upper()
            if not sub:
                continue
            l = len(sub)
            c = sub.count('C')
            g = sub.count('G')
            cpg = sub.count('CG')
            
            gc_pct = ((c + g) / l * 100.0) if l > 0 else 0.0
            expected_cpg = (c * g) / l if l > 0 else 0
            obs_exp_ratio = (cpg / expected_cpg) if expected_cpg > 0 else 0.0
            is_cpg_island = 1 if (gc_pct >= 50.0 and obs_exp_ratio >= 0.60 and cpg >= 4) else 0
            
            end_pos = min(i + window, seq_len)
            cpg_rows.append((contig_name, i, end_pos, float(gc_pct), float(obs_exp_ratio), int(cpg), is_cpg_island))

    with open(fasta_path, "r") as fa_f:
        for line in fa_f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_contig and seq_chunks:
                    full_seq = "".join(seq_chunks)
                    analyze_cpg(current_contig, full_seq)
                    seq_chunks = []
                current_contig = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_contig and seq_chunks:
            full_seq = "".join(seq_chunks)
            analyze_cpg(current_contig, full_seq)

    schema = pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("gc_pct", pa.float32()),
        ("obs_exp_ratio", pa.float32()),
        ("cpg_count", pa.uint32()),
        ("is_cpg_island", pa.uint8())
    ])

    table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in cpg_rows], type=pa.string()),
            pa.array([r[1] for r in cpg_rows], type=pa.uint32()),
            pa.array([r[2] for r in cpg_rows], type=pa.uint32()),
            pa.array([r[3] for r in cpg_rows], type=pa.float32()),
            pa.array([r[4] for r in cpg_rows], type=pa.float32()),
            pa.array([r[5] for r in cpg_rows], type=pa.uint32()),
            pa.array([r[6] for r in cpg_rows], type=pa.uint8()),
        ], schema=schema)
    ])

    pq.write_table(table, out_p / "cpg_islands.parquet", compression="ZSTD")
    print(f"Created cpg_islands.parquet with {len(cpg_rows)} window bins.")

if __name__ == "__main__":
    args = parse_args()
    process_cpg(args.fasta, args.out_dir, args.window, args.step)
