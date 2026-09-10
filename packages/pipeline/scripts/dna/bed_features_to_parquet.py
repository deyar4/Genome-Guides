#!/usr/bin/env python3
"""Convert an authoritative BED feature download to a coordinate Parquet track."""

import argparse
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq


def convert(input_bed, output, source_url, feature=None):
    rows = []
    with open(input_bed) as handle:
        for line in handle:
            if not line.strip() or line.startswith("#"):
                continue
            fields = line.rstrip().split("\t")
            if len(fields) < 3:
                continue
            if feature and (len(fields) < 4 or fields[3] != feature):
                continue
            try:
                start, end = int(fields[1]), int(fields[2])
            except ValueError:
                continue
            rows.append({
                "chrom": fields[0], "start": start, "end": end,
                "length": max(0, end - start),
                "name": fields[3] if len(fields) > 3 else Path(input_bed).stem,
                "source_url": source_url,
            })
    schema = {"chrom": pa.string(), "start": pa.uint64(), "end": pa.uint64(),
              "length": pa.uint64(), "name": pa.string(), "source_url": pa.string()}
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    table = pa.table({k: pa.array([r[k] for r in rows], type=t) for k, t in schema.items()})
    table = table.replace_schema_metadata({b"source_url": source_url.encode(), b"coordinate_system": b"0-based-half-open"})
    pq.write_table(table, output, compression="zstd")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-i", "--input", required=True)
    parser.add_argument("-o", "--output", required=True)
    parser.add_argument("--source-url", required=True)
    parser.add_argument("--feature")
    args = parser.parse_args()
    convert(args.input, args.output, args.source_url, args.feature)
