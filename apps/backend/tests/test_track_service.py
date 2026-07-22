import pytest
from pathlib import Path
from app.services.duckdb_track_service import DuckDBTrackService

def test_duckdb_query_gc_track():
    service = DuckDBTrackService(data_dir="data")
    
    # Query synthetic track generated during test_pipeline
    results = service.query_track_range(
        genome="tiny",
        contig="chr1",
        track_name="gc_content",
        start_pos=0,
        end_pos=5000,
        max_points=10
    )
    
    assert len(results) > 0
    first_bin = results[0]
    assert "bin_start" in first_bin
    assert "avg_value" in first_bin
    assert first_bin["avg_value"] >= 0.0
