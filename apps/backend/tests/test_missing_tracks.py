"""A missing track is a 404 ("not computed"), never an empty list or zeros that look like a real result."""
import pytest
from fastapi.testclient import TestClient

from app.api.dependencies import get_track_service
from app.main import app
from app.services.duckdb_track_service import DuckDBTrackService

B = {"build": "empty"}
ROUTES = [
    ("/dna/telomeres", {}), ("/dna/centromeres", {}), ("/dna/ssr", {"chrom": "c"}), ("/dna/ssr/summary", {}),
    ("/dna/ssr/summary", {"chrom": "c"}), ("/dna/ssr/density", {"chrom": "c"}), ("/dna/ssr/coverage", {}),
    ("/dna/isochores", {"chrom": "c"}),
    ("/genes/top/largest", {}), ("/genes/top/most-exons", {}), ("/genes/stats/biotypes", {}), ("/genes/search", {"q": "ab"}),
    ("/genes/stats/sizes", {}),
    ("/rna/splice-sites", {}), ("/rna/splice-sites/records", {}),
    ("/proteins/stats/size", {}), ("/proteins/top/largest", {}), ("/proteins/rich", {"aa": "A"}),
]


@pytest.fixture(scope="module")
def client(tmp_path_factory):
    root = tmp_path_factory.mktemp("data")
    (root / "tracks" / "empty" / "all").mkdir(parents=True)
    service = DuckDBTrackService(str(root))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app, raise_server_exceptions=False)
    app.dependency_overrides.clear()


@pytest.mark.parametrize("path,params", ROUTES)
def test_missing_track_is_404(client, path, params):
    r = client.get(f"/api/v1{path}", params={**B, **params})
    assert r.status_code == 404, (path, r.status_code, r.text[:120])
    assert "/" not in r.json()["detail"].replace("/api", ""), "no file path in the message"
