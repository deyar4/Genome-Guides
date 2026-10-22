#!/usr/bin/env python3
"""Independent recount of sampled CpG islands from the FASTA with samtools faidx (spec 01, section 5).

For each sampled island the sequence [start_pos, end_pos) is fetched, upper-cased, and length, C, G, CG, gc_pct and obs/exp
are recomputed in plain Python; thresholds are tested with exact integers. Compared with the parquet: length and cpg_count
exactly, gc_pct and obs_exp_ratio to float32 precision (relative 1e-6). The sample is seeded (config tracks.cpg_islands.recount_seed).
Writes a TSV (metric, value, note); exit status 1 on any mismatch."""
import argparse
import random
import subprocess
import sys
from fractions import Fraction
from pathlib import Path

import pyarrow.parquet as pq

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import config_value, write_tsv


def fetch(fasta, contig, start, end):
    r = subprocess.run(["samtools", "faidx", fasta, f"{contig}:{start + 1}-{end}"], check=True, capture_output=True, text=True)
    return "".join(r.stdout.splitlines()[1:]).upper()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fasta", required=True)
    ap.add_argument("--parquet", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--config", default="packages/pipeline/config.yaml")
    ap.add_argument("--n", type=int, default=200)
    ap.add_argument("--seed", type=int, default=None, help="default: config tracks.cpg_islands.recount_seed")
    args = ap.parse_args()
    seed = args.seed if args.seed is not None else config_value(args.config, "tracks", "cpg_islands", "recount_seed")
    cfg = config_value(args.config, "tracks", "cpg_islands")
    min_gc, min_oe = Fraction(cfg["min_gc"]), Fraction(cfg["min_oe"])

    t = pq.read_table(args.parquet).to_pylist()
    sample = random.Random(seed).sample(t, min(args.n, len(t)))
    bad = {"length": 0, "cpg_count": 0, "gc_pct": 0, "obs_exp_ratio": 0, "threshold": 0, "has_n": 0}
    for r in sample:
        s = fetch(args.fasta, r["contig"], r["start_pos"], r["end_pos"])
        length, c, g = len(s), s.count("C"), s.count("G")
        cg = sum(1 for i in range(length - 1) if s[i] == "C" and s[i + 1] == "G")
        bad["length"] += length != r["length"] or length != r["end_pos"] - r["start_pos"]
        bad["cpg_count"] += cg != r["cpg_count"]
        bad["has_n"] += any(ch not in "ACGT" for ch in set(s))
        if c and g:
            gc, oe = 100.0 * (c + g) / length, cg * length / (c * g)
            bad["gc_pct"] += abs(gc - r["gc_pct"]) > 1e-6 * gc
            bad["obs_exp_ratio"] += abs(oe - r["obs_exp_ratio"]) > 1e-6 * max(oe, 1e-12)
            # exact: 100 (c+g) / length > min_gc and cg * length / (c * g) > min_oe
            ok = Fraction(100 * (c + g), length) > min_gc and Fraction(cg * length, c * g) > min_oe
        else:
            ok = False
        bad["threshold"] += not ok
    rows = [("seed", seed, "config tracks.cpg_islands.recount_seed"), ("islands_in_track", len(t), ""),
            ("sampled", len(sample), "")]
    rows += [(f"mismatch_{k}", v, "") for k, v in bad.items()]
    write_tsv(args.out, rows)
    if any(bad.values()):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
