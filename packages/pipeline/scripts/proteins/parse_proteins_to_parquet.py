#!/usr/bin/env python3
"""
Genome Guides - Fast Protein FASTA Census & Homopolymer Analyzer
Parses protein FASTA files into Parquet files for Amino Acid Composition and Homopolymer Repeats.
"""

import sys
import argparse
import gzip
import re
from pathlib import Path
from collections import Counter
import pyarrow as pa
import pyarrow.parquet as pq

AMINO_ACIDS = list("ACDEFGHIKLMNPQRSTVWY")

def parse_args():
    parser = argparse.ArgumentParser(description="Parse Protein FASTA to Parquet.")
    parser.add_argument("--input", "-i", required=True, help="Input Protein FASTA (.faa, .fa, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory for track parquet files")
    return parser.parse_args()

def analyze_proteins(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    total_aa_counts = Counter()
    protein_entries = []
    homopolymers = []
    homo_regex = re.compile(r'([A-Z])\1{4,}')

    open_fn = gzip.open if input_path.endswith(".gz") else open

    current_id = None
    seq_chunks = []

    def process_protein_record(protein_id, sequence):
        length = len(sequence)
        counts = Counter(sequence)
        total_aa_counts.update(counts)
        protein_entries.append((protein_id, length))

        for m in homo_regex.finditer(sequence):
            aa = m.group(1)
            start = m.start()
            end = m.end()
            repeat_len = end - start
            homopolymers.append((protein_id, aa, repeat_len, start, end))

    with open_fn(input_path, "rt") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_id and seq_chunks:
                    full_seq = "".join(seq_chunks).upper()
                    process_protein_record(current_id, full_seq)
                    seq_chunks = []
                current_id = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_id and seq_chunks:
            full_seq = "".join(seq_chunks).upper()
            process_protein_record(current_id, full_seq)

    # 1. Write Amino Acid Composition Parquet
    aa_schema = pa.schema([
        ("amino_acid", pa.string()),
        ("count", pa.uint64()),
        ("frequency_pct", pa.float32())
    ])

    total_aa = sum(total_aa_counts.values()) or 1
    aa_rows = [
        (aa, total_aa_counts[aa], float(total_aa_counts[aa] / total_aa * 100.0))
        for aa in AMINO_ACIDS
    ]

    aa_table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in aa_rows], type=pa.string()),
            pa.array([r[1] for r in aa_rows], type=pa.uint64()),
            pa.array([r[2] for r in aa_rows], type=pa.float32()),
        ], schema=aa_schema)
    ])
    pq.write_table(aa_table, out_p / "amino_acid_composition.parquet", compression="ZSTD")
    print(f"Created amino_acid_composition.parquet with {len(aa_rows)} amino acids.")

    # 2. Write Homopolymers Parquet
    homo_schema = pa.schema([
        ("protein_id", pa.string()),
        ("amino_acid", pa.string()),
        ("repeat_len", pa.uint32()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32())
    ])

    if homopolymers:
        homo_table = pa.Table.from_batches([
            pa.RecordBatch.from_arrays([
                pa.array([r[0] for r in homopolymers], type=pa.string()),
                pa.array([r[1] for r in homopolymers], type=pa.string()),
                pa.array([r[2] for r in homopolymers], type=pa.uint32()),
                pa.array([r[3] for r in homopolymers], type=pa.uint32()),
                pa.array([r[4] for r in homopolymers], type=pa.uint32()),
            ], schema=homo_schema)
        ])
    else:
        homo_table = pa.Table.from_batches([], schema=homo_schema)

    pq.write_table(homo_table, out_p / "protein_homopolymers.parquet", compression="ZSTD")
    print(f"Created protein_homopolymers.parquet with {len(homopolymers)} repeat tracts.")

if __name__ == "__main__":
    args = parse_args()
    analyze_proteins(args.input, args.out_dir)
