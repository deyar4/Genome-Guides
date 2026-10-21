"""Synthetic-fixture tests for audit 05 (P1-P6, C8-C11): weighting, NULL/NaN, input validation.

Fixtures are written to a temp dir, so these tests run on CI without pipeline output.
"""
import duckdb
import pytest
from fastapi.testclient import TestClient

from app.api.dependencies import get_track_service
from app.main import app
from app.services.duckdb_track_service import DuckDBTrackService


@pytest.fixture(scope="module")
def client(tmp_path_factory):
    root = tmp_path_factory.mktemp("data")
    out = root / "tracks" / "synth" / "all"
    out.mkdir(parents=True)
    con = duckdb.connect()
    # gc_content: windows 0-1000 (G=300,C=100), 1000-2000 (poly-A: G=C=0, skew NaN), 2000-2010 (short: G=0,C=10)
    con.execute("""CREATE TABLE gc AS SELECT * FROM (VALUES
        ('c1', 0, 1000, 40.0::FLOAT, 50.0::FLOAT, 300, 100, 300, 300, 0),
        ('c1', 1000, 2000, 0.0::FLOAT, 'NaN'::FLOAT, 1000, 0, 0, 0, 0),
        ('c1', 2000, 2010, 100.0::FLOAT, -100.0::FLOAT, 0, 10, 0, 0, 0)
    ) t(contig, start_pos, end_pos, gc_pct, gc_skew, a_count, c_count, g_count, t_count, n_count)""")
    con.execute("""CREATE TABLE gc2 AS SELECT contig, start_pos::UINTEGER start_pos, end_pos::UINTEGER end_pos, gc_pct, gc_skew,
        a_count::UINTEGER a_count, c_count::UINTEGER c_count, g_count::UINTEGER g_count, t_count::UINTEGER t_count,
        n_count::UINTEGER n_count FROM gc""")
    con.execute(f"COPY gc2 TO '{out}/gc_content.parquet' (FORMAT PARQUET)")
    # hecate: full window mean_entropy 2.0 (n_valid 1000), short window 10.0 (n_valid 10), NULL-only window
    con.execute("""CREATE TABLE h AS SELECT * FROM (VALUES
        ('c1', 0, 1000, 0.5::FLOAT, 0.1::FLOAT, 0.9::FLOAT, 2.0::FLOAT, 1000),
        ('c1', 1000, 1010, 0.1::FLOAT, 0.1::FLOAT, 0.1::FLOAT, 10.0::FLOAT, 10),
        ('c1', 2000, 3000, NULL, NULL, NULL, NULL, 0)
    ) t(contig, start_pos, end_pos, mean_prob, min_prob, max_prob, mean_entropy, n_valid)""")
    con.execute(f"COPY h TO '{out}/hecate_mprob.parquet' (FORMAT PARQUET)")
    con.execute("""CREATE TABLE se AS SELECT * FROM (VALUES
        ('c1', 0, 1000, 1.9::FLOAT, 0, 1000),
        ('c1', 1000, 1100, NULL, 0, 100)
    ) t(contig, start_pos, end_pos, shannon_entropy, telomeric_repeats, n_acgt)""")
    con.execute(f"COPY se TO '{out}/shannon_entropy.parquet' (FORMAT PARQUET)")
    service = DuckDBTrackService(data_dir=str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app, raise_server_exceptions=False)
    app.dependency_overrides.clear()


def get(client, track, start=0, end=3000, max_points=500, contig="c1"):
    return client.get(f"/api/v1/tracks/synth/{contig}/{track}", params={"start": start, "end": end, "max_points": max_points})


def test_hecate_bin_is_weighted_by_n_valid(client):
    r = get(client, "hecate_mprob", end=3000, max_points=1)  # one bin: windows (2.0, n=1000), (10.0, n=10), (NULL)
    assert r.status_code == 200
    rows = r.json()["data"]
    assert len(rows) == 1
    expected = (2.0 * 1000 + 10.0 * 10) / 1010  # NOT the plain mean 6.0
    assert rows[0]["avg_mean_entropy"] == pytest.approx(expected, rel=1e-6)
    assert rows[0]["n_null_mean_entropy"] == 1
    assert rows[0]["sum_n_valid"] == 1010
    assert rows[0]["min_mean_entropy"] == pytest.approx(2.0) and rows[0]["max_mean_entropy"] == pytest.approx(10.0)


def test_null_window_stays_null_not_zero(client):
    rows = get(client, "hecate_mprob", start=2000, end=3000).json()["data"]
    assert rows[0]["avg_mean_entropy"] is None
    assert rows[0]["n_null_mean_entropy"] == 1
    rows = get(client, "shannon_entropy", start=1000, end=1100).json()["data"]
    assert rows[0]["avg_shannon_entropy"] is None


def test_gc_bin_uses_counts_and_survives_nan_skew(client):
    r = get(client, "gc_content", end=2010, max_points=1)
    assert r.status_code == 200  # NaN used to break JSON
    row = r.json()["data"][0]
    # sums over the three windows: G=300, C=110, A=1300, T=300
    assert row["avg_value"] == pytest.approx(100 * (300 + 110) / (1300 + 110 + 300 + 300), rel=1e-6)
    assert row["avg_skew"] == pytest.approx(100 * (300 - 110) / (300 + 110), rel=1e-6)
    assert row["n_null_skew"] == 1  # the poly-A window


def test_all_nan_skew_bin_is_null(client):
    row = get(client, "gc_content", start=1000, end=2000).json()["data"][0]
    assert row["avg_skew"] is None


def test_bins_are_multiples_of_window(client):
    rows = get(client, "gc_content", end=2010, max_points=3).json()["data"]
    assert all(r["bin_start"] % 1000 == 0 for r in rows)


def test_chromosome_list_with_nan_skew(client):
    r = client.get("/api/v1/dna/stats/chromosomes", params={"build": "synth"})
    assert r.status_code == 200
    assert r.json()["chromosomes"][0]["gc_skew"] == pytest.approx(100 * 190 / 410, rel=1e-6)


@pytest.mark.parametrize("path,params", [
    ("/api/v1/tracks/synth/c1' OR '1'='1/gc_content", {"start": 0, "end": 10}),
    ("/api/v1/tracks/synth%27/c1/gc_content", {"start": 0, "end": 10}),
    ("/api/v1/dna/stats/gaps", {"build": "synth", "chrom": "c1' OR '1'='1"}),
    ("/api/v1/genes/search", {"build": "synth", "q": "x' OR '1'='1"}),
    ("/api/v1/dna/stats/chromosomes", {"build": ".."}),
])
def test_injection_and_traversal_rejected(client, path, params):
    assert client.get(path, params=params).status_code in (404, 422)
    assert client.get(path, params=params).status_code != 200


@pytest.mark.parametrize("params", [
    {"start": 0, "end": 10, "max_points": 0},
    {"start": 10, "end": 10},
    {"start": 10, "end": 5},
    {"start": -1, "end": 5},
])
def test_bad_ranges_are_422(client, params):
    assert client.get("/api/v1/tracks/synth/c1/gc_content", params=params).status_code == 422


def test_not_found_does_not_leak_path(client):
    r = client.get("/api/v1/tracks/synth/c1/nope", params={"start": 0, "end": 10})
    assert r.status_code == 404 and "/" not in r.json()["detail"]


def test_metadata_names_cross_entropy(client):
    meta = client.get("/api/v1/tracks/metadata").json()
    text = meta["hecate_mprob"]["columns"]["avg_mean_entropy"]
    assert "Cross-entropy rate" in text and "bits per base" in text


def test_gc_skew_and_window_bins_must_be_window_multiples(client):
    base = {"build": "synth"}
    assert client.get("/api/v1/dna/stats/c1/gc-skew", params={**base, "window_size": 1500}).status_code == 422
    r = client.get("/api/v1/dna/stats/c1/gc-skew", params={**base, "window_size": 2000})
    assert r.status_code == 200
    assert client.get("/api/v1/dna/sequence/window", params={**base, "chrom": "c1", "step": 1500}).status_code == 422
    assert client.get("/api/v1/dna/sequence/window", params={**base, "chrom": "c1", "step": 1000}).status_code == 200


@pytest.mark.parametrize("params", [{"k": 99}, {"k": 5}, {"top_n": -1}, {"top_n": 0}])
def test_kmers_bounds(client, params):
    r = client.get("/api/v1/dna/stats/c1/kmers", params={"build": "synth", **params})
    assert r.status_code == 422


def test_summary_returns_non_window_tables(client, tmp_path):
    from app.api.dependencies import get_track_service
    svc = app.dependency_overrides[get_track_service]()
    out = svc.data_dir / "tracks" / "synth" / "all"
    duckdb.connect().execute(f"COPY (SELECT 'A' AS amino_acid, 3 AS count) TO '{out}/aa.parquet' (FORMAT PARQUET)")
    r = client.get("/api/v1/summary/synth/aa")
    assert r.status_code == 200 and r.json()["data"] == [{"amino_acid": "A", "count": 3}]
