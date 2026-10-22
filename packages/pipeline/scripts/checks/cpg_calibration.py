#!/usr/bin/env python3
"""Descriptive calibration of a CpG island track against RepeatMasker classes and, optionally, the UCSC CpG island track.

Inputs: the islands parquet (contig, start_pos, end_pos, length, n_windows, strict, 0-based half-open), a RepeatMasker
annotation (--rmsk, UCSC rmsk.txt or RepeatMasker .out, 0-based start for rmsk.txt, 1-based closed for .out), optionally
the UCSC cpgIslandExt table (0-based start, as in UCSC tables). Only the contigs given by --contigs are used.

Per island: overlap in bp with each repeat label (SINE/Alu, SINE other, LINE, LTR, DNA, Satellite, Simple_repeat,
Low_complexity, other RNA/Retroposon/Unknown, merged within a label so bases are counted once per label). Label of an
island = the label with the largest overlap if that overlap is at least half of the island, else "none_or_mixed".
Nothing here is a test; the output is a table of counts. Writes a TSV (metric, value, note)."""
import argparse
import gzip
import sys
from pathlib import Path

import numpy as np
import pyarrow.parquet as pq

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import write_tsv

LABELS = ("SINE/Alu", "SINE_other", "LINE", "LTR", "DNA", "Satellite", "Simple_repeat", "Low_complexity", "other_repeat")


def label_of(rep_class, rep_family):
    if rep_class == "SINE":
        return "SINE/Alu" if rep_family == "Alu" else "SINE_other"
    return rep_class if rep_class in LABELS else "other_repeat"


def merge(intervals):
    """Sorted, merged, half-open intervals as two int64 arrays."""
    if not len(intervals):
        return np.empty(0, np.int64), np.empty(0, np.int64)
    a = np.array(sorted(intervals), dtype=np.int64)
    ends = np.maximum.accumulate(a[:, 1])
    new = np.concatenate([[True], a[1:, 0] > ends[:-1]])
    group = np.cumsum(new) - 1
    starts = a[new, 0]
    merged_ends = np.zeros(len(starts), np.int64)
    np.maximum.at(merged_ends, group, a[:, 1])
    return starts, merged_ends


def covered_upto(starts, ends, x):
    """Number of covered bases in [0, x) for merged intervals (vectorised over x)."""
    x = np.asarray(x, dtype=np.int64)
    if not len(starts):
        return np.zeros_like(x)
    prefix = np.concatenate([[0], np.cumsum(ends - starts)])
    idx = np.searchsorted(starts, x, side="right")  # intervals with start < x or <= x
    # intervals 0..idx-1 start at or before x: all but the last are complete if x >= their end
    full = prefix[np.maximum(idx - 1, 0)]
    last = np.where(idx > 0, np.minimum(x, ends[np.maximum(idx - 1, 0)]) - starts[np.maximum(idx - 1, 0)], 0)
    return full + np.maximum(last, 0)


def overlap_bp(starts, ends, s, e):
    return covered_upto(starts, ends, e) - covered_upto(starts, ends, s)


def read_rmsk(path, contigs):
    by = {c: {} for c in contigs}
    opener = gzip.open if str(path).endswith(".gz") else open
    with opener(path, "rt") as f:
        for line in f:
            if line.startswith(("SW", "score", "   score")) or not line.strip():
                continue
            p = line.split("\t") if "\t" in line else line.split()
            if "\t" in line:  # UCSC rmsk.txt: genoName 5, genoStart 6, genoEnd 7, repClass 11, repFamily 12
                contig, s, e, rc, rf = p[5], int(p[6]), int(p[7]), p[11], p[12]
            else:  # RepeatMasker .out: query 4, begin 5, end 6 (1-based closed), class/family 10
                if not p[0].isdigit():
                    continue
                contig, s, e = p[4], int(p[5]) - 1, int(p[6])
                rc, _, rf = p[10].partition("/")
            if contig in by:
                by[contig].setdefault(label_of(rc, rf), []).append((s, e))
    return {c: {lab: merge(v) for lab, v in d.items()} for c, d in by.items()}


def read_islands(path, contigs):
    t = pq.read_table(path).to_pylist()
    return [r for r in t if r["contig"] in contigs]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--islands", required=True)
    ap.add_argument("--rmsk", required=True)
    ap.add_argument("--contigs", nargs="+", required=True)
    ap.add_argument("--ucsc-cpg", default=None, help="UCSC cpgIslandExt table (bin chrom start end ...)")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    contigs = set(args.contigs)

    isl = read_islands(args.islands, contigs)
    rm = read_rmsk(args.rmsk, contigs)
    rows = [("contigs", ",".join(args.contigs), ""), ("islands", len(isl), "merged runs of passing 200 bp windows"),
            ("islands_strict", sum(r["strict"] for r in isl), ""), ("covered_bp", sum(r["length"] for r in isl), "")]
    lens = np.array([r["length"] for r in isl])
    rows += [("median_length_bp", int(np.median(lens)), ""), ("max_length_bp", int(lens.max()), ""),
             ("islands_single_window", sum(r["n_windows"] == 1 for r in isl), "exactly one 200 bp window"),
             ("islands_length_le_400", int((lens <= 400).sum()), "")]

    counts = {lab: 0 for lab in LABELS + ("none_or_mixed",)}
    bps = dict.fromkeys(counts, 0)
    strict_counts = dict.fromkeys(counts, 0)
    for contig in sorted(contigs):
        sub = [r for r in isl if r["contig"] == contig]
        if not sub:
            continue
        s = np.array([r["start_pos"] for r in sub], np.int64)
        e = np.array([r["end_pos"] for r in sub], np.int64)
        ov = np.stack([overlap_bp(*rm[contig][lab], s, e) if lab in rm[contig] else np.zeros(len(sub), np.int64)
                       for lab in LABELS])
        best, top = ov.argmax(0), ov.max(0)
        for i, r in enumerate(sub):
            lab = LABELS[best[i]] if 2 * top[i] >= (e[i] - s[i]) else "none_or_mixed"
            counts[lab] += 1
            strict_counts[lab] += r["strict"]
            bps[lab] += int(e[i] - s[i])
    for lab in counts:
        rows.append((f"islands_label_{lab}", counts[lab], "label = repeat class covering at least half of the island"))
        rows.append((f"island_bp_label_{lab}", bps[lab], ""))
        rows.append((f"strict_islands_label_{lab}", strict_counts[lab], "strict islands (length >= 500, GC >= 55 %, obs/exp >= 0.65)"))

    if args.ucsc_cpg:
        u = {c: [] for c in contigs}
        for line in open(args.ucsc_cpg):
            p = line.rstrip("\n").split("\t")
            if p[1] in contigs:
                u[p[1]].append((int(p[2]), int(p[3])))
        n_u = sum(len(v) for v in u.values())
        bp_u = sum(e - s for v in u.values() for s, e in v)
        inter = ours_hit = ucsc_hit = 0
        for contig in contigs:
            us, ue = merge(u[contig])
            sub = [r for r in isl if r["contig"] == contig]
            if not sub:
                continue
            ms, me = merge([(r["start_pos"], r["end_pos"]) for r in sub])
            if len(us):
                inter += int(overlap_bp(us, ue, ms, me).sum())
                ours_hit += int((overlap_bp(us, ue, np.array([r["start_pos"] for r in sub]), np.array([r["end_pos"] for r in sub])) > 0).sum())
            ucsc_hit += int((overlap_bp(ms, me, np.array([a for a, _ in u[contig]], np.int64), np.array([b for _, b in u[contig]], np.int64)) > 0).sum()) if u[contig] else 0
        bp_o = sum(r["length"] for r in isl)
        rows += [("ucsc_islands", n_u, "UCSC cpgIslandExt records on these contigs"), ("ucsc_covered_bp", bp_u, ""),
                 ("intersection_bp", inter, "bp covered by both (islands within one set are disjoint)"),
                 ("jaccard_bp", f"{inter / (bp_o + bp_u - inter):.4f}", "intersection / union in bp"),
                 ("our_islands_overlapping_ucsc", ours_hit, ""), ("ucsc_islands_overlapping_ours", ucsc_hit, "")]
    write_tsv(args.out, rows)


if __name__ == "__main__":
    main()
