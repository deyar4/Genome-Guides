#!/usr/bin/env python3
"""Counts quoted about the gene unit and the coding rule, computed from the tracks (read-only).

(a) gene ids on one contig that split into several loci (the locus rule and SQL are those of extract_nested_genes.py,
    PAIRS_SQL statements tx_locus and genes) and the number of loci they contain.
(b) prefix rule vs annotated CDS: per (contig, gene_id), prefix class = protein_coding if all its transcripts are NM_/XM_,
    non_coding if none is, mixed otherwise; has_cds = some transcript has cds_bases > 0 in gene_architecture.
Writes a TSV (metric, value, note)."""
import argparse
import sys
from pathlib import Path

import duckdb

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "genes"))
from _common import write_tsv
from extract_nested_genes import PAIRS_SQL


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tracks-dir", required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    t = Path(args.tracks_dir)
    con = duckdb.connect()
    for name in ("transcripts", "gene_architecture", "gene_models"):
        con.execute(f"CREATE VIEW {name} AS SELECT * FROM read_parquet('{t / (name + '.parquet')}')")
    statements = [s for s in PAIRS_SQL.split(";") if s.strip()]
    assert "CREATE TABLE tx_locus" in statements[0] and "CREATE TABLE genes" in statements[1], "PAIRS_SQL layout changed"
    con.execute(statements[0])
    con.execute(statements[1])
    q = lambda sql: con.execute(sql).fetchall()
    rows = [
        ("gene_keys_contig_gene_id", q("SELECT count(*) FROM (SELECT DISTINCT contig, gene_id FROM transcripts)")[0][0], "distinct (contig, gene_id) in transcripts"),
        ("gene_units_loci", q("SELECT count(*) FROM genes")[0][0], "loci = gene units of the nested-gene step"),
    ]
    split = q("""SELECT count(*), sum(n) FROM (SELECT count(*) AS n FROM genes GROUP BY contig, gene_id HAVING count(*) > 1)""")[0]
    rows += [("gene_ids_split_into_loci", split[0], "(contig, gene_id) keys with more than one locus"),
             ("loci_of_split_gene_ids", split[1], "number of loci those keys contain")]
    cross = q("""
        WITH per_gene AS (
            SELECT t.contig, t.gene_id,
                   count(*) FILTER (WHERE regexp_matches(t.transcript_id, '^(NM_|XM_)')) AS n_coding_prefix,
                   count(*) FILTER (WHERE NOT regexp_matches(t.transcript_id, '^(NM_|XM_)')) AS n_other_prefix,
                   max(coalesce(ga.cds_bases, 0)) > 0 AS has_cds
            FROM transcripts t LEFT JOIN gene_architecture ga ON ga.transcript_id = t.transcript_id
            GROUP BY t.contig, t.gene_id)
        SELECT CASE WHEN n_other_prefix = 0 THEN 'protein_coding' WHEN n_coding_prefix = 0 THEN 'non_coding' ELSE 'mixed' END AS prefix_class,
               has_cds, count(*) FROM per_gene GROUP BY 1, 2 ORDER BY 1, 2""")
    for prefix_class, has_cds, n in cross:
        rows.append((f"genes_prefix_{prefix_class}_{'with' if has_cds else 'without'}_cds", n, "(contig, gene_id) keys"))
    write_tsv(args.out, rows)


if __name__ == "__main__":
    main()
