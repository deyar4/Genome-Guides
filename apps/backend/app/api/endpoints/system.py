from fastapi import APIRouter, Depends

from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService

router = APIRouter()


def _builds(track_service: DuckDBTrackService) -> list[str]:
    """Builds that have at least one track file under tracks/<build>/all/ (never a hardcoded list)."""
    root = track_service.data_dir / "tracks"
    if not root.is_dir():
        return []
    return sorted(p.name for p in root.iterdir() if (p / "all").is_dir() and any((p / "all").glob("*.parquet")))


@router.get("/genomes")
def get_genomes(track_service: DuckDBTrackService = Depends(get_track_service)):
    """Lists builds that have at least one .parquet file under tracks/<build>/all/ of the configured data directory, sorted by
    name. The list is derived from the filesystem, never hardcoded; it is [] when there is no tracks directory."""
    return _builds(track_service)


@router.get("/health")
def get_health():
    """Liveness check: always returns {status: 'ok', version: '0.1.0'} (version is a literal in the code) and touches no data."""
    return {"status": "ok", "version": "0.1.0"}


@router.get("/ready")
def get_ready(track_service: DuckDBTrackService = Depends(get_track_service)):
    """Readiness check: status is 'ready' when at least one build with track files exists (same rule as /system/genomes), else
    'not_ready'; components.tracks reports the number of builds or 'no track files found'. Always HTTP 200."""
    builds = _builds(track_service)
    return {"status": "ready" if builds else "not_ready",
            "components": {"tracks": f"{len(builds)} build(s)" if builds else "no track files found"}}
