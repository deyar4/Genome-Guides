"""/dna/cpg-islands: total count, truncation flag, half-open overlap, strict filter, stable paging."""
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
    # file order = .fai order: chr2 is stored before chr10 on purpose
    rows = [("chr2", 0, 200), ("chr2", 400, 1000), ("chr10", 200, 400), ("chr10", 1000, 1200), ("chr10", 5000, 5600)]
    values = ", ".join(
        f"('{c}', {s}, {e}, {e - s}, 60.0, 0.9, 10, {(e - s) // 200}, {1 if e - s >= 500 else 0})" for c, s, e in rows)
    duckdb.connect().execute(f"""COPY (SELECT contig, CAST(start_pos AS UBIGINT) AS start_pos,
        CAST(end_pos AS UBIGINT) AS end_pos, CAST("length" AS UINTEGER) AS "length", CAST(gc_pct AS FLOAT) AS gc_pct,
        CAST(obs_exp_ratio AS FLOAT) AS obs_exp_ratio, CAST(cpg_count AS UBIGINT) AS cpg_count,
        CAST(n_windows AS UINTEGER) AS n_windows, CAST("strict" AS UTINYINT) AS "strict" FROM (VALUES {values})
        t(contig, start_pos, end_pos, "length", gc_pct, obs_exp_ratio, cpg_count, n_windows, "strict"))
        TO '{out}/cpg_islands.parquet' (FORMAT PARQUET)""")
    service = DuckDBTrackService(str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app)
    app.dependency_overrides.clear()


def get(client, **params):
    return client.get("/api/v1/dna/cpg-islands", params={"build": "s", **params})


def test_total_and_truncation_flag(client):
    r = get(client, limit=2).json()
    assert (r["total"], r["returned"], r["truncated"]) == (5, 2, True)
    r = get(client, limit=2, offset=4).json()
    assert (r["total"], r["returned"], r["truncated"]) == (5, 1, False)
    assert get(client).json()["truncated"] is False


def test_paging_is_stable_and_in_file_order(client):
    names = [(i["chrom"], i["start"]) for p in (0, 2, 4) for i in get(client, limit=2, offset=p).json()["islands"]]
    assert names == [("chr2", 0), ("chr2", 400), ("chr10", 200), ("chr10", 1000), ("chr10", 5000)]


def test_overlap_is_half_open(client):
    # chr10 island [200,400): a query starting at 400 or ending at 200 does not overlap it
    assert get(client, chrom="chr10", start=400, end=1000).json()["total"] == 0
    assert get(client, chrom="chr10", start=0, end=200).json()["total"] == 0
    assert get(client, chrom="chr10", start=399, end=401).json()["total"] == 1
    assert get(client, chrom="chr10", start=1199, end=5001).json()["total"] == 2


def test_strict_filter_and_fields(client):
    r = get(client, strict=True).json()
    assert r["total"] == 2 and all(i["strict"] and i["length"] >= 500 for i in r["islands"])
    first = get(client).json()["islands"][0]
    assert first["cpg_count"] == 10 and first["n_windows"] == 1 and first["gc_content"] == pytest.approx(60.0)


def test_bad_ranges_and_limits(client):
    assert get(client, start=10, end=10).status_code == 422
    assert get(client, limit=0).status_code == 422
    assert get(client, limit=10001).status_code == 422
