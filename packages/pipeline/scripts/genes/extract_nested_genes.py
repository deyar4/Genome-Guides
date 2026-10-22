#!/usr/bin/env python3
"""Nested gene pairs (host contains guest) with relationship type, strand relation and direct-host flag.

Definition: docs/analyses/02_nested_genes.md (D43). Gene unit = (contig, gene_id, locus): transcripts of one (contig, gene_id) split into loci where a transcript starts at or after the largest end so far. Intervals are 0-based half-open.
Pair (H, g): same contig, H.start <= g.start and g.end <= H.end, spans not identical, genes different.
coding_class = protein_coding iff some transcript has cds_bases > 0 (gene_architecture), else non_coding.
Strand relation is taken from the strand sets of the transcripts, never from gene_models.strand.
The step fails if its input preconditions do not hold."""

import argparse
from pathlib import Path

import duckdb

import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args, effective_memory_mb


def check_preconditions(con):
    checks = {
        "(contig, transcript_id) is not unique in transcripts":
            "SELECT count(*) - count(DISTINCT (contig, transcript_id)) FROM transcripts",
        "gene_id 'UNKNOWN' present":
            "SELECT count(*) FROM gene_models WHERE gene_id = 'UNKNOWN'",
        "transcript strand outside {+,-}":
            "SELECT count(*) FROM transcripts WHERE strand IS NULL OR strand NOT IN ('+', '-')",
        "exons of one transcript_id on several contigs":
            "SELECT count(*) FROM (SELECT transcript_id FROM exons GROUP BY transcript_id HAVING count(DISTINCT contig) > 1)",
        "transcripts without exons":
            "SELECT count(*) FROM transcripts WHERE (contig, transcript_id) NOT IN (SELECT (contig, transcript_id) FROM exons)",
        "transcripts with overlapping exons":
            "SELECT count(DISTINCT e.transcript_id) FROM exons e JOIN exons f ON e.contig = f.contig "
            "AND e.transcript_id = f.transcript_id AND (e.start_pos, e.end_pos) < (f.start_pos, f.end_pos) "
            "AND f.start_pos < e.end_pos",
        "transcripts missing in gene_architecture":
            "SELECT count(*) FROM transcripts WHERE transcript_id NOT IN (SELECT transcript_id FROM gene_architecture)",
    }
    failed = [(msg, n) for msg, sql in checks.items() if (n := con.execute(sql).fetchone()[0])]
    if failed:
        raise SystemExit("Precondition failed: " + "; ".join(f"{m} ({n})" for m, n in failed))


PAIRS_SQL = """
-- locus: transcripts of one (contig, gene_id) form a new locus when a transcript starts at or after the largest end so far
CREATE TABLE tx_locus AS
SELECT contig, gene_id, transcript_id, strand, start_pos, end_pos,
       sum(brk) OVER (PARTITION BY contig, gene_id ORDER BY start_pos, end_pos, transcript_id) AS locus, abut
FROM (
    SELECT *, coalesce(start_pos >= prev_max, false) AS brk_b, coalesce(start_pos = prev_max, false) AS abut_b,
           CASE WHEN coalesce(start_pos >= prev_max, false) THEN 1 ELSE 0 END AS brk,
           CASE WHEN coalesce(start_pos = prev_max, false) THEN 1 ELSE 0 END AS abut
    FROM (SELECT *, max(end_pos) OVER (w ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING) AS prev_max
          FROM transcripts
          WINDOW w AS (PARTITION BY contig, gene_id ORDER BY start_pos, end_pos, transcript_id)));

CREATE TABLE genes AS
SELECT row_number() OVER (ORDER BY t.contig, t.gene_id, t.locus) AS gix, t.contig, t.gene_id, t.locus,
       any_value(gm.gene_name) AS gene_name, min(t.start_pos) AS start_pos, max(t.end_pos) AS end_pos,
       CASE WHEN max(coalesce(ga.cds_bases, 0)) > 0 THEN 'protein_coding' ELSE 'non_coding' END AS coding_class,
       list_sort(list_distinct(list(t.strand))) AS strand_set,
       count(DISTINCT t.strand) = 1 AS stranded,
       min(t.strand) AS strand
FROM tx_locus t
LEFT JOIN gene_architecture ga ON ga.transcript_id = t.transcript_id
LEFT JOIN gene_models gm ON gm.contig = t.contig AND gm.gene_id = t.gene_id
GROUP BY t.contig, t.gene_id, t.locus;

CREATE TABLE base AS
SELECT h.gix AS host, g.gix AS guest
FROM genes h JOIN genes g
  ON h.contig = g.contig AND h.start_pos <= g.start_pos AND g.end_pos <= h.end_pos
WHERE h.gix <> g.gix AND NOT (h.start_pos = g.start_pos AND h.end_pos = g.end_pos);

-- (H, g) is NOT direct iff some H' has (H', g) in base and (H, H') in base
CREATE TABLE indirect AS
SELECT DISTINCT p2.host, p1.guest FROM base p1 JOIN base p2 ON p1.host = p2.guest;

CREATE TABLE in_intron AS
SELECT DISTINCT b.host, b.guest
FROM base b
JOIN genes g ON g.gix = b.guest
JOIN tx_locus tl ON tl.contig = g.contig
JOIN genes h ON h.gix = b.host AND h.contig = tl.contig AND h.gene_id = tl.gene_id AND h.locus = tl.locus
JOIN introns i ON i.contig = tl.contig AND i.transcript_id = tl.transcript_id
WHERE i.start_pos <= g.start_pos AND g.end_pos <= i.end_pos;

CREATE TABLE over_exon AS
SELECT DISTINCT b.host, b.guest
FROM base b
JOIN genes g ON g.gix = b.guest
JOIN genes h ON h.gix = b.host
JOIN tx_locus tl ON tl.contig = h.contig AND tl.gene_id = h.gene_id AND tl.locus = h.locus
JOIN exons e ON e.contig = tl.contig AND e.transcript_id = tl.transcript_id
WHERE e.start_pos < g.end_pos AND g.start_pos < e.end_pos;

CREATE TABLE pairs AS
SELECT h.contig,
       h.gene_id AS host_gene_id, h.locus AS host_locus, h.gene_name AS host_gene_name,
       h.start_pos AS host_start, h.end_pos AS host_end, h.strand_set AS host_strand_set,
       h.coding_class AS host_coding_class,
       g.gene_id AS guest_gene_id, g.locus AS guest_locus, g.gene_name AS guest_gene_name,
       g.start_pos AS guest_start, g.end_pos AS guest_end, g.strand_set AS guest_strand_set,
       g.coding_class AS guest_coding_class,
       ii.host IS NOT NULL AS in_host_intron, oe.host IS NOT NULL AS overlaps_host_exon,
       CASE WHEN ii.host IS NOT NULL AND oe.host IS NULL THEN 'intronic'
            WHEN ii.host IS NULL AND oe.host IS NOT NULL THEN 'exonic_overlap'
            WHEN ii.host IS NOT NULL THEN 'mixed' ELSE 'span_only' END AS relationship_type,
       CASE WHEN NOT (h.stranded AND g.stranded) THEN 'ambiguous'
            WHEN h.strand = g.strand THEN 'same' ELSE 'opposite' END AS strand_relation,
       ind.host IS NULL AS is_direct
FROM base b
JOIN genes h ON h.gix = b.host
JOIN genes g ON g.gix = b.guest
LEFT JOIN in_intron ii ON ii.host = b.host AND ii.guest = b.guest
LEFT JOIN over_exon oe ON oe.host = b.host AND oe.guest = b.guest
LEFT JOIN indirect ind ON ind.host = b.host AND ind.guest = b.guest;
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--tracks-dir", required=True, help="directory with gene_models, transcripts, exons, introns, gene_architecture")
    ap.add_argument("--fai", required=True, help="FASTA index; gives the contig order of the output")
    ap.add_argument("--out-parquet", required=True)
    ap.add_argument("--out-summary", required=True)
    add_resource_args(ap)
    args = ap.parse_args()

    con = duckdb.connect()
    con.execute(f"SET threads = {args.threads}; SET memory_limit = '{effective_memory_mb(args.memory_mb)}MB'")
    for name in ("gene_models", "transcripts", "exons", "introns", "gene_architecture"):
        path = str(Path(args.tracks_dir) / f"{name}.parquet").replace("'", "''")
        con.execute(f"CREATE VIEW {name} AS SELECT * FROM read_parquet('{path}')")
    check_preconditions(con)
    con.execute(PAIRS_SQL)

    con.execute("CREATE TABLE contig_rank (contig VARCHAR, rank INTEGER)")
    for i, line in enumerate(Path(args.fai).read_text().splitlines()):
        con.execute("INSERT INTO contig_rank VALUES (?, ?)", [line.split("\t")[0], i])
    Path(args.out_parquet).parent.mkdir(parents=True, exist_ok=True)
    con.execute(f"""COPY (SELECT p.* FROM pairs p LEFT JOIN contig_rank r USING (contig)
        ORDER BY r.rank, host_start, host_end, host_gene_id, host_locus, guest_start, guest_end, guest_gene_id, guest_locus)
        TO '{str(args.out_parquet).replace(chr(39), chr(39) * 2)}' (FORMAT PARQUET, COMPRESSION ZSTD)""")

    scalar = lambda sql: con.execute(sql).fetchone()[0]
    rows = {
        "genes": scalar("SELECT count(*) FROM genes"),
        "gene_ids": scalar("SELECT count(*) FROM (SELECT DISTINCT contig, gene_id FROM genes)"),
        "locus_breaks_from_abutting_transcripts_only": scalar("SELECT coalesce(sum(abut), 0) FROM tx_locus"),
        "gene_ids_split_into_loci": scalar("SELECT count(*) FROM (SELECT 1 FROM genes GROUP BY contig, gene_id HAVING count(*) > 1)"),
        "nested_pairs": scalar("SELECT count(*) FROM pairs"),
        "direct_pairs": scalar("SELECT count(*) FROM pairs WHERE is_direct"),
        "distinct_hosts": scalar("SELECT count(DISTINCT (contig, host_gene_id, host_locus)) FROM pairs"),
        "distinct_guests": scalar("SELECT count(DISTINCT (contig, guest_gene_id, guest_locus)) FROM pairs"),
        "guests_with_several_direct_hosts": scalar(
            "SELECT count(*) FROM (SELECT 1 FROM pairs WHERE is_direct GROUP BY contig, guest_gene_id, guest_locus HAVING count(*) > 1)"),
        "max_hosts_of_one_guest": scalar(
            "SELECT coalesce(max(n), 0) FROM (SELECT count(*) n FROM pairs GROUP BY contig, guest_gene_id, guest_locus)"),
        "identical_span_pairs": scalar(
            "SELECT count(*) FROM genes a JOIN genes b ON a.contig = b.contig AND a.start_pos = b.start_pos "
            "AND a.end_pos = b.end_pos AND a.gix < b.gix"),
        "hosts_without_intron": scalar(
            "SELECT count(*) FROM (SELECT DISTINCT contig, host_gene_id, host_locus FROM pairs) h WHERE NOT EXISTS "
            "(SELECT 1 FROM tx_locus tl JOIN introns i ON i.contig = tl.contig AND i.transcript_id = tl.transcript_id "
            "WHERE tl.contig = h.contig AND tl.gene_id = h.host_gene_id AND tl.locus = h.host_locus)"),
    }
    for col in ("relationship_type", "strand_relation", "guest_coding_class"):
        for value, n in con.execute(f"SELECT {col}, count(*) FROM pairs GROUP BY 1 ORDER BY 1").fetchall():
            rows[f"{col}={value}"] = n
    Path(args.out_summary).write_text("".join(f"{k}\t{v}\n" for k, v in rows.items()))


if __name__ == "__main__":
    main()
