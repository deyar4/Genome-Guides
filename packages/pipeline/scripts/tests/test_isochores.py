import sys
from pathlib import Path

import duckdb
import pyarrow.parquet as pq

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "packages/pipeline/scripts/dna"))
from extract_isochores import extract  # noqa: E402


def test_missing_gc_breaks_runs_and_is_not_classified(tmp_path):
    src, out = tmp_path / "gc.parquet", tmp_path / "iso.parquet"
    duckdb.connect().execute("""COPY (SELECT * FROM (VALUES
        ('c', 0, 1000, 30.0::FLOAT), ('c', 1000, 2000, 32.0::FLOAT), ('c', 2000, 3000, 'NaN'::FLOAT),
        ('c', 3000, 4000, 31.0::FLOAT), ('c', 4000, 5000, NULL), ('c', 5000, 6000, 60.0::FLOAT)
    ) t(contig, start_pos, end_pos, gc_pct)) TO '%s' (FORMAT PARQUET)""" % src)
    extract(str(src), str(out))
    rows = pq.read_table(out).to_pylist()
    assert [(r["start"], r["end"], r["isochore_class"]) for r in rows] == [(0, 2000, "L1"), (3000, 4000, "L1"), (5000, 6000, "H3")]
