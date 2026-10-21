import logging

import duckdb
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from fastapi.middleware.cors import CORSMiddleware
from app.api.router import api_router
from app.core.config import settings

def create_app() -> FastAPI:
    app = FastAPI(
        title=settings.PROJECT_NAME,
        description="Vectorized range queries & server-side downsampling over Parquet tracks",
        version=settings.VERSION,
        openapi_url=f"{settings.API_V1_STR}/openapi.json",
    )

    # CORS middleware to allow the frontend to access the API
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],
        allow_credentials=False,  # no cookies or auth are used; "*" with credentials is invalid (audit 05, P10)
        allow_methods=["*"],
        allow_headers=["*"],
    )

    logger = logging.getLogger(__name__)

    @app.exception_handler(duckdb.IOException)
    async def missing_data_handler(request: Request, exc: duckdb.IOException):
        # A track file that does not exist for this build is "not found", not a server error. No path in the message.
        logger.warning("data file unavailable for %s: %s", request.url.path, exc)
        return JSONResponse(status_code=404, content={"detail": "Data not available for this build"})

    @app.exception_handler(duckdb.Error)
    async def query_error_handler(request: Request, exc: duckdb.Error):
        logger.exception("query failed for %s", request.url.path)
        return JSONResponse(status_code=500, content={"detail": "Query failed"})

    app.include_router(api_router, prefix=settings.API_V1_STR)
    app.include_router(api_router, prefix="/api", include_in_schema=False)

    return app

app = create_app()
