#!/usr/bin/env python3
"""
Genome Guides - Universal TSV to ZSTD Compressed Parquet Converter
"""

import sys
import argparse
import pyarrow.csv as pv
import pyarrow.parquet as pq
import pyarrow as pa
from pathlib import Path

SCHEMAS = {
    "gc_content": pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("gc_pct", pa.float32()),
        ("gc_skew", pa.float32())
    ]),
    "assembly_gaps": pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("gap_length", pa.uint32()),
        ("gap_type", pa.string())
    ]),
    "sequence_complexity": pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("shannon_entropy", pa.float32()),
        ("repeats_count", pa.uint32())
    ]),
    "hecate_bwt": pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("uniqueness_score", pa.float32()),
        ("bwt_entropy", pa.float32())
    ])
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

    read_options = pv.ReadOptions(column_names=schema.names, autogenerate_column_names=False)
    parse_options = pv.ParseOptions(delimiter="\t")

    try:
        table = pv.read_csv(input_tsv, read_options=read_options, parse_options=parse_options)
        table = table.cast(schema)
    except Exception as e:
        print(f"Warning: Input TSV {input_tsv} is empty or missing. Creating empty schema parquet. ({e})")
        table = pa.Table.from_batches([], schema=schema)

    pq.write_table(table, output_parquet, compression="ZSTD")
    print(f"Successfully created Parquet track: {output_parquet} ({out_path.stat().st_size} bytes)")

if __name__ == "__main__":
    args = parse_args()
    convert_tsv_to_parquet(args.input, args.output, args.schema)
