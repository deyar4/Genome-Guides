#!/usr/bin/env python3
"""
Genome Guides - Universal TSV to ZSTD Compressed Parquet Converter
"""

import sys
import argparse
import polars as pl
from pathlib import Path
import pyarrow.parquet as pq

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


def write_batched_csv(input_tsv, output_parquet, schema, new_columns):
    batches = pl.scan_csv(
        input_tsv,
        separator="\t",
        has_header=False,
        new_columns=new_columns,
        schema_overrides=schema,
    ).collect_batches(chunk_size=65_536)
    writer = None
    try:
        for frame in batches:
            frame = frame.cast(schema)
            if writer is None:
                writer = pq.ParquetWriter(
                    output_parquet, frame.to_arrow().schema, compression="zstd"
                )
            writer.write_table(frame.to_arrow())
    finally:
        if writer is not None:
            writer.close()
    if writer is None:
        pl.DataFrame(schema=schema).write_parquet(output_parquet, compression="zstd")


def write_batched_assembly_gaps(input_tsv, output_parquet, schema):
    batches = pl.scan_csv(
        input_tsv,
        separator="\t",
        has_header=False,
        new_columns=["chrom", "start", "end", "name", "score", "strand"],
        schema_overrides={"chrom": pl.Utf8, "start": pl.UInt32, "end": pl.UInt32},
    ).collect_batches(chunk_size=65_536)
    writer = None
    try:
        for batch in batches:
            frame = batch.select([
                pl.col("chrom").alias("contig"),
                pl.col("start").alias("start_pos"),
                pl.col("end").alias("end_pos"),
                (pl.col("end") - pl.col("start")).alias("gap_length"),
                pl.lit("assembly_gap").alias("gap_type"),
            ]).cast(schema)
            arrow = frame.to_arrow()
            if writer is None:
                writer = pq.ParquetWriter(output_parquet, arrow.schema, compression="zstd")
            writer.write_table(arrow)
    finally:
        if writer is not None:
            writer.close()
    if writer is None:
        pl.DataFrame(schema=schema).write_parquet(output_parquet, compression="zstd")


def write_batched_seqkit_gc(input_tsv, output_parquet, schema):
    with open(input_tsv, "r") as handle:
        header = handle.readline().rstrip("\n").split("\t")
    if len(header) < 3:
        raise ValueError("seqkit GC input must contain a name and two metric columns")
    columns = [name.lower().replace("#", "").strip() for name in header]
    batches = pl.scan_csv(
        input_tsv,
        separator="\t",
        has_header=True,
        new_columns=columns,
        infer_schema_length=1000,
    ).collect_batches(chunk_size=65_536)
    writer = None
    pattern = r"^(.*)_sliding:(\d+)-(\d+)$"
    try:
        for raw in batches:
            cols = raw.columns
            extracted = raw["name"].str.extract_groups(pattern)
            frame = raw.with_columns([
                pl.when(extracted.struct.field("1").is_not_null())
                .then(extracted.struct.field("1"))
                .otherwise(pl.col("name"))
                .alias("contig"),
                pl.when(extracted.struct.field("2").is_not_null())
                .then(extracted.struct.field("2").cast(pl.UInt32) - 1)
                .otherwise(pl.lit(0, dtype=pl.UInt32))
                .alias("start_pos"),
                pl.when(extracted.struct.field("3").is_not_null())
                .then(extracted.struct.field("3").cast(pl.UInt32))
                .otherwise(pl.lit(0, dtype=pl.UInt32))
                .alias("end_pos"),
            ])
            expressions = [
                pl.col("contig").cast(schema["contig"]),
                pl.col("start_pos").cast(schema["start_pos"]),
                pl.col("end_pos").cast(schema["end_pos"]),
            ]
            metric_names = ["gc_pct", "gc_skew", "a_count", "c_count", "g_count",
                            "t_count", "n_count", "a_pct", "c_pct", "g_pct", "t_pct", "n_pct"]
            for index, name in enumerate(metric_names, 1):
                if index < len(cols):
                    expressions.append(pl.col(cols[index]).cast(schema[name]).alias(name))
                else:
                    default = 0.0 if name.endswith("_pct") or name == "gc_skew" else 0
                    expressions.append(pl.lit(default).cast(schema[name]).alias(name))
            frame = frame.select(expressions)
            arrow = frame.to_arrow()
            if writer is None:
                writer = pq.ParquetWriter(output_parquet, arrow.schema, compression="zstd")
            writer.write_table(arrow)
    finally:
        if writer is not None:
            writer.close()
    if writer is None:
        pl.DataFrame(schema=schema).write_parquet(output_parquet, compression="zstd")


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
            write_batched_seqkit_gc(input_tsv, output_parquet, schema)
            print(f"Successfully created Parquet track: {output_parquet} ({out_path.stat().st_size} bytes)")
            return
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
                write_batched_assembly_gaps(input_tsv, output_parquet, schema)
                print(f"Successfully created Parquet track: {output_parquet} ({out_path.stat().st_size} bytes)")
                return
            else:
                # Fallback to standard reading
                write_batched_csv(input_tsv, output_parquet, schema, list(schema.keys()))
                print(f"Successfully created Parquet track: {output_parquet} ({out_path.stat().st_size} bytes)")
                return
        except (ValueError, pl.exceptions.PolarsError) as exc:
            raise SystemExit(
                f"Error: Could not parse assembly gaps from '{input_tsv}': {exc}"
            ) from exc
    else:
        try:
            write_batched_csv(input_tsv, output_parquet, schema, list(schema.keys()))
            print(f"Successfully created Parquet track: {output_parquet} ({out_path.stat().st_size} bytes)")
            return
        except (ValueError, pl.exceptions.PolarsError) as exc:
            raise SystemExit(
                f"Error: Could not parse TSV '{input_tsv}' as schema '{schema_name}': {exc}"
            ) from exc

    df.write_parquet(output_parquet, compression="zstd")
    print(f"Successfully created Parquet track: {output_parquet} ({out_path.stat().st_size} bytes)")

if __name__ == "__main__":
    args = parse_args()
    convert_tsv_to_parquet(args.input, args.output, args.schema)
