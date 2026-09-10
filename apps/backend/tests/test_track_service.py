import pytest
from pathlib import Path
from app.services.duckdb_track_service import DuckDBTrackService

def test_duckdb_query_gc_track():
    service = DuckDBTrackService(data_dir="data")
    if not (service.data_dir / "tracks" / "tiny" / "all" / "gc_content.parquet").exists():
        pytest.skip("generated pipeline fixtures are not available")

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
    assert "avg_skew" in first_bin
    assert first_bin["avg_value"] >= 0.0

def test_duckdb_query_gap_track():
    service = DuckDBTrackService(data_dir="data")
    if not (service.data_dir / "tracks" / "tiny" / "all" / "assembly_gaps.parquet").exists():
        pytest.skip("generated pipeline fixtures are not available")

    # Query gaps track
    results = service.query_track_range(
        genome="tiny",
        contig="chr1",
        track_name="assembly_gaps",
        start_pos=0,
        end_pos=100
    )

    assert isinstance(results, list)

def test_duckdb_query_homopolymer_track():
    service = DuckDBTrackService(data_dir="data")
    if not (service.data_dir / "tracks" / "tiny" / "all" / "homopolymer_genomic_tracts.parquet").exists():
        pytest.skip("generated pipeline fixtures are not available")

    # Query homopolymers track
    results = service.query_track_range(
        genome="tiny",
        contig="chr1",
        track_name="homopolymer_genomic_tracts",
        start_pos=0,
        end_pos=100
    )

    assert isinstance(results, list)
