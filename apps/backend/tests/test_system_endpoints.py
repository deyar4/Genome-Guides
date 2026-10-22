"""/system/genomes and /system/ready report what is on disk, not constants."""
import duckdb
from fastapi.testclient import TestClient

from app.api.dependencies import get_track_service
from app.main import app
from app.services.duckdb_track_service import DuckDBTrackService


def _client(root):
    service = DuckDBTrackService(data_dir=str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    return TestClient(app, raise_server_exceptions=False)


def test_empty_data_dir_is_not_ready(tmp_path):
    try:
        c = _client(tmp_path)
        assert c.get("/api/v1/system/genomes").json() == []
        assert c.get("/api/v1/system/ready").json()["status"] == "not_ready"
    finally:
        app.dependency_overrides.clear()


def test_builds_with_parquet_are_listed(tmp_path):
    out = tmp_path / "tracks" / "synth" / "all"
    out.mkdir(parents=True)
    (tmp_path / "tracks" / "empty" / "all").mkdir(parents=True)
    duckdb.connect().execute(f"COPY (SELECT 1 AS x) TO '{out}/a.parquet' (FORMAT PARQUET)")
    try:
        c = _client(tmp_path)
        assert c.get("/api/v1/system/genomes").json() == ["synth"]
        assert c.get("/api/v1/system/ready").json()["status"] == "ready"
    finally:
        app.dependency_overrides.clear()
