#!/usr/bin/env python3
"""
Genome Guides - Fast GFF3/GTF Annotation Converter & Density Generator
Parses GFF3/GTF annotation files into Parquet files for Gene Models, Gene Densities, and Biotype Census.
"""

import sys
import argparse
import gzip
from pathlib import Path
from collections import defaultdict
import pyarrow as pa
import pyarrow.parquet as pq

def parse_args():
    parser = argparse.ArgumentParser(description="Parse GFF3/GTF annotations to Parquet.")
    parser.add_argument("--input", "-i", required=True, help="Input GFF3 or GTF file (.gff3, .gtf, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory for track parquet files")
    return parser.parse_args()

def parse_attributes(attr_str):
    attrs = {}
    for item in attr_str.strip().split(";"):
        item = item.strip()
        if not item:
            continue
        if "=" in item:
            k, v = item.split("=", 1)
            attrs[k.strip()] = v.strip().strip('"')
        else:
            parts = item.split(None, 1)
            if len(parts) == 2:
                k, v = parts[0], parts[1].strip('"')
                attrs[k.strip()] = v.strip().strip('"')
    return attrs

def run_parser(input_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    genes_data = []
    exons_data = []
    seen_genes = set()
    biotype_counts = defaultdict(int)
    gene_density_windows = defaultdict(int)
    exon_density_windows = defaultdict(int)
    window_size = 100000  # 100kb density windows

    open_fn = gzip.open if input_path.endswith(".gz") else open

    with open_fn(input_path, "rt") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            parts = line.strip().split("\t")
            if len(parts) < 9:
                continue

            contig, source, feature_type, start_str, end_str, score, strand, phase, attr_str = parts
            try:
                start = int(start_str) - 1  # 0-based
                end = int(end_str)
            except ValueError:
                continue

            attrs = parse_attributes(attr_str)

            if feature_type in ["transcript", "gene", "mRNA", "lnc_RNA", "ncRNA_gene"]:
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

            elif feature_type == "exon":
                exon_id = attrs.get("exon_id") or attrs.get("ID") or f"EXON_{len(exons_data)}"
                exons_data.append((contig, start, end, strand, exon_id))

                w_idx = start // window_size
                exon_density_windows[(contig, w_idx * window_size)] += 1

    # 1. Write Gene Models Parquet
    gene_schema = pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("strand", pa.string()),
        ("gene_id", pa.string()),
        ("gene_name", pa.string()),
        ("biotype", pa.string())
    ])
    
    if genes_data:
        gene_table = pa.Table.from_batches([
            pa.RecordBatch.from_arrays([
                pa.array([g[0] for g in genes_data], type=pa.string()),
                pa.array([g[1] for g in genes_data], type=pa.uint32()),
                pa.array([g[2] for g in genes_data], type=pa.uint32()),
                pa.array([g[3] for g in genes_data], type=pa.string()),
                pa.array([g[4] for g in genes_data], type=pa.string()),
                pa.array([g[5] for g in genes_data], type=pa.string()),
                pa.array([g[6] for g in genes_data], type=pa.string()),
            ], schema=gene_schema)
        ])
    else:
        gene_table = pa.Table.from_batches([], schema=gene_schema)

    pq.write_table(gene_table, out_p / "gene_models.parquet", compression="ZSTD")
    print(f"Created gene_models.parquet with {len(genes_data)} genes.")

    # 2. Write Gene Density Parquet
    density_schema = pa.schema([
        ("contig", pa.string()),
        ("start_pos", pa.uint32()),
        ("end_pos", pa.uint32()),
        ("count", pa.uint32())
    ])

    density_rows = [
        (k[0], k[1], k[1] + window_size, v)
        for k, v in gene_density_windows.items()
    ]
    density_rows.sort(key=lambda x: (x[0], x[1]))

    if density_rows:
        density_table = pa.Table.from_batches([
            pa.RecordBatch.from_arrays([
                pa.array([r[0] for r in density_rows], type=pa.string()),
                pa.array([r[1] for r in density_rows], type=pa.uint32()),
                pa.array([r[2] for r in density_rows], type=pa.uint32()),
                pa.array([r[3] for r in density_rows], type=pa.uint32()),
            ], schema=density_schema)
        ])
    else:
        density_table = pa.Table.from_batches([], schema=density_schema)

    pq.write_table(density_table, out_p / "gene_density.parquet", compression="ZSTD")
    print(f"Created gene_density.parquet with {len(density_rows)} window bins.")

    # 3. Write Biotype Census Parquet
    biotype_schema = pa.schema([
        ("biotype", pa.string()),
        ("count", pa.uint32())
    ])

    b_rows = list(biotype_counts.items())
    b_table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in b_rows], type=pa.string()),
            pa.array([r[1] for r in b_rows], type=pa.uint32()),
        ], schema=biotype_schema)
    ])
    pq.write_table(b_table, out_p / "gene_biotype_census.parquet", compression="ZSTD")
    print(f"Created gene_biotype_census.parquet with {len(b_rows)} biotypes.")

if __name__ == "__main__":
    args = parse_args()
    run_parser(args.input, args.out_dir)
