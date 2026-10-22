#!/usr/bin/env python3
"""Per-rule wall time and peak memory from Snakemake benchmark files next to the memory each rule declares.

Reads benchmarks/<genome>/<rule>.tsv (columns s, max_rss in MB, ...) and the rule files' `mem_mb=config["resources"][<class>]["mem_mb"]`.
A rule whose max_rss exceeds its declared mem_mb is flagged: declared resources then do not describe the real need (D13/D25).
Writes a TSV (rule, seconds, max_rss_mb, declared_mem_mb, over_declared, benchmark_mtime); never edits any decision.
"""
import argparse
import csv
import datetime as dt
import re
from pathlib import Path

import yaml


def declared_mem(rules_dir: Path, config: dict) -> dict:
    out = {}
    for smk in sorted(rules_dir.glob("*.smk")):
        text = smk.read_text()
        for block in re.split(r"(?m)^rule ", text)[1:]:
            name = block.split(":", 1)[0].strip()
            m = re.search(r'mem_mb=config\["resources"\]\["(\w+)"\]\["mem_mb"\]', block)
            n = re.search(r"mem_mb=(\d+)", block)
            if m:
                out[name] = int(config["resources"][m.group(1)]["mem_mb"])
            elif n:
                out[name] = int(n.group(1))
    return out


def table(bench_dir: Path, declared: dict) -> list[dict]:
    rows = []
    for tsv in sorted(bench_dir.glob("*.tsv")):
        with tsv.open() as f:
            data = list(csv.DictReader(f, delimiter="\t"))
        if not data:
            continue
        rss = float(data[-1]["max_rss"]) if data[-1]["max_rss"] not in ("", "NA", "-") else None
        secs = float(data[-1]["s"])
        decl = declared.get(tsv.stem)
        rows.append({
            "rule": tsv.stem, "seconds": f"{secs:.1f}", "max_rss_mb": "" if rss is None else f"{rss:.0f}",
            "declared_mem_mb": "" if decl is None else decl,
            "over_declared": "" if rss is None or decl is None else str(rss > decl),
            "benchmark_mtime": dt.datetime.fromtimestamp(tsv.stat().st_mtime).strftime("%Y-%m-%d %H:%M"),
        })
    return rows


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bench-dir", type=Path, required=True)
    ap.add_argument("--rules-dir", type=Path, default=Path("packages/pipeline/workflow/rules"))
    ap.add_argument("--config", type=Path, default=Path("packages/pipeline/config.yaml"))
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    rows = table(args.bench_dir, declared_mem(args.rules_dir, yaml.safe_load(args.config.read_text())))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]) if rows else ["rule"], delimiter="\t")
        w.writeheader()
        w.writerows(rows)
    print(f"{len(rows)} rules; over declared: {[r['rule'] for r in rows if r['over_declared'] == 'True']}")
