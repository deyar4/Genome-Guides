#!/usr/bin/env python3
"""Find tandem repeats (periods 1-6) and emit hits, summaries, density and enrichment."""

import argparse
import gzip
import re
from collections import Counter, defaultdict
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq


PERIOD_NAMES = {1: "mono", 2: "di", 3: "tri", 4: "tetra", 5: "penta", 6: "hexa"}
MIN_REPEATS = {1: 6, 2: 4, 3: 3, 4: 3, 5: 3, 6: 3}


def write(path, rows, schema):
    path.parent.mkdir(parents=True, exist_ok=True)
    pq.write_table(pa.table({k: pa.array([r.get(k) for r in rows], type=t) for k, t in schema.items()}), path, compression="zstd")


def scan(chrom, sequence):
    candidates = []
    for period, minimum in MIN_REPEATS.items():
        pattern = re.compile(rf"([ACGT]{{{period}}})(?:\1){{{minimum - 1},}}")
        for match in pattern.finditer(sequence):
            motif = match.group(1)
            length = match.end() - match.start()
            candidates.append({
                "chrom": chrom, "start": match.start(), "end": match.end(),
                "motif": motif, "period": period, "repeat_count": length // period,
                "sequence": match.group(0), "length": length,
                "period_name": PERIOD_NAMES[period],
                "complexity": len(set(motif)) / period,
            })
    # Prefer the longest call at a coordinate and remove nested calls.
    selected = []
    for hit in sorted(candidates, key=lambda x: (x["start"], -x["length"], x["period"])):
        if any(hit["start"] >= old["start"] and hit["end"] <= old["end"] for old in selected):
            continue
        selected.append(hit)
    return selected


def extract(fasta, out_dir, window_size=1_000_000):
    hits = []
    chrom_lengths = {}
    open_fn = gzip.open if str(fasta).endswith(".gz") else open
    chrom, block, carry = None, [], ""
    offset, length, block_length = 0, 0, 0
    seen = set()

    def flush():
        nonlocal block, carry, hits, block_length
        if chrom is None or not block:
            return
        sequence = "".join(block)
        combined = carry + sequence
        combined_offset = offset - len(carry)
        for hit in scan(chrom, combined):
            hit["start"] += combined_offset
            hit["end"] += combined_offset
            key = (hit["start"], hit["end"], hit["motif"])
            if key not in seen and hit["start"] >= 0:
                seen.add(key)
                hits.append(hit)
        carry = combined[-1024:]
        block = []
        block_length = 0

    with open_fn(fasta, "rt") as handle:
        for line in handle:
            if line.startswith(">"):
                if chrom is not None:
                    flush()
                    chrom_lengths[chrom] = length
                chrom = line[1:].split()[0]
                block, carry, offset, length, block_length = [], "", 0, 0, 0
                seen = set()
            else:
                sequence = line.strip()
                block.append(sequence)
                length += len(sequence)
                block_length += len(sequence)
                if block_length >= 4_000_000:
                    flush()
                    offset = length
        if chrom is not None:
            flush()
            chrom_lengths[chrom] = length

    summaries = []
    by_period = defaultdict(list)
    for hit in hits:
        by_period[hit["period"]].append(hit)
    for period in sorted(PERIOD_NAMES):
        rows = by_period[period]
        summaries.append({
            "period": period, "name": PERIOD_NAMES[period], "count": len(rows),
            "total_length": sum(r["length"] for r in rows),
            "avg_repeat_count": sum(r["repeat_count"] for r in rows) / len(rows) if rows else 0.0,
        })

    density_counts = Counter((h["chrom"], (h["start"] // window_size) * window_size) for h in hits)
    density = [{
        "chrom": chrom, "start": start, "end": min(chrom_lengths.get(chrom, start + window_size), start + window_size),
        "count": count,
    } for (chrom, start), count in sorted(density_counts.items())]

    genome_length = sum(chrom_lengths.values()) or 1
    genome_ssr_bases = sum(h["length"] for h in hits)
    enrichment = []
    for period in sorted(PERIOD_NAMES):
        rows = by_period[period]
        observed = sum(r["length"] for r in rows)
        expected = genome_length * (0.25 ** period)
        enrichment.append({
            "period": period, "name": PERIOD_NAMES[period], "observed_bases": observed,
            "genome_bases": genome_length, "genome_ssr_fraction": genome_ssr_bases / genome_length,
            "enrichment": observed / expected if expected else 0.0,
        })

    out = Path(out_dir)
    schema = {
        "chrom": pa.string(), "start": pa.uint64(), "end": pa.uint64(), "motif": pa.string(),
        "period": pa.uint8(), "repeat_count": pa.uint32(), "sequence": pa.string(),
        "length": pa.uint64(), "period_name": pa.string(), "complexity": pa.float32(),
    }
    write(out / "ssr_hits.parquet", hits, schema)
    write(out / "ssr_summary.parquet", summaries, {
        "period": pa.uint8(), "name": pa.string(), "count": pa.uint64(),
        "total_length": pa.uint64(), "avg_repeat_count": pa.float64(),
    })
    write(out / "ssr_density.parquet", density, {
        "chrom": pa.string(), "start": pa.uint64(), "end": pa.uint64(), "count": pa.uint64(),
    })
    write(out / "ssr_enrichment.parquet", enrichment, {
        "period": pa.uint8(), "name": pa.string(), "observed_bases": pa.uint64(),
        "genome_bases": pa.uint64(), "genome_ssr_fraction": pa.float64(), "enrichment": pa.float64(),
    })


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-f", "--fasta", required=True)
    parser.add_argument("-o", "--out-dir", required=True)
    parser.add_argument("-w", "--window-size", type=int, default=1_000_000)
    args = parser.parse_args()
    extract(args.fasta, args.out_dir, args.window_size)
