#!/usr/bin/env python3
"""
Genome Guides - Fast GFF3/GTF Annotation Converter & Density Generator
Parses GFF3/GTF annotation files into Parquet files for Gene Models, Gene Densities, and Biotype Census.
Optimized version using regex for fast attribute extraction.
"""

import sys
import argparse
import gzip
import re
from pathlib import Path
from collections import defaultdict
import polars as pl
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args

# Precompiled regex for rapid extraction of key GTF/GFF attributes
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

    genes_data = []
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

                genes_data.append((contig, start, end, strand, gene_id, gene_name, biotype))
                biotype_counts[biotype] += 1

                w_idx = start // window_size
                gene_density_windows[(contig, w_idx * window_size)] += 1

    # 1. Write Gene Models Parquet
    gene_schema = {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "strand": pl.Utf8,
        "gene_id": pl.Utf8,
        "gene_name": pl.Utf8,
        "biotype": pl.Utf8
    }
    
    if genes_data:
        gene_df = pl.DataFrame(genes_data, schema=gene_schema, orient="row")
    else:
        gene_df = pl.DataFrame(schema=gene_schema)

    gene_df.write_parquet(out_p / "gene_models.parquet", compression="zstd")
    print(f"Created gene_models.parquet with {len(genes_data)} genes.")

    # 2. Write Gene Density Parquet
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

    # 3. Write Biotype Census Parquet
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
