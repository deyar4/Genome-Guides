"""Environment-driven deployment settings: CORS origins, DuckDB limits, data directory."""
import pytest
from fastapi.testclient import TestClient
from pydantic import ValidationError

from app.core.config import Settings
from app.services.duckdb_track_service import DuckDBTrackService


def test_defaults_keep_development_behaviour():
    s = Settings(_env_file=None)
    assert s.DATA_DIR == "data" and s.cors_origin_list == ["*"]
    assert s.DUCKDB_THREADS is None and s.DUCKDB_MEMORY_LIMIT is None


def test_empty_strings_mean_unset():
    s = Settings(_env_file=None, DUCKDB_THREADS="", DUCKDB_MEMORY_LIMIT="  ")
    assert s.DUCKDB_THREADS is None and s.DUCKDB_MEMORY_LIMIT is None


def test_cors_origin_list_is_split_and_trimmed():
    s = Settings(_env_file=None, CORS_ORIGINS=" https://a.example , https://b.example,,")
    assert s.cors_origin_list == ["https://a.example", "https://b.example"]


@pytest.mark.parametrize("kwargs", [{"DUCKDB_THREADS": 0}, {"DUCKDB_MEMORY_LIMIT": "4 gigs"}, {"DUCKDB_MEMORY_LIMIT": "1GB'; DROP"}])
def test_invalid_limits_are_rejected(kwargs):
    with pytest.raises(ValidationError):
        Settings(_env_file=None, **kwargs)


def test_service_applies_limits(tmp_path):
    svc = DuckDBTrackService(str(tmp_path), threads=2, memory_limit="512MB")
    assert svc.execute_query("SELECT current_setting('threads') AS t")[0]["t"] == 2
    limit = svc.execute_query("SELECT current_setting('memory_limit') AS m")[0]["m"]
    assert limit.split()[0].startswith("488") or limit.split()[0].startswith("512")  # 512 MB = 488.2 MiB


def test_cors_header_follows_settings(monkeypatch):
    from app.core import config
    monkeypatch.setattr(config.settings, "CORS_ORIGINS", "https://a.example")
    from app.main import create_app
    client = TestClient(create_app())
    r = client.get("/health", headers={"Origin": "https://a.example"})
    assert r.headers.get("access-control-allow-origin") == "https://a.example"
    r = client.get("/health", headers={"Origin": "https://evil.example"})
    assert "access-control-allow-origin" not in r.headers
