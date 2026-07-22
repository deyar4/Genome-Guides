#!/usr/bin/env python3
"""
Genome Guides - Pipeline Integration Test & Synthetic Data Generator
"""

import sys
import subprocess
from pathlib import Path
import pyarrow.parquet as pq

def run_test():
    raw_dir = Path("data/raw")
    raw_dir.mkdir(parents=True, exist_ok=True)
    
    tiny_fa = raw_dir / "tiny.fa"
    with open(tiny_fa, "w") as f:
        f.write(">chr1\nATGCATGCATGCATGCATGCATGCATGCATGCATGCATGC\n")

    # Run GC skew
    cmd_gc = [
        "python3", "packages/pipeline/scripts/dna/extract_gc_skew.py",
        "-f", str(tiny_fa),
        "-o", "data/interim/tiny/gc_skew.tsv",
        "-w", "10", "-s", "10"
    ]
    subprocess.run(cmd_gc, check=True)

    # Convert to Parquet
    cmd_pq = [
        "python3", "packages/pipeline/scripts/common/tsv_to_parquet.py",
        "-i", "data/interim/tiny/gc_skew.tsv",
        "-o", "data/tracks/tiny/all/gc_content.parquet",
        "-s", "gc_content"
    ]
    subprocess.run(cmd_pq, check=True)

    # Verify Parquet
    tbl = pq.read_table("data/tracks/tiny/all/gc_content.parquet")
    assert tbl.num_rows > 0, "Test Parquet track is empty!"
    print(f"Created synthetic test FASTA at: {tiny_fa}")
    print(f"Successfully generated Parquet test track at: data/tracks/tiny/all/gc_content.parquet")

if __name__ == "__main__":
    run_test()
