#!/usr/bin/env python3
"""Find tandem repeats (periods 1-6) and emit hits, summaries and descriptive coverage (D41).

ssr_coverage.parquet is descriptive only: observed SSR bases and counts per period relative to the ACGT bases of the
assembly, with N and other symbols counted separately. It carries no null model and supports no over- or under-representation
claim (the former 0.25**period "enrichment" was removed, D41). Soft-masked bases are folded to uppercase and scanned.
union_ssr_bases counts each base once even when hits of different periods partly overlap (the nested-hit filter only removes
fully contained calls), so overlap_excess_bases = sum of hit lengths - union is reported, not hidden."""

import argparse
import gzip
import re
from collections import Counter, defaultdict
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args, effective_memory_mb


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
    # Sorted by start, a hit is nested in an earlier selected hit iff that hit's end >= this end, so the running maximum end
    # of the selected hits decides (O(n log n); the earlier all-pairs check was quadratic and stalled once lowercase
    # sequence was scanned too).
    selected = []
    max_end = -1
    for hit in sorted(candidates, key=lambda x: (x["start"], -x["length"], x["period"])):
        if hit["end"] <= max_end:
            continue
        selected.append(hit)
        max_end = hit["end"]
    return selected


MAX_PERIOD = max(PERIOD_NAMES)
# Longest stretch at the end of a chunk in which a not-yet-visible repeat could still start (smallest tract that matches)
TAIL_MARGIN = max(period * minimum for period, minimum in MIN_REPEATS.items())


def safe_cut(hits, size):
    """Position up to which hits are final. A hit may still grow if it touches the chunk end, a new tract may start in
    the last TAIL_MARGIN bases, and no emitted hit may straddle the cut (the next chunk restarts the scan there)."""
    cut = size - TAIL_MARGIN
    for hit in hits:
        if hit["end"] > size - MAX_PERIOD:
            cut = min(cut, hit["start"])
    changed = True
    while changed:
        changed = False
        for hit in hits:
            if hit["start"] < cut < hit["end"]:
                cut = hit["start"]
                changed = True
    return max(cut, 0)


def extract(fasta, out_dir, chunk_bases=4_000_000):
    if chunk_bases < 1:
        raise ValueError("chunk size must be positive")

    chrom_lengths = {}
    period_counts = Counter()
    period_bases = Counter()
    total_ssr_bases = 0
    union_ssr_bases = 0
    cover_end = 0
    base_counts = Counter()
    open_fn = gzip.open if str(fasta).endswith(".gz") else open
    chrom, block = None, []
    carry, carry_start, length, block_length = "", 0, 0, 0
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    hit_schema = {
        "chrom": pa.string(), "start": pa.uint64(), "end": pa.uint64(), "motif": pa.string(),
        "period": pa.uint8(), "repeat_count": pa.uint32(), "sequence": pa.string(),
        "length": pa.uint64(), "period_name": pa.string(), "complexity": pa.float32(),
    }
    hit_writer = pq.ParquetWriter(out / "ssr_hits.parquet", pa.schema([
        pa.field(name, dtype) for name, dtype in hit_schema.items()
    ]), compression="zstd")

    def flush(final):
        """Scan carry + new block; emit only hits that cannot change with more sequence (chunk-size independent)."""
        nonlocal block, carry, carry_start, block_length, total_ssr_bases, union_ssr_bases, cover_end
        if chrom is None:
            return
        combined = carry + "".join(block)
        block, block_length = [], 0
        hits = scan(chrom, combined)
        cut = len(combined) if final else safe_cut(hits, len(combined))
        batch = []
        for hit in hits:
            if hit["end"] > cut:
                continue
            hit["start"] += carry_start
            hit["end"] += carry_start
            batch.append(hit)
            period_counts[hit["period"]] += 1
            period_bases[hit["period"]] += hit["length"]
            total_ssr_bases += hit["length"]
            union_ssr_bases += max(0, hit["end"] - max(hit["start"], cover_end))
            cover_end = max(cover_end, hit["end"])
        if batch:
            hit_writer.write_table(pa.table({
                key: pa.array([row[key] for row in batch], type=value)
                for key, value in hit_schema.items()
            }))
        carry_start += cut
        carry = combined[cut:]

    with open_fn(fasta, "rt") as handle:
        for line in handle:
            if line.startswith(">"):
                if chrom is not None:
                    flush(final=True)
                    chrom_lengths[chrom] = length
                chrom = line[1:].split()[0]
                cover_end = 0
                block, carry, carry_start, length, block_length = [], "", 0, 0, 0
            else:
                # Soft-masked (lowercase) bases are sequence too: T2T is 61.7 % lowercase, and the repeats are mostly there.
                sequence = line.strip().upper()
                block.append(sequence)
                acgt = sequence.count("A") + sequence.count("C") + sequence.count("G") + sequence.count("T")
                base_counts["acgt"] += acgt
                base_counts["n"] += sequence.count("N")
                base_counts["other"] += len(sequence) - acgt - sequence.count("N")
                length += len(sequence)
                block_length += len(sequence)
                if block_length >= chunk_bases:
                    flush(final=False)
        if chrom is not None:
            flush(final=True)
            chrom_lengths[chrom] = length
    hit_writer.close()

    summaries = []
    for period in sorted(PERIOD_NAMES):
        summaries.append({
            "period": period, "name": PERIOD_NAMES[period], "count": period_counts[period],
            "total_length": period_bases[period],
            "avg_repeat_count": period_bases[period] / (period_counts[period] * period)
            if period_counts[period] else 0.0,
        })

    genome_length = sum(chrom_lengths.values())
    acgt_bases = base_counts["acgt"]
    coverage = []
    for period in sorted(PERIOD_NAMES):
        observed = period_bases[period]
        coverage.append({
            "period": period, "name": PERIOD_NAMES[period], "count": period_counts[period], "observed_bases": observed,
            "genome_bases": genome_length, "acgt_bases": acgt_bases, "n_bases": base_counts["n"],
            "other_bases": base_counts["other"],
            "coverage_fraction": observed / acgt_bases if acgt_bases else None,
            "density_per_mb": period_counts[period] / acgt_bases * 1e6 if acgt_bases else None,
            "sum_ssr_bases": total_ssr_bases, "union_ssr_bases": union_ssr_bases,
            "overlap_excess_bases": total_ssr_bases - union_ssr_bases,
        })

    write(out / "ssr_summary.parquet", summaries, {
        "period": pa.uint8(), "name": pa.string(), "count": pa.uint64(),
        "total_length": pa.uint64(), "avg_repeat_count": pa.float64(),
    })
    write(out / "ssr_coverage.parquet", coverage, {
        "period": pa.uint8(), "name": pa.string(), "count": pa.uint64(), "observed_bases": pa.uint64(),
        "genome_bases": pa.uint64(), "acgt_bases": pa.uint64(), "n_bases": pa.uint64(), "other_bases": pa.uint64(),
        "coverage_fraction": pa.float64(), "density_per_mb": pa.float64(),
        "sum_ssr_bases": pa.uint64(), "union_ssr_bases": pa.uint64(), "overlap_excess_bases": pa.uint64(),
    })


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-f", "--fasta", required=True)
    parser.add_argument("-o", "--out-dir", required=True)
    parser.add_argument("--chunk-bases", type=int, default=4_000_000)
    add_resource_args(parser)
    args = parser.parse_args()
    chunk_bases = min(args.chunk_bases, max(100_000, effective_memory_mb(args.memory_mb) * 250))
    extract(args.fasta, args.out_dir, chunk_bases)
