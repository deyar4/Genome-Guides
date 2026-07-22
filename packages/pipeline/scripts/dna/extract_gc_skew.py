#!/usr/bin/env python3
"""
Genome Guides - Windowed GC Content & GC Skew Generator
Calculates windowed GC % and GC Skew ((G - C) / (G + C)) across chromosomes.
"""

import sys
import argparse
from pathlib import Path

def parse_args():
    parser = argparse.ArgumentParser(description="Compute Windowed GC Content & GC Skew.")
    parser.add_argument("--fasta", "-f", required=True, help="Input FASTA file")
    parser.add_argument("--out", "-o", required=True, help="Output TSV file")
    parser.add_argument("--window", "-w", type=int, default=1000, help="Window size (bp)")
    parser.add_argument("--step", "-s", type=int, default=1000, help="Step size (bp)")
    return parser.parse_args()

def process_gc_skew(fasta_path: str, out_tsv: str, window: int, step: int):
    out_p = Path(out_tsv)
    out_p.parent.mkdir(parents=True, exist_ok=True)

    current_contig = None
    seq_chunks = []

    with open(fasta_path, "r") as fa_f, open(out_tsv, "w") as out_f:
        def process_contig(contig_name, sequence):
            seq_len = len(sequence)
            for i in range(0, seq_len, step):
                sub = sequence[i:i+window].upper()
                if not sub:
                    continue
                g = sub.count('G')
                c = sub.count('C')
                a = sub.count('A')
                t = sub.count('T')
                total_gc = g + c
                total_valid = total_gc + a + t

                gc_pct = (total_gc / total_valid * 100.0) if total_valid > 0 else 0.0
                gc_skew = ((g - c) / total_gc) if total_gc > 0 else 0.0
                end_pos = min(i + window, seq_len)

                out_f.write(f"{contig_name}\t{i}\t{end_pos}\t{gc_pct:.4f}\t{gc_skew:.4f}\n")

        for line in fa_f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_contig and seq_chunks:
                    process_contig(current_contig, "".join(seq_chunks))
                    seq_chunks = []
                current_contig = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_contig and seq_chunks:
            process_contig(current_contig, "".join(seq_chunks))

if __name__ == "__main__":
    args = parse_args()
    process_gc_skew(args.fasta, args.out, args.window, args.step)
