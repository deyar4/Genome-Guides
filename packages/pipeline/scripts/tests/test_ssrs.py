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


def coverage_rows(tmp_path, fasta_text, **kw):
    fa = tmp_path / "t.fa"
    fa.write_text(fasta_text)
    extract(fa, tmp_path / "out", **kw)
    return {r["period"]: r for r in pq.read_table(tmp_path / "out/ssr_coverage.parquet").to_pylist()}


def test_coverage_counts_acgt_n_other_and_lowercase(tmp_path):
    # 20 bases: 'acgtn' lowercase folded, 1 N, 1 R (other), tract (AC)x4 = 8 bases, mono-A x6 = 6 bases
    seq = "GGTCACACACACTTGGTCNRAAAAAAGGTC"
    rows = coverage_rows(tmp_path, f">c\n{seq.lower()}\n")
    r2, r1 = rows[2], rows[1]
    assert r2["genome_bases"] == len(seq) and r2["n_bases"] == 1 and r2["other_bases"] == 1
    assert r2["acgt_bases"] == len(seq) - 2
    assert r2["observed_bases"] == 8 and r2["count"] == 1
    assert r2["coverage_fraction"] == pytest.approx(8 / (len(seq) - 2))
    assert r1["observed_bases"] == 6
    assert r2["density_per_mb"] == pytest.approx(1 / (len(seq) - 2) * 1e6)
    assert "enrichment" not in r2


def test_union_counts_overlapping_tracts_once(tmp_path):
    # find a partly overlapping pair of different periods in random sequence, then check sum - union equals the overlap
    from extract_ssrs import scan
    rng = random.Random(5)
    seq = "".join(rng.choice("AC") for _ in range(4000))
    hits = scan("c", seq)
    covered = set()
    sums = 0
    for h in hits:
        sums += h["length"]
        covered.update(range(h["start"], h["end"]))
    rows = coverage_rows(tmp_path, f">c\n{seq}\n", chunk_bases=500)
    r = rows[1]
    assert r["sum_ssr_bases"] == sums and r["union_ssr_bases"] == len(covered)
    assert r["overlap_excess_bases"] == sums - len(covered)
