"""/dna/stats/summary when tracks are missing: values are reported as missing (null), never guessed."""
import pytest
from fastapi.testclient import TestClient

from app.api.dependencies import get_track_service
from app.main import app
from app.services.duckdb_track_service import DuckDBTrackService


@pytest.fixture()
def client(tmp_path):
    out = tmp_path / "tracks" / "s" / "all"
    out.mkdir(parents=True)
    (out / "sequence_stats.txt").write_text("num_seqs\tsum_len\tN50\tL50\tGC(%)\tsum_n\n2\t1000\t600\t1\t40.0\t100\n")
    service = DuckDBTrackService(str(tmp_path))
    app.dependency_overrides[get_track_service] = lambda: service
    yield TestClient(app)
    app.dependency_overrides.clear()


def test_no_window_counts_means_null_composition_not_a_guess(client):
    d = client.get("/api/v1/dna/stats/summary", params={"build": "s"}).json()
    assert d["nuclear_composition"] is None and d["mitochondrial_composition"] is None
    assert d["assembly"]["total_length"] == 1000 and d["assembly"]["n50"] == 600
    assert d["assembly"]["n90"] is None and d["assembly"]["median_length"] is None


def test_no_gene_models_means_null_gene_counts(client):
    d = client.get("/api/v1/dna/stats/summary", params={"build": "s"}).json()
    assert d["gene_count"] is None and d["protein_coding_count"] is None


def test_missing_stats_file_is_404(tmp_path):
    (tmp_path / "tracks" / "e" / "all").mkdir(parents=True)
    service = DuckDBTrackService(str(tmp_path))
    app.dependency_overrides[get_track_service] = lambda: service
    try:
        assert TestClient(app).get("/api/v1/dna/stats/summary", params={"build": "e"}).status_code == 404
    finally:
        app.dependency_overrides.clear()
