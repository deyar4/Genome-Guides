import sys
from pathlib import Path

import duckdb

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "packages/pipeline/scripts/common"))
from check_coverage import check_track  # noqa: E402


def make(tmp_path, rows):
    p = tmp_path / "t.parquet"
    duckdb.connect().execute(
        "COPY (SELECT * FROM (VALUES %s) t(contig, start_pos, end_pos)) TO '%s' (FORMAT PARQUET)"
        % (",".join(f"('{c}',{s},{e})" for c, s, e in rows), p))
    return p


def test_complete_track_passes(tmp_path):
    p = make(tmp_path, [("a", 0, 10), ("a", 10, 20), ("a", 20, 25), ("b", 0, 7)])
    assert check_track(duckdb.connect(), p, {"a": 25, "b": 7}, 10) == []


def test_truncated_track_fails(tmp_path):
    p = make(tmp_path, [("a", 0, 10), ("a", 10, 20)])  # a stops at 20 of 25, b missing
    errs = check_track(duckdb.connect(), p, {"a": 25, "b": 7}, 10)
    assert any("a:" in e for e in errs) and any("b: no rows" in e for e in errs)


def test_gap_and_overlap_fail(tmp_path):
    gap = make(tmp_path, [("a", 0, 10), ("a", 20, 25)])
    assert check_track(duckdb.connect(), gap, {"a": 25}, 10)
    p2 = tmp_path / "o"; p2.mkdir()
    ov = make(p2, [("a", 0, 10), ("a", 5, 15), ("a", 15, 25)])
    assert check_track(duckdb.connect(), ov, {"a": 25}, 10)
