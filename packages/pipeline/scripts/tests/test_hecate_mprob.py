"""Tests for process_hecate_mprob.py: stream alignment (audit S1, S2) and truncation."""
import lzma
import random
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
import polars as pl
import pytest

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "packages/pipeline/scripts/common"))
from process_hecate_mprob import process_mprob, stream_pad_indices  # noqa: E402

HECATE = ROOT / "tools/hecate/hecate"
CONTIGS = [("c1", 1001), ("c2", 500), ("c3", 777), ("c4", 4)]  # odd, even, odd, even


def write_fai(path):
    path.write_text("".join(f"{n}\t{l}\t0\t60\t61\n" for n, l in CONTIGS))


def test_pad_indices():
    # c1: entries 0..1001 (pad at 1001); c2: 1002..1501; c3: 1502..2279 (pad at 2279)
    assert stream_pad_indices(CONTIGS).tolist() == [1001, 2279]


def test_synthetic_stream_drops_pads_and_keeps_alignment(tmp_path):
    fai = tmp_path / "x.fai"
    write_fai(fai)
    values, stream = {}, []
    for i, (name, n) in enumerate(CONTIGS):
        v = 0.1 * (i + 1)  # distinct constant per contig
        stream += [v] * n
        if n % 2:
            stream.append(0.999)  # pad marker, must vanish
    (tmp_path / "p.txt").write_text("\n".join(f"{x:.5f}" for x in stream) + "\n")
    out = tmp_path / "o.parquet"
    process_mprob(fai, tmp_path / "p.txt", out, window_size=100, chunk_windows=3)
    df = pl.read_parquet(out)
    for i, (name, n) in enumerate(CONTIGS):
        d = df.filter(pl.col("contig") == name).sort("start_pos")
        assert d.height == -(-n // 100)
        assert d["start_pos"].to_list() == list(range(0, n, 100))
        assert d["end_pos"][-1] == n
        assert np.allclose(d["mean_prob"].to_numpy(), 0.1 * (i + 1), atol=1e-6)
        assert d["max_prob"].max() < 0.99


def test_nonacgt_mask_gives_nan_and_counts(tmp_path):
    fai = tmp_path / "x.fai"
    write_fai(fai)
    stream = []
    for i, (name, n) in enumerate(CONTIGS):
        stream += [0.25] * n + ([0.5] if n % 2 else [])
    (tmp_path / "p.txt").write_text("\n".join(f"{x:.5f}" for x in stream) + "\n")
    # c1: window [0,100) half masked, window [100,200) fully masked; c4 (4 bp) fully masked
    (tmp_path / "m.bed").write_text("c1\t50\t200\nc4\t0\t4\n")
    out = tmp_path / "o.parquet"
    stats = process_mprob(fai, tmp_path / "p.txt", out, window_size=100, chunk_windows=3, nonacgt_bed=tmp_path / "m.bed")
    assert stats == {"bases": 2282, "masked": 154, "prob_zero": 0}
    df = pl.read_parquet(out)
    c1 = df.filter(pl.col("contig") == "c1").sort("start_pos")
    assert c1["n_valid"].to_list()[:3] == [50, 0, 100]
    assert c1["mean_prob"][1] is None and c1["mean_entropy"].null_count() >= 1
    assert np.isclose(c1["mean_prob"][0], 0.25) and np.isclose(c1["mean_entropy"][0], 2.0)
    assert c1["n_valid"][-1] == 1001 - 1000  # last window holds one base
    c4 = df.filter(pl.col("contig") == "c4")
    assert c4["n_valid"].to_list() == [0]
    c2 = df.filter(pl.col("contig") == "c2")
    assert c2["n_valid"].to_list() == [100] * 5


def test_zero_probabilities_are_counted_and_clipped(tmp_path):
    fai = tmp_path / "x.fai"
    write_fai(fai)
    stream = []
    for _, n in CONTIGS:
        stream += [0.25] * n + ([0.5] if n % 2 else [])
    stream[10] = 0.0
    stream[20] = 0.0
    (tmp_path / "p.txt").write_text("\n".join(f"{x:.5f}" for x in stream) + "\n")
    out = tmp_path / "o.parquet"
    stats = process_mprob(fai, tmp_path / "p.txt", out, window_size=100)
    assert stats["prob_zero"] == 2
    first = pl.read_parquet(out).sort(["contig", "start_pos"])["mean_entropy"][0]
    assert np.isclose(first, (98 * 2.0 + 2 * 20.0) / 100, atol=1e-4)


def test_short_stream_raises(tmp_path):
    fai = tmp_path / "x.fai"
    write_fai(fai)
    (tmp_path / "p.txt").write_text("0.5\n" * 100)
    with pytest.raises(ValueError, match="ended early"):
        process_mprob(fai, tmp_path / "p.txt", tmp_path / "o.parquet", window_size=100)


@pytest.mark.skipif(not HECATE.exists(), reason="pinned hecate binary not built (tools/hecate/hecate)")
def test_real_hecate_line_count_and_rows(tmp_path):
    rng = random.Random(1)
    fa = tmp_path / "t.fa"
    with open(fa, "w") as f:
        for name, n in CONTIGS:
            seq = "".join(rng.choice("ACGT") for _ in range(n))
            if name == "c3":
                seq = seq[:100] + "N" * 50 + seq[150:]
            f.write(f">{name}\n{seq}\n")
    write_fai(tmp_path / "t.fa.fai")
    mp = tmp_path / "m.xz"
    subprocess.run([HECATE, "--force", "-e", "-M", "markov", "--mprob", mp, fa, "-o", tmp_path / "t.he"],
                   check=True, capture_output=True)
    lines = sum(1 for _ in lzma.open(mp, "rt"))
    assert lines == sum(n + n % 2 for _, n in CONTIGS)
    out = tmp_path / "o.parquet"
    process_mprob(tmp_path / "t.fa.fai", mp, out, window_size=100)
    df = pl.read_parquet(out)
    assert df.height == sum(-(-n // 100) for _, n in CONTIGS)
    assert df.group_by("contig").agg(pl.col("end_pos").max()).sort("contig")["end_pos"].to_list() == [n for _, n in CONTIGS]


def test_expected_model_class_matches_upstream_dispatch():
    from process_hecate_mprob import expected_model_class
    assert expected_model_class(16_569) == "vmarkov"       # chrM
    assert expected_model_class(199_999) == "vmarkov"
    assert expected_model_class(200_000) == "bmarkov"
    assert expected_model_class(786_431) == "bmarkov"
    assert expected_model_class(786_432) == "markov"
    assert expected_model_class(1 << 20) == "markov"        # T2T chr1 is capped at one block budget
