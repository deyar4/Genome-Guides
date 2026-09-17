#!/usr/bin/env python3
"""Benchmark selected extraction and DuckDB operations."""

import time
import subprocess
import tempfile
import sys
from pathlib import Path
import duckdb

def benchmark_pipeline_extraction():
    print("--- 1. Pipeline Extraction Benchmark ---")
    
    fasta_content = ">chr_bench\n" + "ATGC" * 250000 + "\n"
    with tempfile.NamedTemporaryFile(mode="w", suffix=".fa", delete=False) as f:
        f.write(fasta_content)
        fasta_path = f.name

    try:
        py_script = Path("packages/pipeline/scripts/dna/extract_gc_skew.py")
        if py_script.exists():
            t0 = time.perf_counter()
            subprocess.run([
                "python3", str(py_script),
                "-f", fasta_path,
                "-o", "data/interim/bench_py.tsv",
                "-w", "1000", "-s", "1000"
            ], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            t_py = time.perf_counter() - t0
            print(f"Python gc_skew script time: {t_py:.4f} seconds")
        else:
            print("Python gc_skew script not found. Skipping.")
            t_py = None

        t0 = time.perf_counter()
        cmd = f"seqkit sliding -g -W 1000 -s 1000 {fasta_path} | seqkit fx2tab -n -g -G -C A,C,G,T,N -B A,C,G,T,N -H > data/interim/bench_seqkit.tsv"
        subprocess.run(cmd, shell=True, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        t_seq = time.perf_counter() - t0
        print(f"Seqkit CLI sliding/fx2tab time: {t_seq:.4f} seconds")

        if t_py and t_seq:
            speedup = t_py / t_seq
            print(f"-> Speedup factor: {speedup:.2f}x")

    finally:
        Path(fasta_path).unlink()
        for p in ["data/interim/bench_py.tsv", "data/interim/bench_seqkit.tsv"]:
            if Path(p).exists():
                Path(p).unlink()

def benchmark_duckdb_queries():
    print("\n--- 2. Database Query Speed Benchmark ---")
    parquet_path = Path("data/tracks/tiny/all/gc_content.parquet")
    if not parquet_path.exists():
        print(f"Synthetic parquet track {parquet_path} not found. Run snakemake first. Skipping database benchmark.")
        return

    con = duckdb.connect(database=":memory:")
    
    t0 = time.perf_counter()
    for _ in range(500):
        con.execute(f"""
            SELECT 
                (start_pos / 10) * 10 AS bin_start,
                AVG(gc_pct) AS avg_val
            FROM read_parquet('{parquet_path}')
            WHERE contig = 'chr1' AND start_pos >= 0 AND end_pos <= 5000
            GROUP BY bin_start
            ORDER BY bin_start
        """).fetchall()
    t_duck = (time.perf_counter() - t0) / 500
    print(f"Average DuckDB query time (500 runs): {t_duck * 1000:.2f} ms")

if __name__ == "__main__":
    benchmark_pipeline_extraction()
    benchmark_duckdb_queries()
