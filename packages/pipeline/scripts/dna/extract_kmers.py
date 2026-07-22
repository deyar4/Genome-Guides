#!/usr/bin/env python3
"""
Genome Guides - Fast K-Mer Frequency Spectrum Generator
Counts 3-mer (codon) and 6-mer (hexamer) frequencies across contigs into Parquet format using NumPy.
"""

import sys
import argparse
from pathlib import Path
import numpy as np
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

    num_bins = 4 ** kmer_len
    global_counts = np.zeros(num_bins, dtype=np.uint64)

    # Mapping characters to 0 (A/a), 1 (C/c), 2 (G/g), 3 (T/t)
    mapping = np.zeros(256, dtype=np.uint8)
    mapping[ord('A')] = 0; mapping[ord('a')] = 0
    mapping[ord('C')] = 1; mapping[ord('c')] = 1
    mapping[ord('G')] = 2; mapping[ord('g')] = 2
    mapping[ord('T')] = 3; mapping[ord('t')] = 3

    powers = 4 ** np.arange(kmer_len - 1, -1, -1, dtype=np.uint64)

    current_contig = None
    seq_chunks = []

    def count_sequence_kmers(seq_str):
        nonlocal global_counts
        if len(seq_str) < kmer_len:
            return

        # Convert to numpy byte array (ASCII values)
        arr = np.frombuffer(seq_str.encode('ascii', errors='ignore'), dtype=np.uint8)
        
        # Check validity (only ACGT or acgt)
        is_valid_char = (arr == ord('A')) | (arr == ord('a')) | \
                         (arr == ord('C')) | (arr == ord('c')) | \
                         (arr == ord('G')) | (arr == ord('g')) | \
                         (arr == ord('T')) | (arr == ord('t'))
        
        # Check sliding windows of validity
        from numpy.lib.stride_tricks import sliding_window_view
        valid_windows = sliding_window_view(is_valid_char, kmer_len).all(axis=-1)
        
        if not np.any(valid_windows):
            return

        # Map characters to 0-3
        mapped = mapping[arr]
        windows = sliding_window_view(mapped, kmer_len)
        
        # Keep only the fully valid ACGT windows
        valid_mapped = windows[valid_windows]
        
        # Encode sliding windows into integers using dot product
        encoded = np.dot(valid_mapped.astype(np.uint64), powers)
        
        # Aggregate count spectrum
        counts = np.bincount(encoded, minlength=num_bins)
        global_counts += counts.astype(np.uint64)

    with open(fasta_path, "r") as fa_f:
        for line in fa_f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_contig and seq_chunks:
                    count_sequence_kmers("".join(seq_chunks))
                    seq_chunks = []
                current_contig = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_contig and seq_chunks:
            count_sequence_kmers("".join(seq_chunks))

    # Decode integers back to k-mer strings
    kmers = []
    for i in range(num_bins):
        val = i
        chars = []
        for _ in range(kmer_len):
            rem = val % 4
            val = val // 4
            chars.append("ACGT"[rem])
        kmers.append("".join(reversed(chars)))

    total_counts = np.sum(global_counts)
    total_counts = total_counts if total_counts > 0 else 1

    # Only include non-zero counts to keep files compact (especially for large k)
    valid_indices = global_counts > 0
    rows = []
    for idx in np.where(valid_indices)[0]:
        rows.append((kmers[idx], int(global_counts[idx]), float(global_counts[idx] / total_counts * 100.0)))
        
    rows.sort(key=lambda x: x[1], reverse=True)

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
