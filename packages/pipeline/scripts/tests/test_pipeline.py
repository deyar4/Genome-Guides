#!/usr/bin/env python3
"""Run a small end-to-end pipeline smoke test."""

import sys
import subprocess
from pathlib import Path
import polars as pl

def run_test():
    raw_dir = Path("data/raw")
    raw_dir.mkdir(parents=True, exist_ok=True)
    
    tiny_fa = raw_dir / "tiny.fa"
    with open(tiny_fa, "w") as f:
        f.write(">chr1\nATGCATGCATGCATGCATGCATGCATGCATGCATGCATGC\n")

    Path("data/interim/tiny").mkdir(parents=True, exist_ok=True)  # was missing after `make clean`

    # Run GC skew via seqkit sliding & fx2tab
    cmd_gc = f"seqkit sliding -g -W 10 -s 10 {tiny_fa} | seqkit fx2tab -n -g -G -H > data/interim/tiny/gc_skew.tsv"
    subprocess.run(cmd_gc, shell=True, check=True)

    # Convert to Parquet
    cmd_pq = [
        "python3", "packages/pipeline/scripts/common/tsv_to_parquet.py",
        "-i", "data/interim/tiny/gc_skew.tsv",
        "-o", "data/tracks/tiny/all/gc_content.parquet",
        "-s", "gc_content"
    ]
    subprocess.run(cmd_pq, check=True)

    # Verify Parquet
    df = pl.read_parquet("data/tracks/tiny/all/gc_content.parquet")
    assert df.height > 0, "Test Parquet track is empty!"
    print(f"Created synthetic test FASTA at: {tiny_fa}")
    print(f"Successfully generated Parquet test track at: data/tracks/tiny/all/gc_content.parquet")

if __name__ == "__main__":
    run_test()
