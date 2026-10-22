#!/usr/bin/env python3
"""Grid sensitivity of the CpG islands (spec 01 section 4(i)): rerun the extractor with the 200 bp grid shifted by
--offset and compare with the baseline parquet (offset 0): island counts, covered bp, and the base-pair Jaccard index
|A and B| / |A or B| of the two island sets (islands within one run are disjoint). Writes a TSV (metric, value, note)."""
import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

import pyarrow.parquet as pq

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import write_tsv

EXTRACTOR = Path(__file__).resolve().parents[1] / "dna" / "extract_cpg_islands.py"


def by_contig(path):
    d = {}
    for r in pq.read_table(path, columns=["contig", "start_pos", "end_pos"]).to_pylist():
        d.setdefault(r["contig"], []).append((r["start_pos"], r["end_pos"]))
    for v in d.values():
        v.sort()
    return d


def intersection_bp(a, b):
    i = j = total = 0
    while i < len(a) and j < len(b):
        lo, hi = max(a[i][0], b[j][0]), min(a[i][1], b[j][1])
        total += max(0, hi - lo)
        if a[i][1] < b[j][1]:
            i += 1
        else:
            j += 1
    return total


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fasta", required=True)
    ap.add_argument("--fai", required=True)
    ap.add_argument("--baseline-parquet", required=True)
    ap.add_argument("--offset", type=int, default=100)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        subprocess.run([sys.executable, str(EXTRACTOR), "--fasta", args.fasta, "--fai", args.fai,
                        "--out-parquet", str(d / "s.parquet"), "--out-summary", str(d / "s.tsv"),
                        "--out-meta", str(d / "s.json"), "--grid-offset", str(args.offset)], check=True)
        a, b = by_contig(args.baseline_parquet), by_contig(d / "s.parquet")
    bp = lambda x: sum(e - s for v in x.values() for s, e in v)
    n = lambda x: sum(len(v) for v in x.values())
    inter = sum(intersection_bp(a.get(c, []), b.get(c, [])) for c in set(a) | set(b))
    union = bp(a) + bp(b) - inter
    write_tsv(args.out, [
        ("offset_bp", args.offset, "start of the shifted window grid"),
        ("islands_baseline", n(a), ""), ("islands_shifted", n(b), ""),
        ("covered_bp_baseline", bp(a), ""), ("covered_bp_shifted", bp(b), ""),
        ("intersection_bp", inter, ""), ("union_bp", union, ""),
        ("jaccard_bp", f"{inter / union:.6f}" if union else "NA", "intersection / union in base pairs"),
    ])


if __name__ == "__main__":
    main()
