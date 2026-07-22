import pytest
from app.services.duckdb_track_service import DuckDBTrackService

def test_duckdb_query_chrm_real_data():
    service = DuckDBTrackService(data_dir="data")
    
    # Query real mitochondrial genome track downloaded & processed by Snakemake
    results = service.query_track_range(
        genome="chrm",
        contig="chrM",
        track_name="gc_content",
        start_pos=0,
        end_pos=16569,
        max_points=20
    )
    
    assert len(results) > 0
    first_bin = results[0]
    assert "bin_start" in first_bin
    assert "avg_value" in first_bin
    # Human mitochondrial genome GC content is ~44%
    assert 30.0 <= first_bin["avg_value"] <= 60.0
