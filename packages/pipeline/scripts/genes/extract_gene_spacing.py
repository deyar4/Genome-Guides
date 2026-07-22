#!/usr/bin/env python3
"""
Genome Guides - Gene Spacing & Intergenic Distance Extractor
Calculates intergenic distances between consecutive genes and flags gene overlap pairs.
"""

import sys
import argparse
from pathlib import Path
import pyarrow as pa
import pyarrow.parquet as pq

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Gene Spacing & Overlaps.")
    parser.add_argument("--input", "-i", required=True, help="Input gene_models.parquet file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    return parser.parse_args()

def process_spacing(input_parquet: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    table = pq.read_table(input_parquet)
    df = table.to_pandas()

    spacing_rows = []
    overlap_rows = []

    grouped = df.groupby("contig")
    for contig, group in grouped:
        sorted_genes = group.sort_values("start_pos").to_dict("records")
        for i in range(len(sorted_genes) - 1):
            g1 = sorted_genes[i]
            g2 = sorted_genes[i + 1]

            if g2["start_pos"] > g1["end_pos"]:
                dist = g2["start_pos"] - g1["end_pos"]
                spacing_rows.append((contig, g1["gene_id"], g2["gene_id"], int(dist)))
            else:
                overlap_len = g1["end_pos"] - g2["start_pos"]
                same_strand = 1 if g1["strand"] == g2["strand"] else 0
                overlap_rows.append((contig, g1["gene_id"], g2["gene_id"], int(overlap_len), same_strand))

    # 1. Write Gene Spacing Parquet
    sp_schema = pa.schema([
        ("contig", pa.string()),
        ("gene1_id", pa.string()),
        ("gene2_id", pa.string()),
        ("intergenic_dist", pa.uint32())
    ])

    sp_table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in spacing_rows], type=pa.string()),
            pa.array([r[1] for r in spacing_rows], type=pa.string()),
            pa.array([r[2] for r in spacing_rows], type=pa.string()),
            pa.array([r[3] for r in spacing_rows], type=pa.uint32()),
        ], schema=sp_schema)
    ]) if spacing_rows else pa.Table.from_batches([], schema=sp_schema)

    pq.write_table(sp_table, out_p / "gene_spacing.parquet", compression="ZSTD")
    print(f"Created gene_spacing.parquet with {len(spacing_rows)} consecutive gene pairs.")

    # 2. Write Gene Overlaps Parquet
    ov_schema = pa.schema([
        ("contig", pa.string()),
        ("gene1_id", pa.string()),
        ("gene2_id", pa.string()),
        ("overlap_len", pa.uint32()),
        ("same_strand", pa.uint8())
    ])

    ov_table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in overlap_rows], type=pa.string()),
            pa.array([r[1] for r in overlap_rows], type=pa.string()),
            pa.array([r[2] for r in overlap_rows], type=pa.string()),
            pa.array([r[3] for r in overlap_rows], type=pa.uint32()),
            pa.array([r[4] for r in overlap_rows], type=pa.uint8()),
        ], schema=ov_schema)
    ]) if overlap_rows else pa.Table.from_batches([], schema=ov_schema)

    pq.write_table(ov_table, out_p / "gene_overlaps.parquet", compression="ZSTD")
    print(f"Created gene_overlaps.parquet with {len(overlap_rows)} overlapping gene pairs.")

if __name__ == "__main__":
    args = parse_args()
    process_spacing(args.input, args.out_dir)
