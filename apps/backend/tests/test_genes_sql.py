"""Gene endpoints use parameterised SQL (audit 05 injection finding). HTTP validation already returns 422 for quotes;
the service-level test checks the SQL layer on its own."""
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
    con.execute("""CREATE TABLE g AS SELECT * FROM (VALUES
        ('G1', 'ALPHA', 'protein_coding', 'c1', 100, 200, '+'),
        ('G2', 'O''BRIEN', 'lncRNA', 'c1', 500, 900, '-')
    ) t(gene_id, gene_name, biotype, contig, start_pos, end_pos, strand)""")
    con.execute(f"COPY g TO '{out}/gene_models.parquet' (FORMAT PARQUET)")
    con.execute(f"""COPY (SELECT 'c1' AS contig, 'G1' AS gene1_id, 'G2' AS gene2_id, CAST(300 AS UINTEGER) AS intergenic_dist)
        TO '{out}/gene_spacing.parquet' (FORMAT PARQUET)""")
    service = DuckDBTrackService(data_dir=str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app, raise_server_exceptions=False)
    app.dependency_overrides.clear()


def test_search_matches(client):
    r = client.get("/api/v1/genes/search", params={"build": "synth", "q": "alp"})
    assert [x["gene_id"] for x in r.json()] == ["G1"]


def test_quote_is_data_at_service_level(client, tmp_path_factory):
    service = app.dependency_overrides[get_track_service]()
    path = service.data_dir / "tracks" / "synth" / "all" / "gene_models.parquet"
    sql = f"SELECT gene_id FROM read_parquet('{path}') WHERE gene_name = ?"
    assert service.execute_query(sql, ["O'BRIEN"]) == [{"gene_id": "G2"}]
    assert service.execute_query(sql, ["x' OR '1'='1"]) == []


def test_region_and_density(client):
    r = client.get("/api/v1/genes/region", params={"build": "synth", "chrom": "c1", "start": 0, "end": 300})
    assert [x["gene_id"] for x in r.json()] == ["G1"]
    r = client.get("/api/v1/genes/region", params={"build": "synth", "chrom": "c1' OR '1'='1", "start": 0, "end": 3000})
    assert r.status_code in (200, 422) and not (r.status_code == 200 and r.json())
    r = client.get("/api/v1/genes/density", params={"build": "synth", "chrom": "c1", "bin_size": 400})
    assert r.json() == [{"start": 0, "count": 1}, {"start": 400, "count": 1}]


def test_gene_lookup(client):
    r = client.get("/api/v1/genes/ALPHA", params={"build": "synth"})
    assert r.status_code == 200 and r.json()["gene_id"] == "G1"
    r = client.get("/api/v1/genes/x' OR '1'='1", params={"build": "synth"})
    assert r.status_code in (404, 422)


def test_spacing_reports_the_gap_interval(client):
    # G1 ends at 200, G2 starts at 500: the gap is [200, 500), length 300
    r = client.get("/api/v1/genes/spacing", params={"build": "synth", "chrom": "c1"})
    assert r.json() == [{"chrom": "c1", "gene_id_1": "G1", "gene_id_2": "G2", "start": 200, "end": 500, "distance": 300}]
    assert client.get("/api/v1/genes/spacing", params={"build": "synth", "chrom": "c2"}).json() == []
