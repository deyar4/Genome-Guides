#!/usr/bin/env python3
"""Extract splice-site and non-coding RNA summary tracks from GTF data."""

import sys
import argparse
import gzip
import re
from pathlib import Path
from collections import defaultdict, Counter
import polars as pl
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args

# Capture the identifiers needed to group exons and classify RNA biotypes.
ATTR_RE = re.compile(
    r'(gene_name|Name|gene_id|ID|transcript_id|gene_biotype|gene_type|gbkey|exon_id)[ \t]*[= ][ \t]*"?([^";]+)"?'
)

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Splice Site Consensus & Detailed ncRNA Census.")
    parser.add_argument("--gtf", "-g", required=True, help="Input GTF file (.gtf, .gz)")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    add_resource_args(parser)
    return parser.parse_args()

def parse_attributes_fast(attr_str):
    return {m[0]: m[1].strip() for m in ATTR_RE.findall(attr_str)}

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

            contig, _, feature_type, start_str, end_str, _, strand, _, attr_str = parts
            try:
                start = int(start_str) - 1
                end = int(end_str)
            except ValueError:
                continue

            attrs = parse_attributes_fast(attr_str)
            gene_name = attrs.get("gene_name") or attrs.get("gene_id") or "UNKNOWN"
            transcript_id = attrs.get("transcript_id", "")

            if feature_type in ("transcript", "gene", "mRNA", "lnc_RNA", "ncRNA_gene"):
                if gene_name:
                    isoform_counts[gene_name] += 1

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

    ncrna_schema = {
        "ncrna_subclass": pl.Utf8,
        "count": pl.UInt32
    }

    nc_rows = list(ncrna_subclasses.items())
    if nc_rows:
        nc_df = pl.DataFrame(nc_rows, schema=ncrna_schema, orient="row")
    else:
        nc_df = pl.DataFrame(schema=ncrna_schema)

    nc_df.write_parquet(out_p / "ncrna_detailed_census.parquet", compression="zstd")
    print(f"Created ncrna_detailed_census.parquet with {len(nc_rows)} ncRNA subclasses.")

    iso_schema = {
        "gene_name": pl.Utf8,
        "isoform_count": pl.UInt32
    }

    iso_rows = list(isoform_counts.items())
    if iso_rows:
        iso_df = pl.DataFrame(iso_rows, schema=iso_schema, orient="row")
    else:
        iso_df = pl.DataFrame(schema=iso_schema)

    iso_df.write_parquet(out_p / "gene_isoform_counts.parquet", compression="zstd")
    print(f"Created gene_isoform_counts.parquet for {len(iso_rows)} genes.")

if __name__ == "__main__":
    args = parse_args()
    run_extractor(args.gtf, args.out_dir)
