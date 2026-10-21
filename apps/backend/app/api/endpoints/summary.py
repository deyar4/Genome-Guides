from fastapi import APIRouter, Depends, HTTPException
import duckdb
from app.api.dependencies import get_track_service
from app.services.duckdb_track_service import DuckDBTrackService
from app.schemas.track import SummaryResponse
from app.core.validation import InvalidTrackRequest
import logging

logger = logging.getLogger(__name__)

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
            contig="all",
            track_name=track_name,
            start_pos=0,
            end_pos=2**40
        )
        return SummaryResponse(
            genome=genome,
            track_name=track_name,
            data=data
        )
    except FileNotFoundError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except InvalidTrackRequest as e:
        raise HTTPException(status_code=422, detail=str(e))
    except (duckdb.Error, OSError, ValueError):
        logger.exception("summary query failed: %s/%s", genome, track_name)
        raise HTTPException(status_code=500, detail="Failed to query summary")
