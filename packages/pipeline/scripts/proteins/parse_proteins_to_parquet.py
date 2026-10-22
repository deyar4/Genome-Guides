#!/usr/bin/env python3
"""Extract protein composition and homopolymer tracks from FASTA."""

import sys
import argparse
import gzip
import re
from pathlib import Path
import polars as pl
import math
import pyarrow as pa
import pyarrow.parquet as pq
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args

AMINO_ACIDS = list("ACDEFGHIKLMNPQRSTVWY")

def parse_args():
    parser = argparse.ArgumentParser(description="Parse Protein FASTA to Parquet.")
    parser.add_argument("--input", "-i", required=True, help="Input Protein FASTA (.faa, .fa, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory for track parquet files")
    add_resource_args(parser)
    return parser.parse_args()

def analyze_proteins(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    total_aa_counts = {aa: 0 for aa in AMINO_ACIDS}
    homo_regex = re.compile(r'([ACDEFGHIKLMNPQRSTVWY])\1{4,}')

    open_fn = gzip.open if input_path.endswith(".gz") else open

    current_id = None
    seq_chunks = []
    schemas = {
        "homopolymers": pa.schema([
            ("protein_id", pa.string()), ("amino_acid", pa.string()),
            ("repeat_len", pa.uint32()), ("start_pos", pa.uint32()), ("end_pos", pa.uint32()),
        ]),
        "lengths": pa.schema([("protein_id", pa.string()), ("length", pa.uint32())]),
        "richness": pa.schema([
            ("protein_id", pa.string()), ("length", pa.uint32()),
            ("distinct_amino_acids", pa.uint8()), ("dominant_amino_acid", pa.string()),
            ("dominant_frequency_pct", pa.float32()), ("shannon_entropy", pa.float32()),
        ]),
    }
    writers = {
        name: pq.ParquetWriter(out_p / f"protein_{name}.parquet", schema, compression="zstd")
        for name, schema in schemas.items()
    }
    batch_rows = {name: [] for name in schemas}
    protein_count = 0

    def process_protein_record(protein_id, sequence):
        nonlocal protein_count
        length = len(sequence)
        protein_count += 1
        batch_rows["lengths"].append((protein_id, length))
        counts = {aa: sequence.count(aa) for aa in AMINO_ACIDS}
        entropy = 0.0
        for count in counts.values():
            if count:
                p = count / max(1, length)
                entropy -= p * math.log2(p)
        distinct = sum(1 for count in counts.values() if count)
        dominant = max(counts, key=counts.get) if length else ""
        batch_rows["richness"].append((
            protein_id, length, distinct, dominant,
            float(max(counts.values(), default=0) / max(1, length) * 100.0),
            float(entropy),
        ))
        for aa in AMINO_ACIDS:
            total_aa_counts[aa] += counts[aa]

        for m in homo_regex.finditer(sequence):
            aa = m.group(1)
            start = m.start()
            end = m.end()
            repeat_len = end - start
            batch_rows["homopolymers"].append((protein_id, aa, repeat_len, start, end))
        if protein_count % 4096 == 0:
            flush_batches()

    def flush_batches():
        for name, rows in batch_rows.items():
            if rows:
                writers[name].write_table(pa.Table.from_pylist([
                    dict(zip(schemas[name].names, row)) for row in rows
                ], schema=schemas[name]))
                rows.clear()

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
    flush_batches()
    for writer in writers.values():
        writer.close()


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

    print(f"Created protein_homopolymers.parquet and protein_lengths.parquet for {protein_count} proteins.")

if __name__ == "__main__":
    args = parse_args()
    analyze_proteins(args.input, args.out_dir)
