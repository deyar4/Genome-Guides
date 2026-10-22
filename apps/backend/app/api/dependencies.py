from app.core.config import settings
from app.services.duckdb_track_service import DuckDBTrackService

# We initialize a single instance of the service to be used across the application.
track_service = DuckDBTrackService(
    data_dir=settings.DATA_DIR, threads=settings.DUCKDB_THREADS, memory_limit=settings.DUCKDB_MEMORY_LIMIT
)

def get_track_service() -> DuckDBTrackService:
    return track_service
