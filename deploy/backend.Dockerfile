# Backend image: FastAPI + DuckDB over the precomputed tracks. Build from the repository root:
#   docker build -f deploy/backend.Dockerfile -t genome-guides-backend .
# Tracks are NOT in the image: mount them read-only at /data (tracks/<build>/all/*.parquet, optional raw/<build>.fa + .fai).
FROM python:3.11-slim

RUN pip install --no-cache-dir uv==0.12.15 \
    && useradd --create-home --uid 10001 app
WORKDIR /srv/backend
COPY apps/backend/pyproject.toml apps/backend/uv.lock apps/backend/README.md ./
RUN uv sync --frozen --no-dev --no-install-project
COPY apps/backend/app ./app

ENV PATH="/srv/backend/.venv/bin:$PATH" \
    DATA_DIR=/data \
    WEB_CONCURRENCY=1
USER app
EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD python -c "import urllib.request,sys; sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:8000/api/v1/system/ready').status == 200 else 1)"
# --workers comes from WEB_CONCURRENCY (each worker holds its own DuckDB connection and memory limit)
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
