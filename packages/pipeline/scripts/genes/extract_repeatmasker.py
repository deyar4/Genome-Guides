#!/usr/bin/env python3
"""
Genome Guides - Fast RepeatMasker Transposon Landscape Extractor
Parses UCSC RepeatMasker .out.gz file into Parquet for TE families & 100kb window densities.
"""

import sys
import argparse
import gzip
from pathlib import Path
from collections import defaultdict
import pyarrow as pa
import pyarrow.parquet as pq

def parse_args():
    parser = argparse.ArgumentParser(description="Extract RepeatMasker TE Landscape.")
    parser.add_argument("--input", "-i", required=True, help="Input RepeatMasker out.gz file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    return parser.parse_args()

def process_repeatmasker(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    te_rows = []
    density_windows = defaultdict(int)
    window_size = 100000  # 100kb

    open_fn = gzip.open if input_path.endswith(".gz") else open

    with open_fn(input_path, "rt") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("SW") or line.startswith("score"):
                continue
            parts = line.split()
            if len(parts) < 11:
                continue

            try:
                score = int(parts[0])
                div_pct = float(parts[1])
                contig = parts[4]
                start = int(parts[5]) - 1  # 0-based
                end = int(parts[6])
                strand = parts[8]
                if strand == "C":
                    strand = "-"

                te_name = parts[9]
                te_class = parts[10]

                te_rows.append((contig, start, end, strand, te_name, te_class, float(div_pct)))

                w_idx = start // window_size
                density_windows[(contig, w_idx * window_size)] += 1
            except ValueError:
                continue

    # 1. Write RepeatMasker TE Families Parquet
    te_schema = pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("strand", pa.string()),
        ("te_name", pa.string()),
        ("te_class", pa.string()),
        ("divergence_pct", pa.float32())
    ])

    te_table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in te_rows], type=pa.string()),
            pa.array([r[1] for r in te_rows], type=pa.uint32()),
            pa.array([r[2] for r in te_rows], type=pa.uint32()),
            pa.array([r[3] for r in te_rows], type=pa.string()),
            pa.array([r[4] for r in te_rows], type=pa.string()),
            pa.array([r[5] for r in te_rows], type=pa.string()),
            pa.array([r[6] for r in te_rows], type=pa.float32()),
        ], schema=te_schema)
    ]) if te_rows else pa.Table.from_batches([], schema=te_schema)

    pq.write_table(te_table, out_p / "repeatmasker_families.parquet", compression="ZSTD")
    print(f"Created repeatmasker_families.parquet with {len(te_rows)} transposable elements.")

    # 2. Write RepeatMasker Density Parquet
    d_schema = pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("te_count", pa.uint32())
    ])

    d_rows = [
        (k[0], k[1], k[1] + window_size, v)
        for k, v in density_windows.items()
    ]
    d_rows.sort(key=lambda x: (x[0], x[1]))

    d_table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in d_rows], type=pa.string()),
            pa.array([r[1] for r in d_rows], type=pa.uint32()),
            pa.array([r[2] for r in d_rows], type=pa.uint32()),
            pa.array([r[3] for r in d_rows], type=pa.uint32()),
        ], schema=d_schema)
    ]) if d_rows else pa.Table.from_batches([], schema=d_schema)

    pq.write_table(d_table, out_p / "repeatmasker_density.parquet", compression="ZSTD")
    print(f"Created repeatmasker_density.parquet with {len(d_rows)} window bins.")

if __name__ == "__main__":
    args = parse_args()
    process_repeatmasker(args.input, args.out_dir)
