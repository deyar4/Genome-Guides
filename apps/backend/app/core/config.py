import re
from typing import List, Optional

from pydantic import field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

_MEMORY_LIMIT = re.compile(r"^\d+(\.\d+)?\s?(KB|MB|GB|TB|KiB|MiB|GiB|TiB)$")


class Settings(BaseSettings):
    """Environment-driven settings (also read from a .env file); see .env.example."""
    model_config = SettingsConfigDict(env_file=".env", extra="ignore")
    PROJECT_NAME: str = "Genome Guides Backend"
    API_V1_STR: str = "/api/v1"
    VERSION: str = "0.1.0"
    # Directory holding tracks/<build>/all/*.parquet (and raw/<build>.fa + .fai for the sequence endpoints).
    # A relative value is looked up from the working directory and its two parents (repo checkout layout).
    DATA_DIR: str = "data"
    # Comma-separated list of allowed browser origins, or * (default, development). Set it in production.
    CORS_ORIGINS: str = "*"
    # DuckDB limits for small machines; unset = DuckDB defaults (all cores, 80 % of RAM). Example: 4, "4GB".
    DUCKDB_THREADS: Optional[int] = None
    DUCKDB_MEMORY_LIMIT: Optional[str] = None

    @field_validator("DUCKDB_THREADS", "DUCKDB_MEMORY_LIMIT", mode="before")
    @classmethod
    def _empty_is_unset(cls, v):
        # docker compose and .env.example pass empty strings for "not set"
        return None if isinstance(v, str) and not v.strip() else v

    @field_validator("DUCKDB_THREADS")
    @classmethod
    def _threads_positive(cls, v):
        if v is not None and v < 1:
            raise ValueError("DUCKDB_THREADS must be >= 1")
        return v

    @field_validator("DUCKDB_MEMORY_LIMIT")
    @classmethod
    def _memory_limit_format(cls, v):
        if v is not None and not _MEMORY_LIMIT.match(v.strip()):
            raise ValueError("DUCKDB_MEMORY_LIMIT must look like 4GB or 512MB")
        return v.strip() if v else v

    @property
    def cors_origin_list(self) -> List[str]:
        return [o.strip() for o in self.CORS_ORIGINS.split(",") if o.strip()] or ["*"]


settings = Settings()
