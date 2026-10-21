#!/usr/bin/env python3
"""Download a reference file, optionally gunzip it, verify its sha256 and only then move it to the final path.

A truncated or changed download therefore never appears at the output path (audit 02, W2; D1, D18).
sha256 is checked on the bytes written to the output (after gunzip if --gunzip). Trust on first use: the pinned
values in config.yaml are hashes of the files that produced the thesis results, not upstream-published checksums.
"""
import argparse
import gzip
import hashlib
import os
import shutil
import sys
import time
import urllib.request


def sha256_of(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for block in iter(lambda: fh.read(1 << 22), b""):
            h.update(block)
    return h.hexdigest()


def fetch(url: str, part: str, gunzip: bool, retries: int) -> None:
    last = None
    for attempt in range(1, retries + 1):
        try:
            # urlopen raises on HTTP >= 400 (the equivalent of curl --fail)
            with urllib.request.urlopen(url, timeout=60) as resp, open(part, "wb") as out:
                src = gzip.GzipFile(fileobj=resp) if gunzip else resp
                shutil.copyfileobj(src, out, 1 << 22)
            return
        except Exception as exc:  # network, HTTP, truncated gzip (EOFError / BadGzipFile)
            last = exc
            print(f"attempt {attempt}/{retries} failed: {exc!r}", file=sys.stderr)
            time.sleep(min(2 ** attempt, 30) if retries > 1 and attempt < retries else 0)
    raise SystemExit(f"download failed after {retries} attempts: {last!r}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", required=True)
    ap.add_argument("--sha256", required=True)
    ap.add_argument("--output", required=True)
    ap.add_argument("--gunzip", action="store_true")
    ap.add_argument("--retries", type=int, default=3)
    args = ap.parse_args()

    os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
    part = args.output + ".part"
    fetch(args.url, part, args.gunzip, args.retries)
    got = sha256_of(part)
    if got != args.sha256.lower():
        os.replace(part, args.output + ".rejected")
        raise SystemExit(
            f"sha256 mismatch for {args.url}\n  expected {args.sha256}\n  got      {got}\n"
            f"  rejected file kept as {args.output}.rejected (not used); update config.yaml only after reviewing why it changed"
        )
    os.replace(part, args.output)
    print(f"ok {args.output} sha256={got}")


if __name__ == "__main__":
    main()
