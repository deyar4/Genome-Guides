"""/dna/stats/dinucleotides: expected frequency is f(x) f(y) from the base counts, null (never an estimate) without them."""
import duckdb
import pytest
from fastapi.testclient import TestClient

from app.api.dependencies import get_track_service
from app.main import app
from app.services.duckdb_track_service import DuckDBTrackService


@pytest.fixture(scope="module")
def client(tmp_path_factory):
    root = tmp_path_factory.mktemp("data")
    con = duckdb.connect()
    for build in ("with_counts", "no_counts"):
        out = root / "tracks" / build / "all"
        out.mkdir(parents=True)
        con.execute(f"""COPY (SELECT * FROM (VALUES ('AC', 30, 30.0, 1.5), ('CA', 20, 20.0, 0.8))
            t(dinucleotide, count, frequency_pct, odds_ratio)) TO '{out}/dinucleotide_frequencies.parquet' (FORMAT PARQUET)""")
    out = root / "tracks" / "with_counts" / "all"
    # A = 40, C = 60 over one window (G = T = 0): f(A) = 0.4, f(C) = 0.6
    con.execute(f"""COPY (SELECT 'c1' contig, 0::UINTEGER start_pos, 100::UINTEGER end_pos, 0::FLOAT gc_pct, 0::FLOAT gc_skew,
        40::UINTEGER a_count, 60::UINTEGER c_count, 0::UINTEGER g_count, 0::UINTEGER t_count, 0::UINTEGER n_count)
        TO '{out}/gc_content.parquet' (FORMAT PARQUET)""")
    service = DuckDBTrackService(str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app)
    app.dependency_overrides.clear()


def test_expected_is_product_of_base_frequencies(client):
    r = client.get("/api/v1/dna/stats/dinucleotides", params={"build": "with_counts"}).json()
    by = {d["pair"]: d for d in r["dinucleotides"]}
    assert by["AC"]["expected_freq"] == pytest.approx(0.4 * 0.6)
    assert by["CA"]["expected_freq"] == pytest.approx(0.6 * 0.4)
    assert by["AC"]["observed_freq"] == pytest.approx(0.30)


def test_expected_is_null_without_base_counts(client):
    r = client.get("/api/v1/dna/stats/dinucleotides", params={"build": "no_counts"}).json()
    assert r["base_frequencies"] == {}
    assert [d["expected_freq"] for d in r["dinucleotides"]] == [None, None]
