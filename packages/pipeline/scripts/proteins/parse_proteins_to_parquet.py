#!/usr/bin/env python3
"""
Genome Guides - Fast Protein FASTA Census & Homopolymer Analyzer
Parses protein FASTA files into Parquet files for Amino Acid Composition and Homopolymer Repeats.
Optimized version using fast C-level str.count calls and optimized regex.
"""

import sys
import argparse
import gzip
import re
from pathlib import Path
import polars as pl
import math

AMINO_ACIDS = list("ACDEFGHIKLMNPQRSTVWY")

def parse_args():
    parser = argparse.ArgumentParser(description="Parse Protein FASTA to Parquet.")
    parser.add_argument("--input", "-i", required=True, help="Input Protein FASTA (.faa, .fa, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory for track parquet files")
    return parser.parse_args()

def analyze_proteins(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    total_aa_counts = {aa: 0 for aa in AMINO_ACIDS}
    homopolymers = []
    lengths = []
    per_protein_composition = []
    richness = []
    homo_regex = re.compile(r'([ACDEFGHIKLMNPQRSTVWY])\1{4,}')

    open_fn = gzip.open if input_path.endswith(".gz") else open

    current_id = None
    seq_chunks = []

    def process_protein_record(protein_id, sequence):
        length = len(sequence)
        lengths.append((protein_id, length))
        counts = {aa: sequence.count(aa) for aa in AMINO_ACIDS}
        entropy = 0.0
        for count in counts.values():
            if count:
                p = count / max(1, length)
                entropy -= p * math.log2(p)
        distinct = sum(1 for count in counts.values() if count)
        dominant = max(counts, key=counts.get) if length else ""
        richness.append((protein_id, length, distinct, dominant,
                         float(max(counts.values(), default=0) / max(1, length) * 100.0),
                         float(entropy)))
        for aa, count in counts.items():
            per_protein_composition.append(
                (protein_id, aa, count, float(count / max(1, length) * 100.0))
            )
        for aa in AMINO_ACIDS:
            total_aa_counts[aa] += counts[aa]

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
    aa_schema = {
        "amino_acid": pl.Utf8,
        "count": pl.UInt64,
        "frequency_pct": pl.Float32
    }

    total_aa = sum(total_aa_counts.values()) or 1
    aa_rows = [
        (aa, total_aa_counts[aa], float(total_aa_counts[aa] / total_aa * 100.0))
        for aa in AMINO_ACIDS
    ]

    aa_df = pl.DataFrame(aa_rows, schema=aa_schema, orient="row")
    aa_df.write_parquet(out_p / "amino_acid_composition.parquet", compression="zstd")
    print(f"Created amino_acid_composition.parquet with {len(aa_rows)} amino acids.")

    # 2. Write Homopolymers Parquet
    homo_schema = {
        "protein_id": pl.Utf8,
        "amino_acid": pl.Utf8,
        "repeat_len": pl.UInt32,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32
    }

    if homopolymers:
        homo_df = pl.DataFrame(homopolymers, schema=homo_schema, orient="row")
    else:
        homo_df = pl.DataFrame(schema=homo_schema)

    homo_df.write_parquet(out_p / "protein_homopolymers.parquet", compression="zstd")
    print(f"Created protein_homopolymers.parquet with {len(homopolymers)} repeat tracts.")

    length_df = pl.DataFrame(lengths, schema={
        "protein_id": pl.Utf8, "length": pl.UInt32
    }, orient="row")
    length_df.write_parquet(out_p / "protein_lengths.parquet", compression="zstd")

    composition_df = pl.DataFrame(per_protein_composition, schema={
        "protein_id": pl.Utf8, "amino_acid": pl.Utf8,
        "count": pl.UInt32, "frequency_pct": pl.Float32
    }, orient="row")
    composition_df.write_parquet(out_p / "protein_composition.parquet", compression="zstd")

    richness_df = pl.DataFrame(richness, schema={
        "protein_id": pl.Utf8, "length": pl.UInt32, "distinct_amino_acids": pl.UInt8,
        "dominant_amino_acid": pl.Utf8, "dominant_frequency_pct": pl.Float32,
        "shannon_entropy": pl.Float32
    }, orient="row")
    richness_df.write_parquet(out_p / "protein_richness.parquet", compression="zstd")
    print(f"Created protein_lengths.parquet for {len(lengths)} proteins.")

if __name__ == "__main__":
    args = parse_args()
    analyze_proteins(args.input, args.out_dir)
