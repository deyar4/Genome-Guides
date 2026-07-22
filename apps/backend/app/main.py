from fastapi import FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from app.services.duckdb_track_service import DuckDBTrackService

app = FastAPI(
    title="Genome Guides Backend Engine",
    description="Vectorized range queries & server-side downsampling over Parquet tracks",
    version="0.1.0"
)

# CORS middleware to allow the frontend to access the API
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

track_service = DuckDBTrackService()

@app.get("/api/health")
def health_check():
    return {"status": "ok", "service": "Genome Guides Backend Engine"}

@app.get("/api/tracks/{genome}/{contig}/{track_name}")
def get_track_data(
    genome: str,
    contig: str,
    track_name: str,
    start: int = Query(..., description="Start position (0-based, inclusive)"),
    end: int = Query(..., description="End position (0-based, exclusive)"),
    max_points: int = Query(500, description="Max points for downsampling")
):
    try:
        data = track_service.query_track_range(
            genome=genome,
            contig=contig,
            track_name=track_name,
            start_pos=start,
            end_pos=end,
            max_points=max_points
        )
        return {
            "genome": genome,
            "contig": contig,
            "track_name": track_name,
            "start": start,
            "end": end,
            "max_points": max_points,
            "data": data
        }
    except FileNotFoundError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to query track: {str(e)}")
