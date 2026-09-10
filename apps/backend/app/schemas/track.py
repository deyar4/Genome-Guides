from pydantic import BaseModel, Field
from typing import Any, List, Optional

class TrackDataResponse(BaseModel):
    genome: str
    contig: str
    track_name: str
    start: int
    end: int
    max_points: int
    data: List[Any]

class SummaryResponse(BaseModel):
    genome: str
    track_name: str
    data: List[Any]
