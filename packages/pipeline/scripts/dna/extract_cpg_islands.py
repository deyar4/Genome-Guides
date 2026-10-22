#!/usr/bin/env python3
"""CpG islands as merged runs of passing 200 bp windows (spec docs/analyses/01_cpg_islands.md).

Coordinates are 0-based, half-open. cg[p] = 1 iff base p is C and base p+1 is G. Window i = [s, s+W); cg_in[i] counts
cg[p] for p in [s, s+W-1); a CG on the last base of window i-1 and the first base of window i is the straddle at the
boundary and belongs to a region containing both windows (counted once). All threshold tests are exact integer tests.
Soft-masked bases are folded to uppercase. Windows with any N and the partial last window are never candidates.

Summary tallies that account for every base: head_bases_not_tiled (the [0, grid_offset) bases before the grid) and
tail_bases_not_tiled (the partial last window) are not candidates; non_acgtn_bases counts bases in tiled windows that
are neither A, C, G, T nor N (IUPAC codes such as R, Y), non_acgtn_bases_in_tail the same for the tail. Such bases count
as "other": they are in the window length but in neither C, G nor N.

--legacy-out reproduces the previous bedtools/seqkit/awk track (>= thresholds, cpg_count >= 4, N in the denominator,
partial last window, straddling CG counted in both windows, no merging) for the equivalence check of the spec."""

import argparse
import json
from fractions import Fraction
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "common"))
from resource_args import add_resource_args

A, C, G, T, N = (ord(ch) for ch in "ACGTN")


def read_fai(path):
    entries = []
    for line in Path(path).read_text().splitlines():
        name, length, offset, line_bases, line_width = line.split("\t")[:5]
        entries.append((name, int(length), int(offset), int(line_bases), int(line_width)))
    return entries


def contig_blocks(handle, length, offset, line_bases, line_width, block_bases):
    """Yield the contig as uint8 arrays of about block_bases bases, newlines removed."""
    handle.seek(offset)
    remaining = length
    lines_per_read = max(1, block_bases // line_bases)
    while remaining > 0:
        want = min(remaining, lines_per_read * line_bases)
        n_lines = -(-want // line_bases)
        raw = handle.read(n_lines * line_width)
        seq = raw.replace(b"\n", b"").replace(b"\r", b"")[:want]
        if len(seq) != want:
            raise ValueError("FASTA shorter than its .fai entry")
        remaining -= want
        yield np.frombuffer(seq, dtype=np.uint8)


def window_stats(rows):
    """rows: 2-D uint8 array (windows x width). Per-window counts as int64 arrays."""
    a = rows & 0xDF  # fold case; only letters matter here
    is_c, is_g, is_n = a == C, a == G, a == N
    other = ~((a == A) | is_c | is_g | (a == T) | is_n)  # IUPAC codes other than N (R, Y, ...) and any other byte
    return {
        "c": is_c.sum(1, dtype=np.int64), "g": is_g.sum(1, dtype=np.int64), "n": is_n.sum(1, dtype=np.int64),
        "x": other.sum(1, dtype=np.int64),
        "cg_in": (is_c[:, :-1] & is_g[:, 1:]).sum(1, dtype=np.int64),
        "first_g": is_g[:, 0], "last_c": is_c[:, -1],
    }


def contig_window_stats(handle, entry, window, block_bases, offset):
    """Stats of the full windows of one contig (grid starting at `offset`) plus the partial tail as a dict or None."""
    name, length, foff, line_bases, line_width = entry
    parts, carry, skip = [], np.empty(0, dtype=np.uint8), offset
    for block in contig_blocks(handle, length, foff, line_bases, line_width, block_bases):
        if skip:
            drop = min(skip, len(block))
            block, skip = block[drop:], skip - drop
        buf = np.concatenate([carry, block]) if len(carry) else block
        nw = len(buf) // window
        if nw:
            parts.append(window_stats(buf[: nw * window].reshape(nw, window)))
        carry = buf[nw * window:]
    keys = ("c", "g", "n", "x", "cg_in", "first_g", "last_c")
    stats = {k: np.concatenate([p[k] for p in parts]) if parts else np.empty(0, dtype=np.int64) for k in keys}
    tail = window_stats(carry[None, :]) if len(carry) > 1 else None
    return stats, (len(carry) if len(carry) else 0), tail, carry


def evaluate(c, g, cg, length, p):
    """Integer-exact island test of a region. Returns (gc_ok, oe_ok, strict)."""
    gc_ok = 100 * (c + g) * p["gc_den"] > p["gc_num"] * length
    oe_ok = c * g > 0 and cg * length * p["oe_den"] > p["oe_num"] * c * g
    strict = (length >= p["s_len"] and 100 * (c + g) * p["sgc_den"] >= p["sgc_num"] * length
              and c * g > 0 and cg * length * p["soe_den"] >= p["soe_num"] * c * g)
    return gc_ok, oe_ok, strict


def find_islands(name, offset, window, st, p, tally):
    nw = len(st["c"])
    if nw == 0:
        return []
    c, g, n, cgi = st["c"], st["g"], st["n"], st["cg_in"]
    cand = (n == 0) & (100 * (c + g) * p["gc_den"] > p["gc_num"] * window) \
        & (c * g > 0) & (cgi * window * p["oe_den"] > p["oe_num"] * c * g)
    tally["windows"] += nw
    tally["windows_with_n"] += int((n > 0).sum())
    tally["non_acgtn_bases"] += int(st["x"].sum())
    tally["candidate_windows"] += int(cand.sum())
    straddle = np.zeros(nw, dtype=np.int64)
    straddle[1:] = st["last_c"][:-1] & st["first_g"][1:]
    cum = np.cumsum(straddle)
    padded = np.concatenate([[0], cand.astype(np.int8), [0]])
    edges = np.flatnonzero(np.diff(padded))
    out = []
    for a, b in zip(edges[0::2], edges[1::2] - 1):  # inclusive window indices of the run
        rc, rg = int(c[a:b + 1].sum()), int(g[a:b + 1].sum())
        cg = int(cgi[a:b + 1].sum()) + int(cum[b] - cum[a])
        length = (b - a + 1) * window
        tally["runs"] += 1
        gc_ok, oe_ok, strict = evaluate(rc, rg, cg, length, p)
        if not (gc_ok and oe_ok and length >= p["min_len"]):
            tally["runs_rejected"] += 1
            tally["runs_rejected_bp"] += length
            continue
        out.append({
            "contig": name, "start_pos": offset + int(a) * window, "end_pos": offset + (int(b) + 1) * window,
            "length": length, "gc_pct": 100.0 * (rc + rg) / length, "obs_exp_ratio": cg * length / (rc * rg),
            "cpg_count": cg, "n_windows": int(b - a + 1), "strict": 1 if strict else 0,
        })
    return out


def legacy_rows(name, window, st, tail_stats, tail_len):
    """Old definition: window rows with >= thresholds, cpg_count >= 4, straddle CG in both windows, N in length."""
    rows = []
    nw = len(st["c"])
    extra = np.zeros(nw, dtype=np.int64)
    if nw > 1:
        boundary = st["last_c"][:-1] & st["first_g"][1:]
        extra[:-1] += boundary
        extra[1:] += boundary
    for i in range(nw):
        c, g = int(st["c"][i]), int(st["g"][i])
        cg = int(st["cg_in"][i]) + int(extra[i])
        rows.append((name, i * window, (i + 1) * window, window, c, g, cg))
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fasta", required=True)
    ap.add_argument("--fai", required=True)
    ap.add_argument("--out-parquet", required=True)
    ap.add_argument("--out-summary", required=True)
    ap.add_argument("--out-meta", required=True)
    ap.add_argument("--window", type=int, default=200)
    ap.add_argument("--min-gc", default="50", help="percent, strict >, rational")
    ap.add_argument("--min-oe", default="3/5", help="strict >, rational")
    ap.add_argument("--min-length", type=int, default=200)
    ap.add_argument("--strict-min-length", type=int, default=500)
    ap.add_argument("--strict-min-gc", default="55", help="inclusive >=")
    ap.add_argument("--strict-min-oe", default="13/20", help="inclusive >=")
    ap.add_argument("--block-bases", type=int, default=16_000_000, help="memory knob, does not change results")
    ap.add_argument("--grid-offset", type=int, default=0, help="sensitivity: start of the window grid")
    ap.add_argument("--legacy-out", default=None, help="also write the previous definition's windows (TSV)")
    add_resource_args(ap)
    args = ap.parse_args()

    gc, oe, sgc, soe = (Fraction(x) for x in (args.min_gc, args.min_oe, args.strict_min_gc, args.strict_min_oe))
    p = {"gc_num": gc.numerator, "gc_den": gc.denominator, "oe_num": oe.numerator, "oe_den": oe.denominator,
         "sgc_num": sgc.numerator, "sgc_den": sgc.denominator, "soe_num": soe.numerator, "soe_den": soe.denominator,
         "s_len": args.strict_min_length, "min_len": args.min_length}
    window = args.window
    if window < 2 or args.block_bases < window:
        raise SystemExit("window must be >= 2 and block-bases >= window")
    tally = {k: 0 for k in ("windows", "windows_with_n", "candidate_windows", "runs", "runs_rejected",
                            "runs_rejected_bp", "partial_windows", "contigs_shorter_than_window",
                            "head_bases_not_tiled", "tail_bases_not_tiled", "non_acgtn_bases",
                            "non_acgtn_bases_in_tail")}
    islands, legacy = [], []
    with open(args.fasta, "rb") as handle:
        for entry in read_fai(args.fai):
            name = entry[0]
            st, tail_len, tail, carry = contig_window_stats(handle, entry, window, args.block_bases, args.grid_offset)
            if len(st["c"]) == 0 and entry[1] < window:
                tally["contigs_shorter_than_window"] += 1
            tally["head_bases_not_tiled"] += min(args.grid_offset, entry[1])
            tally["tail_bases_not_tiled"] += tail_len
            tally["non_acgtn_bases_in_tail"] += int((~np.isin(carry & 0xDF, (A, C, G, T, N))).sum())
            if tail_len:
                tally["partial_windows"] += 1
            islands += find_islands(name, args.grid_offset, window, st, p, tally)
            if args.legacy_out:
                legacy += legacy_rows(name, window, st, tail, tail_len)
                if tail is not None:
                    c, g = int(tail["c"][0]), int(tail["g"][0])
                    start = entry[1] - tail_len
                    cg = int(tail["cg_in"][0]) + (
                        1 if len(st["c"]) and st["last_c"][-1] and tail["first_g"][0] else 0)
                    legacy.append((name, start, entry[1], tail_len, c, g, cg))
                    if len(st["c"]):  # the straddle with the last full window also counts there
                        i = len(legacy) - 2
                        if st["last_c"][-1] and tail["first_g"][0]:
                            n0 = legacy[i]
                            legacy[i] = n0[:6] + (n0[6] + 1,)

    schema = pa.schema([("contig", pa.string()), ("start_pos", pa.uint64()), ("end_pos", pa.uint64()),
                        ("length", pa.uint32()), ("gc_pct", pa.float32()), ("obs_exp_ratio", pa.float32()),
                        ("cpg_count", pa.uint64()), ("n_windows", pa.uint32()), ("strict", pa.uint8())])
    table = pa.table({f.name: pa.array([r[f.name] for r in islands], type=f.type) for f in schema}, schema=schema)
    Path(args.out_parquet).parent.mkdir(parents=True, exist_ok=True)
    pq.write_table(table, args.out_parquet, compression="zstd")

    summary = dict(tally, islands=len(islands), strict_islands=sum(r["strict"] for r in islands),
                   covered_bp=sum(r["length"] for r in islands))
    Path(args.out_summary).write_text("".join(f"{k}\t{v}\n" for k, v in summary.items()))
    Path(args.out_meta).write_text(json.dumps({
        "track": "cpg_islands", "coordinates": "0-based, half-open [start_pos, end_pos)",
        "definition": "Maximal runs of adjacent 200 bp tiled windows that each have gc > min_gc percent, "
                      "obs/exp > min_oe, and no N, kept iff the whole run again satisfies both and length >= min_length. "
                      "gc_pct = 100 (C+G)/length; obs_exp_ratio = CG * length / (C * G), dimensionless. "
                      "strict = 1 iff length >= 500, gc_pct >= 55 and obs_exp_ratio >= 0.65. Grid-anchored at "
                      f"{args.grid_offset}; see docs/analyses/01_cpg_islands.md.",
        "parameters": {"window": window, "min_gc": args.min_gc, "min_oe": args.min_oe, "min_length": args.min_length,
                       "strict_min_length": args.strict_min_length, "strict_min_gc": args.strict_min_gc,
                       "strict_min_oe": args.strict_min_oe, "grid_offset": args.grid_offset},
        "units": {"gc_pct": "percent", "obs_exp_ratio": "ratio", "length": "bp"},
    }, indent=2))
    if args.legacy_out:
        with open(args.legacy_out, "w") as out:
            for name, s, e, length, c, g, cg in legacy:
                gc_pct = (c + g) / length * 100.0
                expected = c * g / length
                ratio = cg / expected if expected > 0 else 0.0
                if gc_pct >= 50.0 and ratio >= 0.60 and cg >= 4:
                    out.write(f"{name}\t{s}\t{e}\t{cg}\n")


if __name__ == "__main__":
    main()
