#!/usr/bin/env python3
"""Merge consecutive 1 kb windows of the same GC class into runs (L1/L2/H1/H2/H3 by fixed GC thresholds).

These are GC-class runs on the 1 kb grid, NOT isochores in the literature sense (those are defined on much larger
windows; audit 03 S10, UNCITED). Windows without a GC value (gc_pct NULL/NaN, e.g. all-N) are missing: they are counted,
reported and break a run; they are never classified.
"""

import argparse
import math
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args


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
    parquet = pq.ParquetFile(gc_path)
    merged = []
    current = None
    missing = 0
    for batch in parquet.iter_batches(batch_size=65_536):
        for row in batch.to_pylist():
            raw = row.get("gc_pct")
            if raw is None or math.isnan(raw):
                missing += 1
                if current:
                    merged.append(current)
                    current = None
                continue
            gc = float(raw)
            cls = classify(gc)
            if current and current["chrom"] == row["contig"] and current["isochore_class"] == cls and current["end"] == row["start_pos"]:
                old_len = current["end"] - current["start"]
                new_len = max(1, row["end_pos"] - row["start_pos"])
                current["gc_content"] = (current["gc_content"] * old_len + gc * new_len) / (old_len + new_len)
                current["end"] = row["end_pos"]
            else:
                if current:
                    merged.append(current)
                current = {
                    "chrom": row["contig"], "start": row["start_pos"], "end": row["end_pos"],
                    "gc_content": gc, "isochore_class": cls,
                }
    if current:
        merged.append(current)
    print(f"isochore_class: {len(merged)} runs, {missing} windows without gc_pct skipped (missing, not classified)")
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    schema = {"chrom": pa.string(), "start": pa.uint64(), "end": pa.uint64(),
              "gc_content": pa.float32(), "isochore_class": pa.string()}
    pq.write_table(pa.table({k: pa.array([r[k] for r in merged], type=t) for k, t in schema.items()}),
                   output, compression="zstd")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-i", "--input", required=True)
    parser.add_argument("-o", "--output", required=True)
    add_resource_args(parser)
    args = parser.parse_args()
    extract(args.input, args.output)
