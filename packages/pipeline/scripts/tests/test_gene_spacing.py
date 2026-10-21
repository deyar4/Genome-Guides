"""Half-open boundary cases for extract_gene_spacing.py (audit 03, S11)."""
import sys
from pathlib import Path

import duckdb

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "packages/pipeline/scripts/genes"))
from extract_gene_spacing import process_spacing  # noqa: E402


def test_adjacent_genes_are_spacing_zero_not_overlap(tmp_path):
    src = tmp_path / "gm.parquet"
    con = duckdb.connect()
    # [0,100) [100,200) adjacent; [200,300) [250,400) overlap by 50; [500,600) after a gap of 100 from 400
    con.execute("""COPY (SELECT * FROM (VALUES
        ('c1', 0, 100, 'a', '+'), ('c1', 100, 200, 'b', '+'), ('c1', 200, 300, 'c', '+'),
        ('c1', 250, 400, 'd', '-'), ('c1', 500, 600, 'e', '+')
    ) t(contig, start_pos, end_pos, gene_id, strand)) TO '%s' (FORMAT PARQUET)""" % src)
    process_spacing(str(src), str(tmp_path / "out"))
    spacing = {(r[0], r[1]): r[2] for r in con.execute(
        "SELECT gene1_id, gene2_id, intergenic_dist FROM read_parquet(?)", [str(tmp_path / "out/gene_spacing.parquet")]).fetchall()}
    overlaps = {(r[0], r[1]): r[2] for r in con.execute(
        "SELECT gene1_id, gene2_id, overlap_len FROM read_parquet(?)", [str(tmp_path / "out/gene_overlaps.parquet")]).fetchall()}
    assert spacing == {("a", "b"): 0, ("b", "c"): 0, ("d", "e"): 100}
    assert overlaps == {("c", "d"): 50}
