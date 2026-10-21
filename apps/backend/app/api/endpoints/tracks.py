from fastapi import APIRouter, Depends, HTTPException, Query
import duckdb
from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService
from app.schemas.track import TrackDataResponse
from app.core.track_meta import TRACK_METADATA
from app.core.validation import InvalidTrackRequest
import logging

logger = logging.getLogger(__name__)

router = APIRouter()

@router.get("/metadata")
def get_track_metadata():
    """Definitions (units, log base, window, weighting) of the window tracks, D17."""
    return TRACK_METADATA


@router.get("/{genome}/{contig}/{track_name}", response_model=TrackDataResponse)
def get_track_data(
    genome: str,
    contig: str,
    track_name: str,
    start: int = Query(..., ge=0, description="Start position (0-based, inclusive)"),
    end: int = Query(..., gt=0, description="End position (0-based, exclusive)"),
    max_points: int = Query(500, ge=1, le=10000, description="Max points for downsampling"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """
    Query a specific region for a track and optionally downsample the results.
    """
    try:
        data, truncated = track_service.query_track_range_ex(
            genome=genome,
            contig=contig,
            track_name=track_name,
            start_pos=start,
            end_pos=end,
            max_points=max_points
        )
        return TrackDataResponse(
            genome=genome,
            contig=contig,
            track_name=track_name,
            start=start,
            end=end,
            max_points=max_points,
            data=data,
            truncated=truncated,
        )
    except InvalidTrackRequest as e:
        raise HTTPException(status_code=422, detail=str(e))
    except FileNotFoundError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except (duckdb.Error, OSError, ValueError):
        logger.exception("track query failed: %s/%s/%s", genome, contig, track_name)
        raise HTTPException(status_code=500, detail="Failed to query track")
