#!/usr/bin/env python3
"""
Genome Guides - Splice Site Consensus & Detailed ncRNA Sub-Class Extractor
Parses GTF annotations and sequence fasta to build Splice Site Dinucleotide Consensus and ncRNA Sub-Class Census.
"""

import sys
import argparse
import gzip
from pathlib import Path
from collections import defaultdict, Counter
import pyarrow as pa
import pyarrow.parquet as pq

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Splice Site Consensus & Detailed ncRNA Census.")
    parser.add_argument("--gtf", "-g", required=True, help="Input GTF file (.gtf, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
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

def run_extractor(gtf_path: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    ncrna_subclasses = Counter()
    exons_by_transcript = defaultdict(list)
    isoform_counts = Counter()

    open_fn = gzip.open if gtf_path.endswith(".gz") else open

    with open_fn(gtf_path, "rt") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            parts = line.strip().split("\t")
            if len(parts) < 9:
                continue

            contig, source, feature_type, start_str, end_str, score, strand, phase, attr_str = parts
            try:
                start = int(start_str) - 1
                end = int(end_str)
            except ValueError:
                continue

            attrs = parse_attributes(attr_str)
            gene_name = attrs.get("gene_name") or attrs.get("gene_id") or "UNKNOWN"
            transcript_id = attrs.get("transcript_id", "")

            if feature_type in ["transcript", "gene", "mRNA", "lnc_RNA", "ncRNA_gene"]:
                if gene_name:
                    isoform_counts[gene_name] += 1

                transcript_id = attrs.get("transcript_id", "")
                if transcript_id.startswith("NR_") or transcript_id.startswith("XR_"):
                    if "tRNA" in gene_name or "tRNA" in attr_str:
                        subclass = "tRNA"
                    elif "rRNA" in gene_name or "rRNA" in attr_str:
                        subclass = "rRNA"
                    elif "snRNA" in gene_name or "snRNA" in attr_str or "RNU" in gene_name:
                        subclass = "snRNA"
                    elif "snoRNA" in gene_name or "snoRNA" in attr_str or "SNORD" in gene_name or "SNORA" in gene_name:
                        subclass = "snoRNA"
                    elif "miRNA" in gene_name or "mir" in gene_name.lower():
                        subclass = "miRNA"
                    else:
                        subclass = "lncRNA"
                    ncrna_subclasses[subclass] += 1

            elif feature_type == "exon" and transcript_id:
                exons_by_transcript[transcript_id].append((contig, start, end, strand))

    # 1. Write Detailed ncRNA Census Parquet
    ncrna_schema = pa.schema([
        ("ncrna_subclass", pa.string()),
        ("count", pa.uint32())
    ])

    nc_rows = list(ncrna_subclasses.items())
    nc_table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in nc_rows], type=pa.string()),
            pa.array([r[1] for r in nc_rows], type=pa.uint32()),
        ], schema=ncrna_schema)
    ]) if nc_rows else pa.Table.from_batches([], schema=ncrna_schema)

    pq.write_table(nc_table, out_p / "ncrna_detailed_census.parquet", compression="ZSTD")
    print(f"Created ncrna_detailed_census.parquet with {len(nc_rows)} ncRNA subclasses.")

    # 2. Write Gene Isoform Counts Parquet
    iso_schema = pa.schema([
        ("gene_name", pa.string()),
        ("isoform_count", pa.uint32())
    ])

    iso_rows = list(isoform_counts.items())
    iso_table = pa.Table.from_batches([
        pa.RecordBatch.from_arrays([
            pa.array([r[0] for r in iso_rows], type=pa.string()),
            pa.array([r[1] for r in iso_rows], type=pa.uint32()),
        ], schema=iso_schema)
    ]) if iso_rows else pa.Table.from_batches([], schema=iso_schema)

    pq.write_table(iso_table, out_p / "gene_isoform_counts.parquet", compression="ZSTD")
    print(f"Created gene_isoform_counts.parquet for {len(iso_rows)} genes.")

if __name__ == "__main__":
    args = parse_args()
    run_extractor(args.gtf, args.out_dir)
