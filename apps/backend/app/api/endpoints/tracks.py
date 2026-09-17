from fastapi import APIRouter, Depends, HTTPException, Query
import duckdb
from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService
from app.schemas.track import TrackDataResponse

router = APIRouter()

@router.get("/{genome}/{contig}/{track_name}", response_model=TrackDataResponse)
def get_track_data(
    genome: str,
    contig: str,
    track_name: str,
    start: int = Query(..., description="Start position (0-based, inclusive)"),
    end: int = Query(..., description="End position (0-based, exclusive)"),
    max_points: int = Query(500, description="Max points for downsampling"),
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """
    Query a specific region for a track and optionally downsample the results.
    """
    try:
        data = track_service.query_track_range(
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
            data=data
        )
    except FileNotFoundError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except (duckdb.Error, OSError, ValueError) as exc:
        raise HTTPException(status_code=500, detail=f"Failed to query track: {exc}") from exc
