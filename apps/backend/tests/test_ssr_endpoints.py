"""SSR endpoints: filtering, paging and parameter binding on a small fixture (scale check was done once on 13.5 M synthetic rows)."""
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
    con.execute(f"""COPY (SELECT 'c'||(1+i%2) AS chrom, (i//2)*100 AS "start", (i//2)*100+20 AS "end", 'AC' AS motif,
        2 + (i//2)%2 AS period, 10 AS repeat_count, 'ACACACACACACACACACAC' AS sequence, 20 AS length,
        'x' AS period_name, 0.5 AS complexity FROM range(100) t(i) ORDER BY chrom, "start")
        TO '{out}/ssr_hits.parquet' (FORMAT PARQUET)""")
    con.execute(f"""COPY (SELECT 'c1' AS chrom, 0 AS "start", 1000 AS "end", 3 AS count) TO '{out}/ssr_density.parquet' (FORMAT PARQUET)""")
    service = DuckDBTrackService(data_dir=str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app, raise_server_exceptions=False)
    app.dependency_overrides.clear()


def ssr(client, **p):
    return client.get("/api/v1/dna/ssr", params={"build": "synth", **p})


def test_chrom_filter_order_and_paging(client):
    rows = ssr(client, chrom="c1", limit=5000).json()
    assert len(rows) == 50 and all(r["chrom"] == "c1" for r in rows)
    assert [r["start"] for r in rows] == sorted(r["start"] for r in rows)
    page = ssr(client, chrom="c1", limit=10, offset=45).json()
    assert [r["start"] for r in page] == [r["start"] for r in rows[45:]]


def test_interval_overlap_is_half_open(client):
    # c1 hits are [0,20) [100,120) ...: query [20,100) overlaps none, [19,101) overlaps two
    assert ssr(client, chrom="c1", start=20, end=100).json() == []
    assert [r["start"] for r in ssr(client, chrom="c1", start=19, end=101).json()] == [0, 100]


def test_period_and_min_repeats(client):
    assert {r["period"] for r in ssr(client, chrom="c1", period=3, limit=5000).json()} == {3}
    assert ssr(client, chrom="c1", min_repeats=11).json() == []


def test_unknown_chrom_empty_and_limit_bounds(client):
    assert ssr(client, chrom="nochr").json() == []
    assert ssr(client, chrom="c1", limit=5001).status_code == 422
    assert ssr(client, chrom="c1", period=7).status_code == 422


def test_quote_in_chrom_is_not_sql(client):
    r = ssr(client, chrom="c1' OR '1'='1")
    assert r.status_code in (200, 422) and not (r.status_code == 200 and r.json())


def test_density_filters_chrom(client):
    r = client.get("/api/v1/dna/ssr/density", params={"build": "synth", "chrom": "c1"})
    assert r.status_code == 200 and r.json()[0]["count"] == 3
    assert client.get("/api/v1/dna/ssr/density", params={"build": "synth", "chrom": "c2"}).json() == []
