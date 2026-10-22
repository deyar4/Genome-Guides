"""extract_cpg_islands.py against an independent naive implementation of docs/analyses/01_cpg_islands.md."""
import random
import subprocess
import sys
from fractions import Fraction
from pathlib import Path

import duckdb
import pytest

ROOT = Path(__file__).resolve().parents[4]
SCRIPT = ROOT / "packages/pipeline/scripts/dna/extract_cpg_islands.py"


def write_fasta(path, contigs, width=60):
    with open(path, "w") as f:
        for name, seq in contigs.items():
            f.write(f">{name}\n")
            for i in range(0, len(seq), width):
                f.write(seq[i:i + width] + "\n")
    offset, lines = 0, []
    with open(path, "rb") as f:
        data = f.read()
    pos = 0
    for name, seq in contigs.items():
        pos = data.index(b">" + name.encode(), pos) + len(name) + 2
        lines.append(f"{name}\t{len(seq)}\t{pos}\t{width}\t{width + 1}\n")
        pos += len(seq) + -(-len(seq) // width)
    Path(str(path) + ".fai").write_text("".join(lines))


def naive(contigs, window=200, min_gc=Fraction(50), min_oe=Fraction(3, 5), min_length=200,
          strict=(500, Fraction(55), Fraction(13, 20))):
    """Direct string-based implementation: Fractions, CG counted by scanning the substring."""
    def stats(s):
        s = s.upper()
        c, g = s.count("C"), s.count("G")
        cg = sum(1 for i in range(len(s) - 1) if s[i] == "C" and s[i + 1] == "G")
        return c, g, cg, len(s)

    out = []
    for name, seq in contigs.items():
        starts = list(range(0, len(seq) - window + 1, window))
        cand = []
        for s in starts:
            w = seq[s:s + window]
            c, g, cg, L = stats(w)
            ok = ("N" not in w.upper()) and Fraction(100 * (c + g), L) > min_gc and c * g > 0 \
                and Fraction(cg * L, c * g) > min_oe
            cand.append(ok)
        i = 0
        while i < len(starts):
            if not cand[i]:
                i += 1
                continue
            j = i
            while j + 1 < len(starts) and cand[j + 1]:
                j += 1
            region = seq[starts[i]: starts[j] + window]
            c, g, cg, L = stats(region)
            if Fraction(100 * (c + g), L) > min_gc and c * g > 0 and Fraction(cg * L, c * g) > min_oe and L >= min_length:
                st = int(L >= strict[0] and Fraction(100 * (c + g), L) >= strict[1] and Fraction(cg * L, c * g) >= strict[2])
                out.append((name, starts[i], starts[j] + window, cg, j - i + 1, st))
            i = j + 1
    return out


def run(tmp_path, contigs, window=200, extra=()):
    fa = tmp_path / "x.fa"
    write_fasta(fa, contigs)
    out = tmp_path / "o.parquet"
    subprocess.run([sys.executable, str(SCRIPT), "--fasta", str(fa), "--fai", f"{fa}.fai", "--out-parquet", str(out),
                    "--out-summary", str(tmp_path / "s.tsv"), "--out-meta", str(tmp_path / "m.json"),
                    "--window", str(window), "--block-bases", str(window * 7), *extra], check=True)
    rows = duckdb.sql(f"SELECT contig, start_pos, end_pos, cpg_count, n_windows, strict FROM '{out}' "
                      "ORDER BY contig, start_pos").fetchall()
    summary = dict(line.split("\t") for line in (tmp_path / "s.tsv").read_text().splitlines())
    return rows, summary, out


def rich(rng, n):  # CpG-rich: many C, G, and CG pairs
    return "".join(rng.choice(["CG", "CG", "CCGG", "GC", "CGCG", "A", "T"]) for _ in range(n // 2))[:n]


def poor(rng, n):
    return "".join(rng.choice("AT") for _ in range(n))


def test_random_genomes_match_naive_including_straddles_and_soft_mask(tmp_path):
    rng = random.Random(7)
    total = 0
    for trial in range(8):
        contigs = {}
        for k in range(3):
            parts = []
            for _ in range(rng.randint(3, 12)):
                parts.append((rich if rng.random() < 0.5 else poor)(rng, rng.choice([60, 130, 200, 410, 700])))
            seq = "".join(parts) + poor(rng, rng.randint(0, 150))
            if trial % 2:
                seq = "".join(ch.lower() if rng.random() < 0.3 else ch for ch in seq)
            contigs[f"c{k}"] = seq
        rows, _, _ = run(tmp_path, contigs, window=60, extra=("--min-length", "60", "--strict-min-length", "150"))
        assert rows == naive(contigs, window=60, min_length=60, strict=(150, Fraction(55), Fraction(13, 20)))
        total += len(rows)
    assert total > 20  # the comparison is not vacuous


def test_planted_island_exact_coordinates(tmp_path):
    seq = "A" * 400 + ("CG" * 100) * 2 + "T" * 400  # 400 bp of CG repeats at [400, 800)
    rows, summary, _ = run(tmp_path, {"c": seq})
    assert [(r[1], r[2], r[3], r[4]) for r in rows] == [(400, 800, 200, 2)]
    assert summary["islands"] == "1" and summary["windows_with_n"] == "0"


def test_poly_a_poly_c_and_n_windows_give_nothing(tmp_path):
    rows, summary, _ = run(tmp_path, {"a": "A" * 1000, "c": "C" * 1000, "n": ("CG" * 99 + "N" * 2) * 3})
    assert rows == [] and summary["windows_with_n"] == "3"


def test_straddling_cg_counted_once_and_decides_the_run(tmp_path):
    # window 30. w1: C=18 G=2 CG=2 (last base C); w2: C=2 G=18 CG=2 (first base G). Each window passes alone.
    # Union without the straddle: CG=4, C*G=400, L=60 -> 4*60 = 0.6*400 exactly -> fails; the straddle CG makes 5.
    w1 = "A" * 10 + "CG" * 2 + "C" * 16
    w2 = "G" * 16 + "CG" * 2 + "A" * 10
    rows, _, _ = run(tmp_path, {"c": w1 + w2}, window=30, extra=("--min-length", "30"))
    assert [(r[1], r[2], r[3], r[4]) for r in rows] == [(0, 60, 5, 2)]
    # without the straddle (second window starts with A instead of G) the union fails and the run is rejected
    w2b = "A" + w2[:-1]  # shifted one base: still C=2 G=18 CG=2, no straddle
    rows, summary, _ = run(tmp_path, {"c": w1 + w2b}, window=30, extra=("--min-length", "30"))
    assert rows == [] and summary["runs_rejected"] == "1" and summary["runs_rejected_bp"] == "60"


def window_with(n_cg, width=30):
    """C=10, G=10 and exactly n_cg CG dinucleotides, in `width` bp."""
    seq = "CG" * n_cg + "A" + "G" * (10 - n_cg) + "A" + "C" * (10 - n_cg)
    return seq + "A" * (width - len(seq))


def test_boundaries_are_strict(tmp_path):
    # oe = n_cg * 30 / 100: n_cg = 2 gives exactly 0.6 -> fails, n_cg = 3 gives 0.9 -> passes
    assert run(tmp_path, {"c": window_with(2)}, window=30, extra=("--min-length", "30"))[0] == []
    assert len(run(tmp_path, {"c": window_with(3)}, window=30, extra=("--min-length", "30"))[0]) == 1
    # gc exactly 50 % (C=3 G=2 in 10 bp) fails
    assert run(tmp_path, {"c": "CGCGC" + "AAAAA"}, window=10, extra=("--min-length", "10"))[0] == []


def test_short_contig_and_cg_at_last_base_and_partial_window(tmp_path):
    contigs = {"short": "CGCGCG", "tail": "A" * 195 + "CGCGC" + "CG"}
    rows, summary, _ = run(tmp_path, contigs)
    assert rows == naive(contigs)
    assert summary["contigs_shorter_than_window"] == "1" and summary["partial_windows"] == "2"
