"""
Genome Guides - DuckDB Parquet Track Query Service
Provides high-performance vectorized range queries & server-side downsampling over Parquet tracks.
"""

from pathlib import Path
from typing import Dict, Any, List, Optional
import threading
import duckdb

class DuckDBTrackService:
    def __init__(self, data_dir: str = "data"):
        self.conn = duckdb.connect(database=":memory:")
        self._lock = threading.Lock()
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
        """Resolve path to Parquet track file."""
        path = self.data_dir / "tracks" / genome / "all" / f"{track_name}.parquet"
        if not path.exists():
            # Try contig-partitioned path
            path = self.data_dir / "tracks" / genome / contig / f"{track_name}.parquet"
        if not path.exists():
            raise FileNotFoundError(f"Track file not found for genome '{genome}', track '{track_name}' at {path}")
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
        """Query region range with automatic server-side binning/downsampling or raw queries."""
        with self._lock:
            return self._query_track_range(
                genome, contig, track_name, start_pos, end_pos, max_points
            )

    def _query_track_range(
        self,
        genome: str,
        contig: str,
        track_name: str,
        start_pos: int,
        end_pos: int,
        max_points: int,
    ) -> List[Dict[str, Any]]:
        parquet_path = self.get_track_file(genome, contig, track_name)

        # Detect the columns and types in the parquet file
        columns_rel = self.conn.execute(f"DESCRIBE SELECT * FROM read_parquet('{parquet_path}') LIMIT 0")
        columns_info = columns_rel.fetchall()
        columns = [row[0] for row in columns_info]
        types = {row[0]: row[1] for row in columns_info}

        has_coords = "start_pos" in columns and "end_pos" in columns
        has_contig = "contig" in columns or "chrom" in columns
        contig_col = "contig" if "contig" in columns else "chrom"

        if not has_coords:
            # Global summary table (e.g. amino acid composition, biotype census)
            query = f"SELECT * FROM read_parquet('{parquet_path}')"
        else:
            # Coordinate-based track
            # Identify numerical columns to aggregate (floats, doubles, integers)
            numeric_cols = []
            for col, col_type in types.items():
                if col in ["contig", "chrom", "start_pos", "end_pos", "strand", "base", "gap_type", "gene_id", "gene_name", "biotype"]:
                    continue
                col_type_lower = col_type.lower()
                if any(t in col_type_lower for t in ["double", "float", "int", "decimal", "real"]):
                    numeric_cols.append(col)

            interval_tracks = {
                "assembly_gaps",
                "homopolymer_genomic_tracts",
                "gene_models",
                "repeatmasker_families",
            }

            if numeric_cols and track_name not in interval_tracks:
                # Continuous numerical track (like gc_content, shannon_entropy, repeat/gene density)
                region_length = max(1, end_pos - start_pos)
                bin_size = max(1, region_length // max_points)

                select_cols = [f"(start_pos / {bin_size}) * {bin_size} AS bin_start"]
                for col in numeric_cols:
                    if col == "gc_pct":
                        select_cols.append("AVG(gc_pct) AS avg_value")
                        select_cols.append("MIN(gc_pct) AS min_value")
                        select_cols.append("MAX(gc_pct) AS max_value")
                    elif col == "gc_skew":
                        select_cols.append("AVG(gc_skew) AS avg_skew")
                    else:
                        select_cols.append(f"AVG({col}) AS avg_{col}")
                        select_cols.append(f"MIN({col}) AS min_{col}")
                        select_cols.append(f"MAX({col}) AS max_{col}")

                where_clause = f"start_pos < {end_pos} AND end_pos > {start_pos}"
                if has_contig:
                    where_clause = f"{contig_col} = '{contig}' AND " + where_clause

                query = f"""
                    SELECT
                        {", ".join(select_cols)}
                    FROM read_parquet('{parquet_path}')
                    WHERE {where_clause}
                    GROUP BY bin_start
                    ORDER BY bin_start
                """
            else:
                # Interval/discrete feature track (gaps, homopolymers, gene models, repeat families)
                where_clause = f"start_pos < {end_pos} AND end_pos > {start_pos}"
                if has_contig:
                    where_clause = f"{contig_col} = '{contig}' AND " + where_clause

                query = f"""
                    SELECT *
                    FROM read_parquet('{parquet_path}')
                    WHERE {where_clause}
                    ORDER BY start_pos
                    LIMIT 5000
                """

        rel = self.conn.execute(query)
        desc = [d[0] for d in rel.description]
        results = []
        for row in rel.fetchall():
            results.append(dict(zip(desc, row)))

        return results

    def execute_query(self, query: str) -> List[Dict[str, Any]]:
        """Execute a generic SQL query against DuckDB and return dictionaries."""
        with self._lock:
            rel = self.conn.execute(query)
            desc = [d[0] for d in rel.description]
            results = []
            for row in rel.fetchall():
                results.append(dict(zip(desc, row)))
            return results
