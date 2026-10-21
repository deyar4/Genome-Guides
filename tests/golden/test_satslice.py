"""Golden test on the 1 Mb satellite slice (tests/fixtures/satslice.fa, chr1:126,300,000-127,299,999 of T2T-CHM13v2.0).

Needs `make pipeline-satslice` (about 5 s). Skips when the tracks are absent so `make test` stays fast; `make golden` runs it.
"""
import sys
from pathlib import Path

import duckdb
import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).parent))
from make_golden import QUERY  # noqa: E402

GOLDEN = Path(__file__).parent / "satslice_windows.csv"

pytestmark = pytest.mark.skipif(
    not (ROOT / "data/tracks/satslice/all/hecate_mprob.parquet").exists(), reason="run `make pipeline-satslice` first")


def rows():
    return duckdb.sql(QUERY).fetchall()


def test_windows_tile_the_slice():
    r = rows()
    assert len(r) == 1000
    assert r[0][0] == 0 and r[-1][1] == 999_999
    assert all(a[1] == b[0] for a, b in zip(r, r[1:]))          # contiguous, half-open
    assert r[-1][1] - r[-1][0] == 999                              # short tail window


def test_missing_data_counts():
    r = rows()
    assert sum(x[7] for x in r) == 999_999                         # hecate n_valid: every base valid (no N in T2T)
    assert all(x[5] >= 500 for x in r)                             # entropy never NULL here (D33 threshold n_acgt >= 500)
    assert r[-1][5] == 999 and r[-1][7] == 999


def test_matches_golden():
    got = rows()
    want = duckdb.sql(f"SELECT * FROM read_csv('{GOLDEN}', header=true)").fetchall()
    assert len(got) == len(want)
    for g, w in zip(got, want):
        assert g[0] == w[0] and g[1] == w[1] and g[5] == w[5] and g[7] == w[7]
        for i in (2, 3, 4, 6):
            if g[i] is None or w[i] is None:
                assert g[i] == w[i]
            else:
                assert g[i] == pytest.approx(w[i], abs=2e-4), (g[0], i)


def test_satellite_is_more_predictable_than_unique_sequence():
    """Expected biology (ASSUMPTION, adaptive model): the tandem array has lower cross-entropy than the genome-wide
    mean of 1.55 bits/base. Not a threshold on a specific value, only a sanity direction."""
    r = rows()
    mean = sum(x[6] for x in r) / len(r)
    assert mean < 1.55
