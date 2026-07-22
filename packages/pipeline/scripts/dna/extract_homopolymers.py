#!/usr/bin/env python3
"""
Genome Guides - Genomic Homopolymer Run Extractor
Scans genome for long single-nucleotide homopolymers (>= 10 consecutive bases).
"""

import sys
import argparse
import re
from pathlib import Path
import pyarrow as pa
import pyarrow.parquet as pq

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Genomic Homopolymers.")
    parser.add_argument("--fasta", "-f", required=True, help="Input FASTA file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    parser.add_argument("--min-len", "-m", type=int, default=10, help="Minimum run length")
    return parser.parse_args()

def process_homopolymers(fasta_path: str, out_dir: str, min_len: int):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    rows = []
    pattern = re.compile(rf'([A-Z])\1{{{min_len-1},}}')

    current_contig = None
    seq_chunks = []

    def scan_contig(contig_name, sequence):
        seq_u = sequence.upper()
        for m in pattern.finditer(seq_u):
            base = m.group(1)
            if base in "ACGT":
                start = m.start()
                end = m.end()
                run_len = end - start
                rows.append((contig_name, start, end, base, run_len))

    with open(fasta_path, "r") as fa_f:
        for line in fa_f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_contig and seq_chunks:
                    scan_contig(current_contig, "".join(seq_chunks))
                    seq_chunks = []
                current_contig = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_contig and seq_chunks:
            scan_contig(current_contig, "".join(seq_chunks))

    schema = pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("base", pa.string()),
        ("run_length", pa.uint32())
    ])

    table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in rows], type=pa.string()),
            pa.array([r[1] for r in rows], type=pa.uint32()),
            pa.array([r[2] for r in rows], type=pa.uint32()),
            pa.array([r[3] for r in rows], type=pa.string()),
            pa.array([r[4] for r in rows], type=pa.uint32()),
        ], schema=schema)
    ]) if rows else pa.Table.from_batches([], schema=schema)

    pq.write_table(table, out_p / "homopolymer_genomic_tracts.parquet", compression="ZSTD")
    print(f"Created homopolymer_genomic_tracts.parquet with {len(rows)} homopolymer runs (>= {min_len} bp).")

if __name__ == "__main__":
    args = parse_args()
    process_homopolymers(args.fasta, args.out_dir, args.min_len)
