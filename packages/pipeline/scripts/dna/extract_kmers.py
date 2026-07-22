#!/usr/bin/env python3
"""
Genome Guides - Fast K-Mer Frequency Spectrum Generator
Counts 3-mer (codon) and 6-mer (hexamer) frequencies across contigs into Parquet format.
"""

import sys
import argparse
from pathlib import Path
from collections import Counter
import pyarrow as pa
import pyarrow.parquet as pq

def parse_args():
    parser = argparse.ArgumentParser(description="Generate K-mer Frequency Spectrum.")
    parser.add_argument("--fasta", "-f", required=True, help="Input FASTA file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    parser.add_argument("--kmer-len", "-k", type=int, default=6, help="K-mer length (e.g. 3 or 6)")
    return parser.parse_args()

def process_kmers(fasta_path: str, out_dir: str, kmer_len: int):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    counts = Counter()
    current_contig = None
    seq_chunks = []

    def count_seq(sequence):
        seq_upper = sequence.upper()
        for i in range(len(seq_upper) - kmer_len + 1):
            kmer = seq_upper[i:i+kmer_len]
            if "N" not in kmer:
                counts[kmer] += 1

    with open(fasta_path, "r") as fa_f:
        for line in fa_f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_contig and seq_chunks:
                    count_seq("".join(seq_chunks))
                    seq_chunks = []
                current_contig = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_contig and seq_chunks:
            count_seq("".join(seq_chunks))

    total = sum(counts.values()) or 1
    rows = [
        (kmer, count, float(count / total * 100.0))
        for kmer, count in counts.most_common()
    ]

    schema = pa.schema([
        ("kmer", pa.string()),
        ("count", pa.uint64()),
        ("frequency_pct", pa.float32())
    ])

    table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in rows], type=pa.string()),
            pa.array([r[1] for r in rows], type=pa.uint64()),
            pa.array([r[2] for r in rows], type=pa.float32()),
        ], schema=schema)
    ])

    out_file = out_p / f"kmers_{kmer_len}mer.parquet"
    pq.write_table(table, out_file, compression="ZSTD")
    print(f"Created kmers_{kmer_len}mer.parquet with {len(rows)} distinct {kmer_len}-mers.")

if __name__ == "__main__":
    args = parse_args()
    process_kmers(args.fasta, args.out_dir, args.kmer_len)
