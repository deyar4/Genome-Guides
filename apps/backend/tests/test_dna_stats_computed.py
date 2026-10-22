"""/dna/stats/* fields computed from tracks (no placeholders). Hand-checkable synthetic fixture."""
import duckdb
import pytest
from fastapi.testclient import TestClient

from app.api.dependencies import get_track_service
from app.main import app
from app.services.duckdb_track_service import DuckDBTrackService


@pytest.fixture(scope="module")
def client(tmp_path_factory):
    root = tmp_path_factory.mktemp("data")
    out = root / "tracks" / "s" / "all"
    out.mkdir(parents=True)
    con = duckdb.connect()
    # contig lengths (a+c+g+t+n): c1 = 600, c2 = 300, c3 = 100, chrM = 50
    con.execute(f"""COPY (SELECT contig, start_pos::UINTEGER start_pos, end_pos::UINTEGER end_pos,
        0::FLOAT gc_pct, 0::FLOAT gc_skew, a::UINTEGER a_count, c::UINTEGER c_count, g::UINTEGER g_count,
        t::UINTEGER t_count, n::UINTEGER n_count FROM (VALUES
        ('c1', 0, 500, 100, 100, 100, 100, 100),
        ('c1', 500, 600, 50, 20, 20, 10, 0),
        ('c2', 0, 300, 100, 50, 50, 100, 0),
        ('c3', 0, 100, 10, 40, 40, 10, 0),
        ('chrM', 0, 50, 20, 5, 5, 20, 0)
        ) t(contig, start_pos, end_pos, a, c, g, t, n)) TO '{out}/gc_content.parquet' (FORMAT PARQUET)""")
    con.execute(f"""COPY (SELECT * FROM (VALUES ('c1', 1, 5), ('c1', 10, 20), ('c2', 3, 4))
        t(chrom, start, "end")) TO '{out}/ssr_hits.parquet' (FORMAT PARQUET)""")
    con.execute(f"""COPY (SELECT * FROM (VALUES ('c1','g1','protein_coding'), ('c1','g2','lncRNA'),
        ('c2','g1','protein_coding')) t(contig, gene_id, biotype)) TO '{out}/gene_models.parquet' (FORMAT PARQUET)""")
    (out / "sequence_stats.txt").write_text(
        "num_seqs\tsum_len\tN50\tL50\tGC(%)\tsum_n\n4\t1050\t600\t1\t40.0\t100\n")
    service = DuckDBTrackService(str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app)
    app.dependency_overrides.clear()


def test_summary_assembly_lengths_and_n90(client):
    a = client.get("/api/v1/dna/stats/summary", params={"build": "s"}).json()["assembly"]
    # sorted lengths 600, 300, 100, 50 (total 1050): cumulative 600 (57 %) -> N90 needs 945: 600+300+100 = 1000 -> 100, L90 = 3
    assert (a["n90"], a["l90"]) == (100, 3)
    assert (a["min_len"], a["max_len"]) == (50, 600)
    assert a["mean_length"] == pytest.approx(1050 / 4)
    assert a["median_length"] == pytest.approx((300 + 100) / 2)


def test_summary_compositions_are_exact_and_split_chrM(client):
    d = client.get("/api/v1/dna/stats/summary", params={"build": "s"}).json()
    nuc, mito = d["nuclear_composition"], d["mitochondrial_composition"]
    assert (nuc["a"], nuc["c"], nuc["g"], nuc["t"], nuc["n"]) == (260, 210, 210, 220, 100)
    assert nuc["gc_content"] == pytest.approx(100 * 420 / 900)
    assert (mito["a"], mito["c"], mito["g"], mito["t"], mito["n"]) == (20, 5, 5, 20, 0)
    assert mito["gc_content"] == pytest.approx(20.0)


def test_summary_gene_counts_keep_same_id_on_two_contigs(client):
    d = client.get("/api/v1/dna/stats/summary", params={"build": "s"}).json()
    assert (d["gene_count"], d["protein_coding_count"]) == (3, 2)


def test_chromosome_ssr_counts(client):
    rows = {c["name"]: c for c in client.get("/api/v1/dna/stats/chromosomes", params={"build": "s"}).json()["chromosomes"]}
    assert (rows["c1"]["ssr_count"], rows["c2"]["ssr_count"], rows["c3"]["ssr_count"]) == (2, 1, 0)
    assert "dinucleotides" not in rows["c1"]
