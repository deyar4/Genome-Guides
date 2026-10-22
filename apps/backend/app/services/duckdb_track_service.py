"""
Genome Guides - DuckDB Parquet Track Query Service
Provides high-performance vectorized range queries & server-side downsampling over Parquet tracks.
"""

from pathlib import Path
from typing import Dict, Any, List, Optional, Tuple
import math
import threading
import duckdb

from app.core.track_meta import COUNT_COLUMNS, GC_PCT_SQL, GC_SKEW_SQL, WEIGHT_COLUMN
from app.core.validation import InvalidTrackRequest, check_identifier

INTERVAL_ROW_LIMIT = 5000


def _finite(row):
    """Non-finite floats are not valid JSON; they are missing values and leave as None."""
    return tuple(None if isinstance(v, float) and not math.isfinite(v) else v for v in row)


class DuckDBTrackService:
    def __init__(self, data_dir: str = "data", threads: Optional[int] = None, memory_limit: Optional[str] = None):
        self.conn = duckdb.connect(database=":memory:")
        # Values are validated by Settings (SET takes no bound parameters).
        if threads is not None:
            self.conn.execute(f"SET threads = {int(threads)}")
        if memory_limit is not None:
            self.conn.execute(f"SET memory_limit = '{memory_limit}'")
        self._lock = threading.Lock()
        self._window_cache: Dict[str, int] = {}
        raw_path = Path(data_dir)
        if not raw_path.is_absolute():
            cwd = Path.cwd()
            if (cwd / raw_path).exists():
                self.data_dir = (cwd / raw_path).resolve()
            elif (cwd.parent / raw_path).exists():
                self.data_dir = (cwd.parent / raw_path).resolve()
            elif (cwd.parent.parent / raw_path).exists():
                self.data_dir = (cwd.parent.parent / raw_path).resolve()
            else:
                self.data_dir = (cwd / raw_path).resolve()
        else:
            self.data_dir = raw_path

    def get_track_file(self, genome: str, contig: str, track_name: str) -> Path:
        """Resolve path to Parquet track file. Names are validated, so they cannot escape data_dir."""
        check_identifier(genome, "genome")
        check_identifier(contig, "contig")
        check_identifier(track_name, "track_name")
        path = self.data_dir / "tracks" / genome / "all" / f"{track_name}.parquet"
        if not path.exists():
            # Try contig-partitioned path
            path = self.data_dir / "tracks" / genome / contig / f"{track_name}.parquet"
        if not path.exists():
            # No filesystem path in the message (audit 05, P7)
            raise FileNotFoundError(f"Track '{track_name}' not found for genome '{genome}'")
        return path

    def query_track_range(
        self,
        genome: str,
        contig: str,
        track_name: str,
        start_pos: int,
        end_pos: int,
        max_points: int = 500
    ) -> List[Dict[str, Any]]:
        """Query a region with server-side binning (window tracks) or raw rows (interval tracks)."""
        rows, _ = self.query_track_range_ex(genome, contig, track_name, start_pos, end_pos, max_points)
        return rows

    def query_track_range_ex(
        self,
        genome: str,
        contig: str,
        track_name: str,
        start_pos: int,
        end_pos: int,
        max_points: int = 500,
    ) -> Tuple[List[Dict[str, Any]], bool]:
        """Like query_track_range, also returns whether an interval query hit the row limit (truncated)."""
        if start_pos < 0 or end_pos <= start_pos:
            raise InvalidTrackRequest("Require 0 <= start < end (0-based, half-open)")
        if max_points < 1:
            raise InvalidTrackRequest("max_points must be >= 1")
        with self._lock:
            return self._query_track_range(genome, contig, track_name, start_pos, end_pos, max_points)

    def window_size(self, parquet_path: Path) -> int:
        with self._lock:
            return self._window_size(parquet_path)

    def _window_size(self, parquet_path: Path) -> int:
        """Largest window width in the file (the base resolution, D31). Bins are multiples of it so they stay exact."""
        key = str(parquet_path)
        if key not in self._window_cache:
            row = self.conn.execute(
                "SELECT MAX(CAST(end_pos AS BIGINT) - CAST(start_pos AS BIGINT)) FROM read_parquet(?)", [key]
            ).fetchone()
            self._window_cache[key] = int(row[0] or 1)
        return self._window_cache[key]

    @staticmethod
    def _clean(col: str, col_type: str) -> str:
        """NaN is not valid JSON and is a missing value here: turn it into NULL (counted separately)."""
        if any(t in col_type.lower() for t in ("float", "double", "real")):
            return f"CASE WHEN isnan({col}) THEN NULL ELSE {col} END"
        return col

    def _query_track_range(
        self,
        genome: str,
        contig: str,
        track_name: str,
        start_pos: int,
        end_pos: int,
        max_points: int,
    ) -> Tuple[List[Dict[str, Any]], bool]:
        parquet_path = self.get_track_file(genome, contig, track_name)
        src = "read_parquet(?)"
        path_param = str(parquet_path)

        # Detect the columns and types in the parquet file
        columns_info = self.conn.execute(f"DESCRIBE SELECT * FROM {src}", [path_param]).fetchall()
        columns = [row[0] for row in columns_info]
        types = {row[0]: row[1] for row in columns_info}

        has_coords = "start_pos" in columns and "end_pos" in columns
        contig_col = "contig" if "contig" in columns else ("chrom" if "chrom" in columns else None)
        truncated = False
        is_interval = False

        if not has_coords:
            # Global summary table (e.g. amino acid composition, biotype census)
            query, params = f"SELECT * FROM {src}", [path_param]
        else:
            numeric_cols = []
            for col, col_type in types.items():
                if col in ["contig", "chrom", "start_pos", "end_pos", "strand", "base", "gap_type", "gene_id", "gene_name", "biotype"]:
                    continue
                if any(t in col_type.lower() for t in ["double", "float", "int", "decimal", "real"]):
                    numeric_cols.append(col)

            interval_tracks = {
                "assembly_gaps",
                "homopolymer_genomic_tracts",
                "gene_models",
                "repeatmasker_families",
            }
            where = "start_pos < ? AND end_pos > ?"
            where_params: List[Any] = [end_pos, start_pos]
            if contig_col:
                where = f"{contig_col} = ? AND " + where
                where_params = [contig] + where_params

            if numeric_cols and track_name not in interval_tracks:
                # Window track: bins are multiples of the base window so every bin is an exact aggregate (D31)
                window = self._window_size(parquet_path)
                raw_bin = -(-(end_pos - start_pos) // max_points)  # ceil
                bin_size = max(window, -(-raw_bin // window) * window)

                select_cols = [
                    f"(CAST(start_pos AS BIGINT) // {bin_size}) * {bin_size} AS bin_start",
                    "MAX(end_pos) AS bin_end",
                    "COUNT(*) AS n_windows",
                ]
                weight = WEIGHT_COLUMN.get(track_name)
                for col in numeric_cols:
                    clean = self._clean(col, types[col])
                    if track_name == "gc_content" and col == "gc_pct":
                        # Counts give the exact value. Fixtures without base counts (all zero) fall back to the window-length-weighted
                        # mean of gc_pct; N-only windows (gc_pct NaN -> NULL) stay NULL.
                        fallback = (f"SUM({clean} * CAST(end_pos - start_pos AS DOUBLE)) / "
                                    f"NULLIF(SUM(CASE WHEN {clean} IS NOT NULL THEN CAST(end_pos - start_pos AS DOUBLE) END), 0)")
                        select_cols += [f"COALESCE({GC_PCT_SQL}, {fallback}) AS avg_value", f"MIN({clean}) AS min_value", f"MAX({clean}) AS max_value"]
                    elif track_name == "gc_content" and col == "gc_skew":
                        select_cols += [f"{GC_SKEW_SQL} AS avg_skew", f"COUNT(*) - COUNT({clean}) AS n_null_skew"]
                    elif track_name == "gc_content":
                        continue  # per-base counts/percentages are only used through the exact formulas above
                    elif col in COUNT_COLUMNS or col.endswith("_count"):
                        select_cols.append(f"SUM({col}) AS sum_{col}")
                        if col not in (weight,):
                            select_cols.append(f"AVG({col}) AS avg_{col}")
                    else:
                        if weight and weight in columns:
                            avg = (f"SUM({clean} * CAST({weight} AS DOUBLE)) / "
                                   f"NULLIF(SUM(CASE WHEN {clean} IS NOT NULL THEN CAST({weight} AS DOUBLE) END), 0)")
                        else:
                            avg = f"AVG({clean})"
                        select_cols += [
                            f"{avg} AS avg_{col}",
                            f"MIN({clean}) AS min_{col}",
                            f"MAX({clean}) AS max_{col}",
                            f"COUNT(*) - COUNT({clean}) AS n_null_{col}",
                        ]

                query = f"""
                    SELECT {", ".join(select_cols)}
                    FROM {src}
                    WHERE {where}
                    GROUP BY bin_start
                    ORDER BY bin_start
                """
                params = [path_param] + where_params
            else:
                # Interval/discrete feature track (gaps, homopolymers, gene models, repeat families)
                is_interval = True
                query = f"""
                    SELECT *
                    FROM {src}
                    WHERE {where}
                    ORDER BY start_pos
                    LIMIT {INTERVAL_ROW_LIMIT + 1}
                """
                params = [path_param] + where_params

        rel = self.conn.execute(query, params)
        desc = [d[0] for d in rel.description]
        results = [dict(zip(desc, _finite(row))) for row in rel.fetchall()]
        if is_interval and len(results) > INTERVAL_ROW_LIMIT:
            truncated = True
            results = results[:INTERVAL_ROW_LIMIT]
        return results, truncated

    def execute_query(self, query: str, params: Optional[list] = None) -> List[Dict[str, Any]]:
        """Execute a generic SQL query against DuckDB and return dictionaries."""
        with self._lock:
            rel = self.conn.execute(query, params or [])
            desc = [d[0] for d in rel.description]
            results = []
            for row in rel.fetchall():
                results.append(dict(zip(desc, _finite(row))))
            return results
