#!/usr/bin/env python3
"""Post-check: a window track must tile every contig exactly (audit 04, M1/C18).

For each contig in the .fai: first window starts at 0, windows are contiguous (no gap, no overlap, no duplicate),
all but the last have the nominal width, the last ends at the contig length. A track that stops early (as hecate_mprob
did before the S1 fix) fails loudly instead of leaving silent blank regions.
"""
import argparse
import sys
from pathlib import Path

import duckdb


def check_track(con, path: Path, lengths: dict, window: int) -> list:
    errors = []
    rows = con.execute(
        """SELECT contig, CAST(start_pos AS BIGINT), CAST(end_pos AS BIGINT) FROM read_parquet(?) ORDER BY contig, start_pos""",
        [str(path)],
    ).fetchall()
    by_contig = {}
    for contig, s, e in rows:
        by_contig.setdefault(contig, []).append((s, e))
    for contig in sorted(set(by_contig) - set(lengths)):
        errors.append(f"{path.name}: contig {contig} not in .fai")
    for contig, length in lengths.items():
        wins = by_contig.get(contig)
        if not wins:
            errors.append(f"{path.name}: {contig}: no rows (expected {length} bp)")
            continue
        expected = -(-length // window)
        if len(wins) != expected:
            errors.append(f"{path.name}: {contig}: {len(wins)} windows, expected {expected}")
        prev_end = 0
        for i, (s, e) in enumerate(wins):
            last = i == len(wins) - 1
            if s != prev_end:
                errors.append(f"{path.name}: {contig}: window {i} starts at {s}, expected {prev_end} (gap or overlap)")
                break
            if (not last and e - s != window) or e <= s:
                errors.append(f"{path.name}: {contig}: window {i} [{s},{e}) has wrong width")
                break
            prev_end = e
        else:
            if prev_end != length:
                errors.append(f"{path.name}: {contig}: ends at {prev_end}, contig length is {length}")
    return errors


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fai", required=True)
    ap.add_argument("--tracks-dir", required=True)
    ap.add_argument("--tracks", nargs="+", default=["gc_content", "shannon_entropy", "hecate_mprob"])
    ap.add_argument("--window", type=int, default=1000, help="nominal window width (D31: 1000)")
    args = ap.parse_args()

    lengths = {}
    for line in Path(args.fai).read_text().splitlines():
        name, length = line.split("\t")[:2]
        lengths[name] = int(length)
    con = duckdb.connect()
    errors = []
    for name in args.tracks:
        path = Path(args.tracks_dir) / f"{name}.parquet"
        if not path.exists():
            errors.append(f"{name}: file missing ({path})")
            continue
        errs = check_track(con, path, lengths, args.window)
        print(f"{'FAIL' if errs else 'ok  '} {name}: {len(lengths)} contigs")
        errors += errs
    for e in errors[:30]:
        print("  " + e, file=sys.stderr)
    if len(errors) > 30:
        print(f"  ... {len(errors) - 30} more", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
