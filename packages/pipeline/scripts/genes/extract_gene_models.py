#!/usr/bin/env python3
"""Extract gene models, density bins, and biotype counts from GFF3/GTF."""

import sys
import argparse
import gzip
import re
from pathlib import Path
from collections import defaultdict
import polars as pl
import pyarrow as pa
import pyarrow.parquet as pq
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args

# Capture identifiers used to deduplicate genes and classify transcripts.
ATTR_RE = re.compile(
    r'(gene_name|Name|gene_id|ID|transcript_id|gene_biotype|gene_type|gbkey|exon_id)[ \t]*[= ][ \t]*"?([^";]+)"?'
)

def parse_args():
    parser = argparse.ArgumentParser(description="Parse GFF3/GTF annotations to Parquet.")
    parser.add_argument("--input", "-i", required=True, help="Input GFF3 or GTF file (.gff3, .gtf, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory for track parquet files")
    add_resource_args(parser)
    return parser.parse_args()

def parse_attributes_fast(attr_str):
    return {m[0]: m[1].strip() for m in ATTR_RE.findall(attr_str)}

def run_parser(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    gene_schema = {
        "contig": pa.string(), "start_pos": pa.uint32(), "end_pos": pa.uint32(),
        "strand": pa.string(), "gene_id": pa.string(), "gene_name": pa.string(),
        "biotype": pa.string(),
    }
    gene_writer = None
    gene_count = 0
    seen_genes = set()
    biotype_counts = defaultdict(int)
    gene_density_windows = defaultdict(int)
    window_size = 100000  # 100kb density windows
    malformed_rows = 0

    open_fn = gzip.open if input_path.endswith(".gz") else open

    with open_fn(input_path, "rt") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            parts = line.strip().split("\t")
            if len(parts) < 9:
                malformed_rows += 1
                continue

            contig, _, feature_type, start_str, end_str, _, strand, _, attr_str = parts
            if feature_type not in ("transcript", "gene", "mRNA", "lnc_RNA", "ncRNA_gene"):
                continue

            try:
                start = int(start_str) - 1  # 0-based
                end = int(end_str)
            except ValueError:
                malformed_rows += 1
                continue

            attrs = parse_attributes_fast(attr_str)
            gene_name = attrs.get("gene_name") or attrs.get("Name") or attrs.get("gene_id") or "UNKNOWN"
            gene_id = attrs.get("gene_id") or attrs.get("ID") or gene_name

            gene_key = (contig, gene_id)
            if gene_key not in seen_genes:
                seen_genes.add(gene_key)

                transcript_id = attrs.get("transcript_id", "N/A")
                biotype = attrs.get("gene_biotype") or attrs.get("gene_type") or attrs.get("gbkey") or ("protein_coding" if transcript_id.startswith("NM_") or transcript_id.startswith("XM_") else "non_coding")

                row = {
                    "contig": contig, "start_pos": start, "end_pos": end,
                    "strand": strand, "gene_id": gene_id, "gene_name": gene_name,
                    "biotype": biotype,
                }
                table = pa.Table.from_pylist([row], schema=pa.schema([
                    pa.field(name, dtype) for name, dtype in gene_schema.items()
                ]))
                if gene_writer is None:
                    gene_writer = pq.ParquetWriter(
                        out_p / "gene_models.parquet", table.schema, compression="zstd"
                    )
                gene_writer.write_table(table)
                gene_count += 1
                biotype_counts[biotype] += 1

                w_idx = start // window_size
                gene_density_windows[(contig, w_idx * window_size)] += 1


    if gene_writer is not None:
        gene_writer.close()
    else:
        empty_schema = pa.schema([pa.field(name, dtype) for name, dtype in gene_schema.items()])
        pq.write_table(pa.Table.from_pylist([], schema=empty_schema), out_p / "gene_models.parquet", compression="zstd")
    print(f"Created gene_models.parquet with {gene_count} genes.")

    density_schema = {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "count": pl.UInt32
    }

    density_rows = [
        (k[0], k[1], k[1] + window_size, v)
        for k, v in gene_density_windows.items()
    ]
    density_rows.sort(key=lambda x: (x[0], x[1]))

    if density_rows:
        density_df = pl.DataFrame(density_rows, schema=density_schema, orient="row")
    else:
        density_df = pl.DataFrame(schema=density_schema)

    density_df.write_parquet(out_p / "gene_density.parquet", compression="zstd")
    print(f"Created gene_density.parquet with {len(density_rows)} window bins.")

    biotype_schema = {
        "biotype": pl.Utf8,
        "count": pl.UInt32
    }

    b_rows = list(biotype_counts.items())
    if b_rows:
        b_df = pl.DataFrame(b_rows, schema=biotype_schema, orient="row")
    else:
        b_df = pl.DataFrame(schema=biotype_schema)

    b_df.write_parquet(out_p / "gene_biotype_census.parquet", compression="zstd")
    print(f"Created gene_biotype_census.parquet with {len(b_rows)} biotypes.")
    if malformed_rows:
        print(f"Warning: skipped {malformed_rows} malformed annotation rows.", file=sys.stderr)

if __name__ == "__main__":
    args = parse_args()
    run_parser(args.input, args.out_dir)
