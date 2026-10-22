"""The number-check scripts (scripts/checks) on small synthetic inputs with hand-computed expectations."""
import shutil
import subprocess
import sys
from pathlib import Path

import duckdb
import pytest

ROOT = Path(__file__).resolve().parents[4]
CHECKS = ROOT / "packages/pipeline/scripts/checks"
sys.path.insert(0, str(CHECKS))
from cpg_grid_shift import intersection_bp  # noqa: E402


def read_tsv(path):
    rows = [line.split("\t") for line in Path(path).read_text().splitlines()]
    return {r[0]: r[1] for r in rows[1:]}


def test_intersection_bp():
    a = [(0, 10), (20, 30)]
    b = [(5, 25), (28, 40)]
    assert intersection_bp(a, b) == 5 + 5 + 2  # [5,10) + [20,25) + [28,30)
    assert intersection_bp(a, []) == 0


def test_gene_unit_checks_hand_computed(tmp_path):
    con = duckdb.connect()
    con.execute("""COPY (SELECT * FROM (VALUES
        ('c1', 'G1', 'NM_1', '+', 0, 100), ('c1', 'G1', 'NR_1', '+', 10, 50),
        ('c1', 'TR', 'NR_2', '+', 200, 300), ('c1', 'TR', 'NR_3', '+', 500, 600),
        ('c1', 'G2', 'XM_1', '-', 1000, 1100), ('c1', 'G3', 'NR_4', '+', 2000, 2100)
    ) t(contig, gene_id, transcript_id, strand, start_pos, end_pos)) TO '%s/transcripts.parquet' (FORMAT PARQUET)""" % tmp_path)
    con.execute("""COPY (SELECT * FROM (VALUES
        ('NM_1', 90), ('NR_1', 0), ('NR_2', 0), ('NR_3', 0), ('XM_1', 60), ('NR_4', 30)
    ) t(transcript_id, cds_bases)) TO '%s/gene_architecture.parquet' (FORMAT PARQUET)""" % tmp_path)
    con.execute("""COPY (SELECT * FROM (VALUES
        ('c1', 'G1', 'g1'), ('c1', 'TR', 'tr'), ('c1', 'G2', 'g2'), ('c1', 'G3', 'g3')
    ) t(contig, gene_id, gene_name)) TO '%s/gene_models.parquet' (FORMAT PARQUET)""" % tmp_path)
    out = tmp_path / "o.tsv"
    subprocess.run([sys.executable, str(CHECKS / "gene_unit_checks.py"), "--tracks-dir", str(tmp_path), "--out", str(out)], check=True)
    r = read_tsv(out)
    assert r["gene_keys_contig_gene_id"] == "4"
    assert r["gene_ids_split_into_loci"] == "1" and r["loci_of_split_gene_ids"] == "2"  # TR: [200,300) and [500,600)
    assert r["gene_units_loci"] == "5"
    assert r["genes_prefix_mixed_with_cds"] == "1"            # G1: NM_ + NR_, CDS 90
    assert r["genes_prefix_protein_coding_with_cds"] == "1"   # G2
    assert r["genes_prefix_non_coding_with_cds"] == "1"       # G3 (NR_ with CDS)
    assert r["genes_prefix_non_coding_without_cds"] == "1"    # TR


@pytest.mark.skipif(shutil.which("samtools") is None, reason="samtools not on PATH")
def test_recount_passes_on_true_island_and_fails_on_wrong_count(tmp_path):
    seq = "A" * 400 + "CG" * 200 + "T" * 400  # island [400, 800): 200 CG, gc 100 %, obs/exp = 200*400/(200*200) = 2
    fa = tmp_path / "x.fa"
    fa.write_text(">c\n" + "\n".join(seq[i:i + 60] for i in range(0, len(seq), 60)) + "\n")
    subprocess.run(["samtools", "faidx", str(fa)], check=True)

    def write_parquet(path, cg):
        duckdb.connect().execute(f"""COPY (SELECT 'c' AS contig, 400::UBIGINT AS start_pos, 800::UBIGINT AS end_pos,
            400::UINTEGER AS length, 100.0::FLOAT AS gc_pct, 2.0::FLOAT AS obs_exp_ratio, {cg}::UBIGINT AS cpg_count)
            TO '{path}' (FORMAT PARQUET)""")

    cfg = ROOT / "packages/pipeline/config.yaml"
    good, bad = tmp_path / "good.parquet", tmp_path / "bad.parquet"
    write_parquet(good, 200)
    write_parquet(bad, 199)
    args = [sys.executable, str(CHECKS / "cpg_island_recount.py"), "--fasta", str(fa), "--config", str(cfg), "--n", "5"]
    assert subprocess.run(args + ["--parquet", str(good), "--out", str(tmp_path / "g.tsv")]).returncode == 0
    assert read_tsv(tmp_path / "g.tsv")["mismatch_cpg_count"] == "0"
    assert subprocess.run(args + ["--parquet", str(bad), "--out", str(tmp_path / "b.tsv")]).returncode == 1
    assert read_tsv(tmp_path / "b.tsv")["mismatch_cpg_count"] == "1"
