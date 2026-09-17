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
import polars as pl
import pyarrow as pa
import pyarrow.parquet as pq
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args

def parse_args():
    parser = argparse.ArgumentParser(description="Extract RepeatMasker TE Landscape.")
    parser.add_argument("--input", "-i", required=True, help="Input RepeatMasker out.gz file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    add_resource_args(parser)
    return parser.parse_args()

def process_repeatmasker(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    density_windows = defaultdict(int)
    window_size = 100000  # 100kb
    te_schema = pa.schema([
        ("contig", pa.string()), ("start_pos", pa.uint32()), ("end_pos", pa.uint32()),
        ("strand", pa.string()), ("te_name", pa.string()), ("te_class", pa.string()),
        ("divergence_pct", pa.float32()),
    ])
    te_path = out_p / "repeatmasker_families.parquet"
    te_writer = pq.ParquetWriter(te_path, te_schema, compression="zstd")
    batch = []
    record_count = 0

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

                batch.append((contig, start, end, strand, te_name, te_class, float(div_pct)))
                record_count += 1
                if len(batch) >= 65_536:
                    te_writer.write_table(pa.Table.from_pylist([
                        dict(zip(te_schema.names, row)) for row in batch
                    ], schema=te_schema))
                    batch.clear()

                w_idx = start // window_size
                density_windows[(contig, w_idx * window_size)] += 1
            except ValueError:
                continue
    if batch:
        te_writer.write_table(pa.Table.from_pylist([
            dict(zip(te_schema.names, row)) for row in batch
        ], schema=te_schema))
    te_writer.close()

    print(f"Created repeatmasker_families.parquet with {record_count} transposable elements.")

    # 2. Write RepeatMasker Density Parquet
    d_schema = {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "te_count": pl.UInt32
    }

    d_rows = [
        (k[0], k[1], k[1] + window_size, v)
        for k, v in density_windows.items()
    ]
    d_rows.sort(key=lambda x: (x[0], x[1]))

    if d_rows:
        d_df = pl.DataFrame(d_rows, schema=d_schema, orient="row")
    else:
        d_df = pl.DataFrame(schema=d_schema)

    d_df.write_parquet(out_p / "repeatmasker_density.parquet", compression="zstd")
    print(f"Created repeatmasker_density.parquet with {len(d_rows)} window bins.")

if __name__ == "__main__":
    args = parse_args()
    process_repeatmasker(args.input, args.out_dir)
