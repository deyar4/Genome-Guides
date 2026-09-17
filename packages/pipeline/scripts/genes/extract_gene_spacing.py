#!/usr/bin/env python3
"""
Genome Guides - Gene Spacing & Intergenic Distance Extractor
Calculates intergenic distances between consecutive genes and flags gene overlap pairs.
Optimized, vectorized version using polars.
"""

import sys
import argparse
from pathlib import Path
import polars as pl

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Gene Spacing & Overlaps.")
    parser.add_argument("--input", "-i", required=True, help="Input gene_models.parquet file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    return parser.parse_args()

def process_spacing(input_parquet: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    sp_schema = {
        "contig": pl.Utf8,
        "gene1_id": pl.Utf8,
        "gene2_id": pl.Utf8,
        "intergenic_dist": pl.UInt32
    }
    ov_schema = {
        "contig": pl.Utf8,
        "gene1_id": pl.Utf8,
        "gene2_id": pl.Utf8,
        "overlap_len": pl.UInt32,
        "same_strand": pl.UInt8
    }

    try:
        df = pl.read_parquet(input_parquet)
    except FileNotFoundError as exc:
        raise SystemExit(f"Error: Input Parquet not found: {input_parquet}") from exc
    except (OSError, pl.exceptions.PolarsError) as exc:
        raise SystemExit(f"Error: Could not read input Parquet '{input_parquet}': {exc}") from exc

    required_columns = {"contig", "start_pos", "end_pos", "gene_id", "strand"}
    missing = required_columns.difference(df.columns)
    if missing:
        names = ", ".join(sorted(missing))
        raise SystemExit(f"Error: Input Parquet is missing required columns: {names}")

    if df.is_empty():
        pl.DataFrame(schema=sp_schema).write_parquet(out_p / "gene_spacing.parquet", compression="zstd")
        pl.DataFrame(schema=ov_schema).write_parquet(out_p / "gene_overlaps.parquet", compression="zstd")
        print("Created empty spacing and overlap tables (empty input).")
        return

    df = df.sort(["contig", "start_pos"])

    df = df.with_columns([
        pl.col("start_pos").shift(-1).over("contig").alias("next_start_pos"),
        pl.col("gene_id").shift(-1).over("contig").alias("next_gene_id"),
        pl.col("strand").shift(-1).over("contig").alias("next_strand")
    ])

    valid_pairs = df.drop_nulls(subset=["next_start_pos", "next_gene_id"])

    spacing_df = valid_pairs.filter(pl.col("next_start_pos") > pl.col("end_pos"))
    if not spacing_df.is_empty():
        sp_df = spacing_df.with_columns(
            (pl.col("next_start_pos") - pl.col("end_pos")).cast(pl.UInt32).alias("intergenic_dist")
        ).select([
            pl.col("contig"),
            pl.col("gene_id").alias("gene1_id"),
            pl.col("next_gene_id").alias("gene2_id"),
            pl.col("intergenic_dist")
        ])
    else:
        sp_df = pl.DataFrame(schema=sp_schema)

    overlap_df = valid_pairs.filter(pl.col("next_start_pos") <= pl.col("end_pos"))
    if not overlap_df.is_empty():
        ov_df = overlap_df.with_columns([
            (pl.col("end_pos") - pl.col("next_start_pos")).cast(pl.UInt32).alias("overlap_len"),
            (pl.col("strand") == pl.col("next_strand")).cast(pl.UInt8).alias("same_strand")
        ]).select([
            pl.col("contig"),
            pl.col("gene_id").alias("gene1_id"),
            pl.col("next_gene_id").alias("gene2_id"),
            pl.col("overlap_len"),
            pl.col("same_strand")
        ])
    else:
        ov_df = pl.DataFrame(schema=ov_schema)

    sp_df.write_parquet(out_p / "gene_spacing.parquet", compression="zstd")
    ov_df.write_parquet(out_p / "gene_overlaps.parquet", compression="zstd")
    print(f"Created gene_spacing.parquet with {sp_df.height} consecutive gene pairs.")
    print(f"Created gene_overlaps.parquet with {ov_df.height} overlapping gene pairs.")

if __name__ == "__main__":
    args = parse_args()
    process_spacing(args.input, args.out_dir)
