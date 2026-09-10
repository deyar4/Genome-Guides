from app.services.duckdb_track_service import DuckDBTrackService

# We initialize a single instance of the service to be used across the application.
track_service = DuckDBTrackService()

def get_track_service() -> DuckDBTrackService:
    return track_service
