#!/usr/bin/env python3
"""Merge GC windows into classical L1/L2/H1/H2/H3 isochore segments."""

import argparse
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq


def classify(gc):
    if gc < 37:
        return "L1"
    if gc < 41:
        return "L2"
    if gc < 46:
        return "H1"
    if gc < 53:
        return "H2"
    return "H3"


def extract(gc_path, output):
    table = pq.read_table(gc_path)
    rows = table.to_pylist()
    rows.sort(key=lambda r: (r.get("contig", ""), r.get("start_pos", 0)))
    merged = []
    for row in rows:
        gc = float(row.get("gc_pct") or 0)
        cls = classify(gc)
        current = merged[-1] if merged else None
        if current and current["chrom"] == row["contig"] and current["isochore_class"] == cls and current["end"] == row["start_pos"]:
            old_len = current["end"] - current["start"]
            new_len = max(1, row["end_pos"] - row["start_pos"])
            current["gc_content"] = (current["gc_content"] * old_len + gc * new_len) / (old_len + new_len)
            current["end"] = row["end_pos"]
        else:
            merged.append({
                "chrom": row["contig"], "start": row["start_pos"], "end": row["end_pos"],
                "gc_content": gc, "isochore_class": cls,
            })
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    schema = {"chrom": pa.string(), "start": pa.uint64(), "end": pa.uint64(),
              "gc_content": pa.float32(), "isochore_class": pa.string()}
    pq.write_table(pa.table({k: pa.array([r[k] for r in merged], type=t) for k, t in schema.items()}),
                   output, compression="zstd")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-i", "--input", required=True)
    parser.add_argument("-o", "--output", required=True)
    args = parser.parse_args()
    extract(args.input, args.output)
