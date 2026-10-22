#!/usr/bin/env python3
"""Equivalence check of docs/analyses/01_cpg_islands.md section 5(b): the new extractor in --legacy-out mode must
reproduce the windows of the previous bedtools/seqkit/awk track exactly.

The previous rule is reproduced verbatim from git 1341ce6^ (packages/pipeline/workflow/rules/dna.smk, rule
compute_cpg_islands): bedtools makewindows/nuc, seqkit locate, bedtools map, awk (gc >= 50, ratio >= 0.60, count >= 4).
Compared key: (contig, start, end, CG count). Needs bedtools and seqkit on PATH (conda env genome-guides).
sort gets -S 1G -T <workdir> (memory cap, temp files on the workdir disk, not tmpfs); order of equal results is unchanged.
t2t needs about 6-8 GB of disk in --workdir and several minutes (see the logbook entry for the measured time).
Writes a TSV (metric, value, note). Exit status 1 if any window is missing, extra or has a different CG count."""
import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import write_tsv

EXTRACTOR = Path(__file__).resolve().parents[1] / "dna" / "extract_cpg_islands.py"

OLD_PIPELINE = r"""
set -euo pipefail
bedtools makewindows -g {fai} -w 200 -s 200 | sort -S 1G -T {d} -k1,1 -k2,2n > {d}/windows.bed
bedtools nuc -fi {fasta} -bed {d}/windows.bed | grep -v "^#" > {d}/nuc.bed
seqkit locate -i -P -p "CG" --bed {fasta} 2>/dev/null | sort -S 1G -T {d} -k1,1 -k2,2n > {d}/locs.bed
if [ ! -s {d}/locs.bed ]; then
    awk 'BEGIN {{OFS="\t"}} {{print $0, 0}}' {d}/nuc.bed
else
    bedtools map -a {d}/nuc.bed -b {d}/locs.bed -c 4 -o count -null 0
fi > {d}/mapped.bed
awk 'BEGIN {{OFS="\t"}} {{
    gc_pct = $5 * 100.0;
    expected = ($7 * $8) / $12;
    ratio = (expected > 0) ? ($13 / expected) : 0.0;
    is_island = (gc_pct >= 50.0 && ratio >= 0.60 && $13 >= 4.0) ? 1 : 0;
    if (is_island == 1) print $1, $2, $3, gc_pct, ratio, $13, is_island
}}' {d}/mapped.bed > {d}/old.tsv
"""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fasta", required=True)
    ap.add_argument("--fai", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--workdir", default=None, help="kept if given (large for t2t), else a temp dir")
    args = ap.parse_args()

    tmp = None if args.workdir else tempfile.TemporaryDirectory()
    d = Path(args.workdir or tmp.name)
    d.mkdir(parents=True, exist_ok=True)
    subprocess.run(["bash", "-c", OLD_PIPELINE.format(fasta=args.fasta, fai=args.fai, d=d)], check=True)
    subprocess.run([sys.executable, str(EXTRACTOR), "--fasta", args.fasta, "--fai", args.fai,
                    "--out-parquet", str(d / "new.parquet"), "--out-summary", str(d / "new_summary.tsv"),
                    "--out-meta", str(d / "new_meta.json"), "--legacy-out", str(d / "legacy.tsv")], check=True)

    def keys(path, cols):
        out = set()
        for line in open(path):
            f = line.rstrip("\n").split("\t")
            out.add((f[0], int(f[1]), int(f[2]), int(f[cols])))
        return out

    old = keys(d / "old.tsv", 5)   # contig start end gc ratio count is_island
    new = keys(d / "legacy.tsv", 3)  # contig start end count
    old_w = {k[:3] for k in old}
    new_w = {k[:3] for k in new}
    missing, extra = old_w - new_w, new_w - old_w
    count_diff = len(old - new) - len(missing)
    write_tsv(args.out, [
        ("old_windows", len(old), "windows passing the old rule (bedtools/seqkit/awk, git 1341ce6^)"),
        ("new_legacy_windows", len(new), "windows passing the old rule in extract_cpg_islands.py --legacy-out"),
        ("missing_in_new", len(missing), "in old, not in new"),
        ("extra_in_new", len(extra), "in new, not in old"),
        ("same_window_different_cg_count", count_diff, "window in both with a different CG count"),
    ])
    if missing or extra or count_diff:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
