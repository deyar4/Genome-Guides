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
    mean of 1.546 bits/base (n_valid-weighted mean of hecate_mprob.mean_entropy of the full T2T run, HANDOFF 2026-10-21, LOGBOOK
    full-run entry). Direction only, not a value."""
    r = rows()
    mean = sum(x[6] for x in r) / len(r)
    assert mean < 1.546


def _reference_windows():
    """Independent pure-Python reference computed from the fixture FASTA, not from any pipeline output."""
    import math
    seq = "".join(l.strip() for l in (ROOT / "tests/fixtures/satslice.fa").read_text().splitlines()[1:]).upper()
    assert len(seq) == 999_999
    for start in range(0, len(seq), 1000):
        w = seq[start:start + 1000]
        c = {b: w.count(b) for b in "ACGT"}
        tot = sum(c.values())
        gc = 100.0 * (c["G"] + c["C"]) / tot
        ent = None if tot < 500 else -sum(n / tot * math.log2(n / tot) for n in c.values() if n)
        skew = None if c["G"] + c["C"] == 0 else 100.0 * (c["G"] - c["C"]) / (c["G"] + c["C"])
        yield start, gc, skew, ent


def test_gc_skew_entropy_match_independent_reference():
    got = {r[0]: r for r in rows()}
    n = 0
    for start, gc, skew, ent in _reference_windows():
        r = got[start]
        assert r[2] == pytest.approx(gc, abs=6e-3), start        # window gc_pct is stored rounded to 2 decimals (seqkit)
        assert (r[3] is None) == (skew is None)
        if skew is not None:
            assert r[3] == pytest.approx(skew, abs=6e-3), start  # window skew is stored rounded to 2 decimals (seqkit output)
        assert (r[4] is None) == (ent is None)
        if ent is not None:
            assert r[4] == pytest.approx(ent, abs=2e-4), start   # rounded to 4 decimals
        n += 1
    assert n == 1000
