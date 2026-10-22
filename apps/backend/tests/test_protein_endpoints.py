"""Protein endpoints: homopolymer filters are applied by the server; unmapped fields are null, not fabricated."""
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
    con.execute("""CREATE TABLE h AS SELECT * FROM (VALUES
        ('P1', 'Q', 5, 0, 5), ('P2', 'A', 12, 3, 15), ('P3', 'E', 8, 1, 9), ('P4', 'L', 12, 0, 12)
    ) t(protein_id, amino_acid, repeat_len, start_pos, end_pos)""")
    con.execute(f"COPY h TO '{out}/protein_homopolymers.parquet' (FORMAT PARQUET)")
    con.execute("""CREATE TABLE l AS SELECT * FROM (VALUES ('P1', 10), ('P2', 30)) t(protein_id, length)""")
    con.execute(f"COPY l TO '{out}/protein_lengths.parquet' (FORMAT PARQUET)")
    service = DuckDBTrackService(data_dir=str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app, raise_server_exceptions=False)
    app.dependency_overrides.clear()


def test_homopolymers_min_length_limit_and_order(client):
    r = client.get("/api/v1/proteins/tracts/homopolymers", params={"build": "synth", "min_length": 8}).json()
    assert [(t["protein_id"], t["length"]) for t in r] == [("P2", 12), ("P4", 12), ("P3", 8)]
    r = client.get("/api/v1/proteins/tracts/homopolymers", params={"build": "synth", "limit": 1}).json()
    assert [t["protein_id"] for t in r] == ["P2"]


def test_largest_does_not_fabricate_gene_mapping(client):
    r = client.get("/api/v1/proteins/top/largest", params={"build": "synth"}).json()
    assert r[0]["protein_id"] == "P2"
    for key in ("gene_id", "gene_name", "transcript_id", "sequence", "is_mitochondrial"):
        assert r[0][key] is None
