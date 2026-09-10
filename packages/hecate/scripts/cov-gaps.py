#!/usr/bin/env python3
"""Parse cargo llvm-cov --text output and summarize uncovered code."""

import re
import sys
import json


def summary_from_json(path):
    data = json.load(open(path))
    totals = data["data"][0]["totals"]
    print("=== OVERALL ===")
    for key in ["lines", "functions"]:
        t = totals[key]
        print(f"  {key}: {t['covered']}/{t['count']} ({t['percent']:.1f}%)")
    print()
    print("=== PER-FILE LINE COVERAGE ===")
    files = sorted(
        data["data"][0]["files"],
        key=lambda f: f["summary"]["lines"]["percent"],
    )
    for f in files:
        name = f["filename"]
        # Strip the project prefix.
        for prefix in ["/home/workspace/hecate/src/"]:
            if name.startswith(prefix):
                name = name[len(prefix):]
                break
        s = f["summary"]["lines"]
        print(f"  {s['percent']:5.1f}%  {s['covered']:4d}/{s['count']:4d}  {name}")


def gaps_from_text(path):
    file = None
    uncovered = {}
    with open(path) as f:
        for line in f:
            m = re.match(r"^(/home/.*\.rs):", line)
            if m:
                file = m.group(1).replace(
                    "/home/workspace/hecate/src/", ""
                )
                continue
            if file and "|      0|" in line:
                parts = line.split("|", 2)
                if len(parts) >= 3:
                    code = parts[2].strip()
                    if code and code != "}" and not code.startswith("//"):
                        uncovered.setdefault(file, []).append(code)

    for f in sorted(uncovered, key=lambda k: -len(uncovered[k])):
        lines = uncovered[f]
        print(f"{len(lines):4d} uncovered lines  {f}")


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "gaps"
    if cmd == "summary":
        summary_from_json(sys.argv[2])
    elif cmd == "gaps":
        gaps_from_text(sys.argv[2])
    else:
        print(f"usage: {sys.argv[0]} [summary <json> | gaps <text>]", file=sys.stderr)
        sys.exit(1)
