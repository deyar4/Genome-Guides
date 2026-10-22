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
    return _builds(track_service)


@router.get("/health")
def get_health():
    return {"status": "ok", "version": "0.1.0"}


@router.get("/ready")
def get_ready(track_service: DuckDBTrackService = Depends(get_track_service)):
    builds = _builds(track_service)
    return {"status": "ready" if builds else "not_ready",
            "components": {"tracks": f"{len(builds)} build(s)" if builds else "no track files found"}}
