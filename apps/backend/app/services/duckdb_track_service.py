"""
Genome Guides - DuckDB Parquet Track Query Service
Provides high-performance vectorized range queries & server-side downsampling over Parquet tracks.
"""

from pathlib import Path
from typing import Dict, Any, List, Optional
import duckdb

class DuckDBTrackService:
    def __init__(self, data_dir: str = "data"):
        self.conn = duckdb.connect(database=":memory:")
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
        """Query region range with automatic server-side binning/downsampling."""
        parquet_path = self.get_track_file(genome, contig, track_name)
        
        region_length = max(1, end_pos - start_pos)
        bin_size = max(1, region_length // max_points)

        query = f"""
            SELECT 
                (start_pos / {bin_size}) * {bin_size} AS bin_start,
                AVG(gc_pct) AS avg_value,
                MIN(gc_pct) AS min_value,
                MAX(gc_pct) AS max_value
            FROM read_parquet('{parquet_path}')
            WHERE contig = '{contig}' AND start_pos >= {start_pos} AND end_pos <= {end_pos}
            GROUP BY bin_start
            ORDER BY bin_start
        """
        
        rel = self.conn.execute(query)
        columns = [desc[0] for desc in rel.description]
        results = []
        for row in rel.fetchall():
            results.append(dict(zip(columns, row)))
            
        return results
