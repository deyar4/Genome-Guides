import pytest
from fastapi.testclient import TestClient
from app.main import app

client = TestClient(app)

def test_health_check():
    response = client.get("/api/health")
    assert response.status_code == 200
    assert response.json() == {"status": "ok", "service": "Genome Guides Backend Engine"}

def test_get_track_data_not_found():
    # Query a non-existent track to trigger FileNotFoundError / 404
    response = client.get(
        "/api/tracks/nonexistent_genome/chr1/gc_content",
        params={"start": 0, "end": 1000}
    )
    assert response.status_code == 404
    assert "not found" in response.json()["detail"].lower()

def test_get_track_data_success():
    # Query the synthetic track generated for test_pipeline
    response = client.get(
        "/api/tracks/tiny/chr1/gc_content",
        params={"start": 0, "end": 5000, "max_points": 10}
    )
    assert response.status_code == 200
    res_data = response.json()
    assert res_data["genome"] == "tiny"
    assert res_data["contig"] == "chr1"
    assert res_data["track_name"] == "gc_content"
    assert len(res_data["data"]) > 0
    assert "avg_value" in res_data["data"][0]
