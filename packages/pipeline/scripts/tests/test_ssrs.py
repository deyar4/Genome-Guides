"""SSR extraction must not depend on chunk size or on soft-masking (audit: case-sensitive regex, memory-derived chunks)."""
import random
import sys
from pathlib import Path

import pyarrow.parquet as pq
import pytest

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "packages/pipeline/scripts/dna"))
from extract_ssrs import extract  # noqa: E402


def make_fasta(path, seed):
    rng = random.Random(seed)
    lines = []
    for c in range(3):
        parts = []
        for _ in range(60):
            parts.append("".join(rng.choice("ACGT") for _ in range(rng.randint(5, 80))))
            motif = "".join(rng.choice("ACGT") for _ in range(rng.randint(1, 6)))
            parts.append(motif * rng.randint(1, 30))
            if rng.random() < 0.1:
                parts.append("N" * rng.randint(1, 20))
        seq = "".join(parts)
        lines.append(f">c{c}")
        lines += [seq[i:i + 60] for i in range(0, len(seq), 60)]
    path.write_text("\n".join(lines) + "\n")


def hits(tmp_path, fasta, chunk, tag):
    out = tmp_path / tag
    extract(str(fasta), str(out), window_size=1000, chunk_bases=chunk)
    t = pq.read_table(out / "ssr_hits.parquet").to_pylist()
    return sorted((r["chrom"], r["start"], r["end"], r["motif"], r["period"]) for r in t)


@pytest.mark.parametrize("seed", [1, 2, 3])
@pytest.mark.parametrize("chunk", [1, 7, 60, 61, 333, 1000])
def test_result_does_not_depend_on_chunk_size(tmp_path, seed, chunk):
    fa = tmp_path / "g.fa"
    make_fasta(fa, seed)
    reference = hits(tmp_path, fa, 10**9, "ref")
    assert len(reference) > 50
    assert hits(tmp_path, fa, chunk, f"c{chunk}") == reference


def test_soft_masked_bases_are_scanned(tmp_path):
    fa, fa_lower = tmp_path / "g.fa", tmp_path / "g_lower.fa"
    make_fasta(fa, 5)
    fa_lower.write_text("\n".join(l if l.startswith(">") else l.lower() for l in fa.read_text().splitlines()) + "\n")
    up = hits(tmp_path, fa, 500, "up")
    assert len(up) > 50
    assert hits(tmp_path, fa_lower, 500, "lo") == up
    rng = random.Random(99)  # case flips mid-tract; the scan is case-insensitive, N stays N
    mixed = tmp_path / "mixed.fa"
    mixed.write_text("\n".join(l if l.startswith(">") else "".join(c.lower() if rng.random() < 0.5 else c for c in l)
                               for l in fa.read_text().splitlines()) + "\n")
    assert hits(tmp_path, mixed, 500, "mx") == up


def test_scan_matches_all_pairs_definition_and_is_fast():
    import time
    from extract_ssrs import scan
    rng = random.Random(7)
    seq = "".join(rng.choice("ACGT") for _ in range(3000)) + "ACACACACAC" * 5000   # many candidates in one long array
    seq += "".join(rng.choice("AC") for _ in range(200_000))                          # low-complexity: many short candidates
    t = time.time()
    got = scan("c", seq)
    assert time.time() - t < 30
    # reference: the original definition on a small prefix
    small = seq[:3000] + seq[3000:3200] + seq[-3000:]
    cands = scan("c", small)
    for h in cands:
        assert not any(h is not o and h["start"] >= o["start"] and h["end"] <= o["end"] for o in cands)
    assert len(got) > 1000
