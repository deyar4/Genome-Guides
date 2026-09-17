#!/usr/bin/env python3
"""
Genome Guides - Gene Spacing & Intergenic Distance Extractor
Calculates intergenic distances between consecutive genes and flags gene overlap pairs.
Optimized, vectorized version using polars.
"""

import argparse
from pathlib import Path
import duckdb

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Gene Spacing & Overlaps.")
    parser.add_argument("--input", "-i", required=True, help="Input gene_models.parquet file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    return parser.parse_args()

def process_spacing(input_parquet: str, out_dir: str):
    out_p = Path(out_dir)
    out_p.mkdir(parents=True, exist_ok=True)

    try:
        con = duckdb.connect()
        columns = {
            row[0] for row in con.execute(
                f"DESCRIBE SELECT * FROM read_parquet(?)", [input_parquet]
            ).fetchall()
        }
    except FileNotFoundError as exc:
        raise SystemExit(f"Error: Input Parquet not found: {input_parquet}") from exc
    except (OSError, duckdb.Error) as exc:
        raise SystemExit(f"Error: Could not read input Parquet '{input_parquet}': {exc}") from exc

    required_columns = {"contig", "start_pos", "end_pos", "gene_id", "strand"}
    missing = required_columns.difference(columns)
    if missing:
        names = ", ".join(sorted(missing))
        raise SystemExit(f"Error: Input Parquet is missing required columns: {names}")

    safe_input = input_parquet.replace("'", "''")
    source = f"read_parquet('{safe_input}')"
    pairs = f"""
        WITH ordered AS (
            SELECT contig, start_pos, end_pos, gene_id, strand,
                LEAD(start_pos) OVER (PARTITION BY contig ORDER BY start_pos, gene_id) AS next_start_pos,
                LEAD(gene_id) OVER (PARTITION BY contig ORDER BY start_pos, gene_id) AS next_gene_id,
                LEAD(strand) OVER (PARTITION BY contig ORDER BY start_pos, gene_id) AS next_strand
            FROM {source}
        )
    """
    try:
        con.execute(f"""
            COPY (
                {pairs}
                SELECT contig, gene_id AS gene1_id, next_gene_id AS gene2_id,
                    CAST(next_start_pos - end_pos AS UINTEGER) AS intergenic_dist
                FROM ordered
                WHERE next_start_pos IS NOT NULL AND next_start_pos > end_pos
            ) TO ? (FORMAT PARQUET, COMPRESSION ZSTD)
        """, [str(out_p / "gene_spacing.parquet")])
        con.execute(f"""
            COPY (
                {pairs}
                SELECT contig, gene_id AS gene1_id, next_gene_id AS gene2_id,
                    CAST(end_pos - next_start_pos AS UINTEGER) AS overlap_len,
                    CAST(strand = next_strand AS UTINYINT) AS same_strand
                FROM ordered
                WHERE next_start_pos IS NOT NULL AND next_start_pos <= end_pos
            ) TO ? (FORMAT PARQUET, COMPRESSION ZSTD)
        """, [str(out_p / "gene_overlaps.parquet")])
    except duckdb.Error as exc:
        raise SystemExit(f"Error: Could not calculate gene spacing: {exc}") from exc
    finally:
        con.close()
    print("Created gene_spacing.parquet and gene_overlaps.parquet.")

if __name__ == "__main__":
    args = parse_args()
    process_spacing(args.input, args.out_dir)
