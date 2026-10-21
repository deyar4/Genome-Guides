#!/usr/bin/env python3
"""Calculate intergenic gaps and overlaps relative to the covered span (D39, resolves Q-gene-spacing).

Definition. Per contig, genes are sorted by (start, gene_id); intervals are half-open [start, end). For gene i let
M_i = max(end_j) over the genes j before i in that order, and g_i = the gene attaining M_i (ties: largest gene_id).
The first gene of a contig has no M and produces no row (count: genes - gap rows - overlap rows).
- start_i >= M_i: gene i lies right of everything before it. gene_spacing row (gene1 = g_i, gene2 = i),
  intergenic_dist = start_i - M_i (0 when abutting). These gaps are the complement of the union of all gene intervals.
- start_i <  M_i: gene i overlaps the covered span. gene_overlaps row (gene1 = g_i, gene2 = i),
  overlap_len = min(end_i, M_i) - start_i (intersection with g_i, since start_i >= start of g_i), contained = (end_i <= M_i),
  same_strand = strand of g_i equals strand of i. Only the max-end gene is paired; other overlapping genes get no row.
Replaces "adjacent by start" (next gene by start, gap from the preceding gene's own end), which mismeasured nested genes.
"""

import argparse
from pathlib import Path
import duckdb
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Gene Spacing & Overlaps.")
    parser.add_argument("--input", "-i", required=True, help="Input gene_models.parquet file")
    parser.add_argument("--out-dir", "-o", required=True, help="Output directory")
    add_resource_args(parser)
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
    # struct ordering is (end_pos, gene_id): max end first, then the largest gene_id, so the result is order-independent
    pairs = f"""
        WITH ordered AS (
            SELECT contig, start_pos, end_pos, gene_id, strand,
                MAX({{'e': end_pos, 'g': gene_id, 's': strand}}) OVER (
                    PARTITION BY contig ORDER BY start_pos, gene_id
                    ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING) AS prev
            FROM {source}
        ),
        cover AS (
            SELECT contig, start_pos, end_pos, gene_id, strand,
                prev.e AS cover_end, prev.g AS cover_gene, prev.s AS cover_strand
            FROM ordered
        )
    """
    try:
        con.execute(f"""
            COPY (
                {pairs}
                SELECT contig, cover_gene AS gene1_id, gene_id AS gene2_id,
                    CAST(start_pos - cover_end AS UINTEGER) AS intergenic_dist
                FROM cover
                WHERE cover_end IS NOT NULL AND start_pos >= cover_end
                ORDER BY contig, start_pos, gene_id
            ) TO ? (FORMAT PARQUET, COMPRESSION ZSTD)
        """, [str(out_p / "gene_spacing.parquet")])
        con.execute(f"""
            COPY (
                {pairs}
                SELECT contig, cover_gene AS gene1_id, gene_id AS gene2_id,
                    CAST(LEAST(end_pos, cover_end) - start_pos AS UINTEGER) AS overlap_len,
                    CAST(strand = cover_strand AS UTINYINT) AS same_strand,
                    CAST(end_pos <= cover_end AS UTINYINT) AS contained
                FROM cover
                WHERE cover_end IS NOT NULL AND start_pos < cover_end
                ORDER BY contig, start_pos, gene_id
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
