#!/usr/bin/env python3
"""
Genome Guides - Universal TSV to ZSTD Compressed Parquet Converter
"""

import sys
import argparse
import polars as pl
from pathlib import Path

# We define the column names and types for each schema
SCHEMAS = {
    "gc_content": {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "gc_pct": pl.Float32,
        "gc_skew": pl.Float32,
        "a_count": pl.UInt32,
        "c_count": pl.UInt32,
        "g_count": pl.UInt32,
        "t_count": pl.UInt32,
        "n_count": pl.UInt32,
        "a_pct": pl.Float32,
        "c_pct": pl.Float32,
        "g_pct": pl.Float32,
        "t_pct": pl.Float32,
        "n_pct": pl.Float32
    },
    "assembly_gaps": {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "gap_length": pl.UInt32,
        "gap_type": pl.Utf8
    },
    "sequence_complexity": {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "shannon_entropy": pl.Float32,
        "repeats_count": pl.UInt32
    },
    "hecate_bwt": {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "uniqueness_score": pl.Float32,
        "bwt_entropy": pl.Float32
    },
    "homopolymers": {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "base": pl.Utf8,
        "run_length": pl.UInt32
    }
}

def parse_args():
    parser = argparse.ArgumentParser(description="Convert TSV genomic track to Parquet")
    parser.add_argument("--input", "-i", required=True, help="Input TSV file")
    parser.add_argument("--output", "-o", required=True, help="Output Parquet file")
    parser.add_argument("--schema", "-s", required=True, choices=list(SCHEMAS.keys()), help="Schema name")
    return parser.parse_args()

def convert_tsv_to_parquet(input_tsv: str, output_parquet: str, schema_name: str):
    schema = SCHEMAS[schema_name]
    out_path = Path(output_parquet)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    input_path = Path(input_tsv)
    if not input_path.exists():
        raise SystemExit(f"Error: Input TSV not found: {input_tsv}")
    if input_path.stat().st_size == 0:
        df = pl.DataFrame(schema=schema)
        df.write_parquet(output_parquet, compression="zstd")
        print(f"Created empty Parquet track: {output_parquet}")
        return

    # Check if it is a seqkit sliding-window GC/Skew output
    is_seqkit_gc = False
    if schema_name == "gc_content":
        try:
            with open(input_tsv, "r") as f:
                first_line = f.readline()
                if first_line.startswith("#name") or "sliding" in first_line:
                    is_seqkit_gc = True
        except OSError as exc:
            raise SystemExit(f"Error: Cannot read input TSV '{input_tsv}': {exc}") from exc

    if is_seqkit_gc:
        try:
            # We first read without schema since columns are dirty
            raw_df = pl.read_csv(input_tsv, separator="\t")
            new_cols = [c.lower().replace('#', '').strip() for c in raw_df.columns]
            raw_df = raw_df.rename(dict(zip(raw_df.columns, new_cols)))

            # Extract contig, start, end from name (e.g., "chr1_sliding:1-1000")
            # We use regex replace and extract
            pattern = r'^(.*)_sliding:(\d+)-(\d+)$'

            # Use `str.extract` with grouping
            extracted = raw_df.select([
                pl.col("name").str.extract(pattern, 1).alias("contig_ext"),
                pl.col("name").str.extract(pattern, 2).cast(pl.UInt32).alias("start_ext"),
                pl.col("name").str.extract(pattern, 3).cast(pl.UInt32).alias("end_ext")
            ])

            # Combine back
            raw_df = raw_df.with_columns([
                pl.when(extracted["contig_ext"].is_not_null())
                  .then(extracted["contig_ext"])
                  .otherwise(pl.col("name"))
                  .alias("contig"),
                pl.when(extracted["start_ext"].is_not_null())
                  .then(extracted["start_ext"] - 1)
                  .otherwise(pl.lit(0, dtype=pl.UInt32))
                  .alias("start_pos"),
                pl.when(extracted["end_ext"].is_not_null())
                  .then(extracted["end_ext"])
                  .otherwise(pl.lit(0, dtype=pl.UInt32))
                  .alias("end_pos")
            ])

            if len(raw_df.columns) >= 13:
                # all columns are present
                cols = list(raw_df.columns)
                df = raw_df.select([
                    pl.col("contig").cast(schema["contig"]),
                    pl.col("start_pos").cast(schema["start_pos"]),
                    pl.col("end_pos").cast(schema["end_pos"]),
                    pl.col(cols[1]).cast(schema["gc_pct"]).alias("gc_pct"),
                    pl.col(cols[2]).cast(schema["gc_skew"]).alias("gc_skew"),
                    pl.col(cols[3]).cast(schema["a_count"]).alias("a_count"),
                    pl.col(cols[4]).cast(schema["c_count"]).alias("c_count"),
                    pl.col(cols[5]).cast(schema["g_count"]).alias("g_count"),
                    pl.col(cols[6]).cast(schema["t_count"]).alias("t_count"),
                    pl.col(cols[7]).cast(schema["n_count"]).alias("n_count"),
                    pl.col(cols[8]).cast(schema["a_pct"]).alias("a_pct"),
                    pl.col(cols[9]).cast(schema["c_pct"]).alias("c_pct"),
                    pl.col(cols[10]).cast(schema["g_pct"]).alias("g_pct"),
                    pl.col(cols[11]).cast(schema["t_pct"]).alias("t_pct"),
                    pl.col(cols[12]).cast(schema["n_pct"]).alias("n_pct")
                ])
            else:
                cols = list(raw_df.columns)
                df = raw_df.select([
                    pl.col("contig").cast(schema["contig"]),
                    pl.col("start_pos").cast(schema["start_pos"]),
                    pl.col("end_pos").cast(schema["end_pos"]),
                    pl.col(cols[1]).cast(schema["gc_pct"]).alias("gc_pct"),
                    pl.col(cols[2]).cast(schema["gc_skew"]).alias("gc_skew"),
                    pl.lit(0).cast(schema["a_count"]).alias("a_count"),
                    pl.lit(0).cast(schema["c_count"]).alias("c_count"),
                    pl.lit(0).cast(schema["g_count"]).alias("g_count"),
                    pl.lit(0).cast(schema["t_count"]).alias("t_count"),
                    pl.lit(0).cast(schema["n_count"]).alias("n_count"),
                    pl.lit(0.0).cast(schema["a_pct"]).alias("a_pct"),
                    pl.lit(0.0).cast(schema["c_pct"]).alias("c_pct"),
                    pl.lit(0.0).cast(schema["g_pct"]).alias("g_pct"),
                    pl.lit(0.0).cast(schema["t_pct"]).alias("t_pct"),
                    pl.lit(0.0).cast(schema["n_pct"]).alias("n_pct")
                ])
        except (KeyError, ValueError, pl.exceptions.PolarsError) as exc:
            raise SystemExit(
                f"Error: Could not parse seqkit GC content from '{input_tsv}': {exc}"
            ) from exc

    elif schema_name == "assembly_gaps":
        try:
            # Check if it is a BED file from seqkit locate (which has 6 columns, no header)
            with open(input_tsv, "r") as f:
                first_line = f.readline().strip()

            num_cols = len(first_line.split("\t")) if first_line else 0
            if num_cols == 6:
                df = pl.read_csv(input_tsv, separator="\t", has_header=False,
                                 new_columns=["chrom", "start", "end", "name", "score", "strand"])
                df = df.select([
                    pl.col("chrom").cast(schema["contig"]).alias("contig"),
                    pl.col("start").cast(schema["start_pos"]).alias("start_pos"),
                    pl.col("end").cast(schema["end_pos"]).alias("end_pos"),
                    (pl.col("end") - pl.col("start")).cast(schema["gap_length"]).alias("gap_length"),
                    pl.lit("assembly_gap").cast(schema["gap_type"]).alias("gap_type")
                ])
            else:
                # Fallback to standard reading
                col_names = list(schema.keys())
                dtypes = list(schema.values())
                df = pl.read_csv(input_tsv, separator="\t", has_header=False, new_columns=col_names, dtypes=dtypes)
        except (ValueError, pl.exceptions.PolarsError) as exc:
            raise SystemExit(
                f"Error: Could not parse assembly gaps from '{input_tsv}': {exc}"
            ) from exc
    else:
        col_names = list(schema.keys())
        # Use read_csv with matching columns
        try:
            df = pl.read_csv(input_tsv, separator="\t", has_header=False, new_columns=col_names)
            # Ensure types match the schema precisely
            df = df.cast(schema)
        except (ValueError, pl.exceptions.PolarsError) as exc:
            raise SystemExit(
                f"Error: Could not parse TSV '{input_tsv}' as schema '{schema_name}': {exc}"
            ) from exc

    df.write_parquet(output_parquet, compression="zstd")
    print(f"Successfully created Parquet track: {output_parquet} ({out_path.stat().st_size} bytes)")

if __name__ == "__main__":
    args = parse_args()
    convert_tsv_to_parquet(args.input, args.output, args.schema)
