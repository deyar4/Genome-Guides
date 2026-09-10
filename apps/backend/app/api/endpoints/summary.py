from fastapi import APIRouter, Depends, HTTPException
from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService
from app.schemas.track import SummaryResponse

router = APIRouter()

@router.get("/{genome}/{track_name}", response_model=SummaryResponse)
def get_global_summary(
    genome: str,
    track_name: str,
    track_service: DuckDBTrackService = Depends(get_track_service)
):
    """
    Get a global summary of a track for the entire genome.
    """
    try:
        data = track_service.query_track_range(
            genome=genome,
            contig="",
            track_name=track_name,
            start_pos=0,
            end_pos=0
        )
        return SummaryResponse(
            genome=genome,
            track_name=track_name,
            data=data
        )
    except FileNotFoundError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to query summary: {str(e)}")
