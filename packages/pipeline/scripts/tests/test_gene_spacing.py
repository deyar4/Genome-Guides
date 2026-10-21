"""Covered-span gene spacing (D39): half-open boundaries, nesting, ties, and a brute-force interval-union oracle."""
import random
import sys
from pathlib import Path

import duckdb

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "packages/pipeline/scripts/genes"))
from extract_gene_spacing import process_spacing  # noqa: E402


def run(tmp_path, genes):
    """genes: (contig, start, end, id, strand). Returns (spacing, overlaps) as lists of tuples."""
    src = tmp_path / "gm.parquet"
    con = duckdb.connect()
    values = ", ".join(f"('{c}', {s}, {e}, '{g}', '{st}')" for c, s, e, g, st in genes)
    con.execute(f"COPY (SELECT * FROM (VALUES {values}) t(contig, start_pos, end_pos, gene_id, strand)) TO '{src}' (FORMAT PARQUET)")
    process_spacing(str(src), str(tmp_path / "out"))
    sp = con.execute("SELECT contig, gene1_id, gene2_id, intergenic_dist FROM read_parquet(?) ORDER BY 1, 3",
                     [str(tmp_path / "out/gene_spacing.parquet")]).fetchall()
    ov = con.execute("SELECT contig, gene1_id, gene2_id, overlap_len, same_strand, contained FROM read_parquet(?) ORDER BY 1, 3",
                     [str(tmp_path / "out/gene_overlaps.parquet")]).fetchall()
    return sp, ov


def test_adjacent_is_spacing_zero_and_partial_overlap(tmp_path):
    sp, ov = run(tmp_path, [("c1", 0, 100, "a", "+"), ("c1", 100, 200, "b", "+"), ("c1", 200, 300, "c", "+"),
                            ("c1", 250, 400, "d", "-"), ("c1", 500, 600, "e", "+")])
    assert sp == [("c1", "a", "b", 0), ("c1", "b", "c", 0), ("c1", "d", "e", 100)]
    assert ov == [("c1", "c", "d", 50, 0, 0)]


def test_nested_genes_measured_from_covered_span(tmp_path):
    sp, ov = run(tmp_path, [("c1", 100, 1000, "A", "+"), ("c1", 200, 300, "B", "-"), ("c1", 500, 600, "C", "+"),
                            ("c1", 1000, 1100, "D", "+"), ("c1", 1200, 1300, "E", "+")])
    assert ov == [("c1", "A", "B", 100, 0, 1), ("c1", "A", "C", 100, 1, 1)]  # intersection = nested gene length
    assert sp == [("c1", "A", "D", 0), ("c1", "D", "E", 100)]  # no spurious gap of 200 after B


def test_chain_overlap_uses_max_end_gene(tmp_path):
    sp, ov = run(tmp_path, [("c1", 0, 100, "A", "+"), ("c1", 50, 150, "B", "+"), ("c1", 120, 200, "C", "+")])
    assert sp == [] and ov == [("c1", "A", "B", 50, 1, 0), ("c1", "B", "C", 30, 1, 0)]


def test_same_start_and_ties_are_order_independent(tmp_path):
    g = [("c1", 0, 100, "x", "+"), ("c1", 0, 100, "y", "-"), ("c1", 0, 50, "z", "+"), ("c1", 100, 120, "w", "+")]
    (tmp_path / "a").mkdir()
    a = run(tmp_path / "a", g)
    (tmp_path / "b").mkdir()
    b = run(tmp_path / "b", list(reversed(g)))
    assert a == b
    sp, ov = a
    assert sp == [("c1", "y", "w", 0)]  # tie on max end 100: largest gene_id 'y' is the covering gene
    assert ("c1", "x", "y", 100, 0, 1) in ov


def test_one_base_gap_single_gene_and_contig_isolation(tmp_path):
    sp, ov = run(tmp_path, [("c1", 0, 100, "a", "+"), ("c1", 101, 200, "b", "+"), ("c2", 0, 10, "s", "+"),
                            ("c3", 5, 9, "p", "+"), ("c3", 5, 9, "q", "+")])
    assert sp == [("c1", "a", "b", 1)] and ov == [("c3", "p", "q", 4, 1, 1)]


def merged_gaps(genes):
    out = []
    for contig in sorted({g[0] for g in genes}):
        iv = sorted((s, e) for c, s, e, _, _ in genes if c == contig)
        end = iv[0][1]
        for s, e in iv[1:]:
            if s > end:
                out.append((contig, s - end))
            end = max(end, e)
    return sorted(out)


def test_gaps_equal_complement_of_union_on_random_genes(tmp_path):
    rng = random.Random(7)  # seed is test-local, not a pipeline parameter
    genes = []
    for i in range(300):
        s = rng.randrange(0, 20000)
        genes.append((f"c{i % 3}", s, s + rng.randrange(1, 600), f"g{i:04d}", rng.choice("+-")))
    sp, ov = run(tmp_path, genes)
    got = sorted((c, d) for c, _, _, d in sp if d > 0)
    assert got == sorted(x for x in merged_gaps(genes) if x[1] > 0)
    # zero-length gaps (abutting) plus positive gaps plus overlaps plus first genes account for every gene
    n_first = len({g[0] for g in genes})
    assert len(sp) + len(ov) + n_first == len(genes)
