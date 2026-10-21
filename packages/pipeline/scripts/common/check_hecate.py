#!/usr/bin/env python3
"""Check the hecate binary against the pinned sha256 and write a provenance file.

The commit is the real pin (enforced when building). The hash additionally detects a swapped binary, but a build
with another Rust or C compiler may legitimately differ, so a mismatch is a warning unless --strict is given
(used on the reference machine and in the Docker image).
"""
import argparse
import hashlib
import platform
import sys
from pathlib import Path


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--binary", required=True)
    ap.add_argument("--commit", required=True)
    ap.add_argument("--expected", action="append", default=[], metavar="PLATFORM=SHA256")
    ap.add_argument("--strict", action="store_true", help="fail on a hash mismatch or a platform without a pinned hash")
    ap.add_argument("-o", "--output", required=True)
    args = ap.parse_args()

    key = f"{platform.system().lower()}-{platform.machine().lower()}"
    expected = dict(e.split("=", 1) for e in args.expected)
    actual = sha256_of(args.binary)
    if key not in expected:
        status = "unpinned"
        msg = f"no pinned sha256 for platform '{key}' (binary has {actual})"
    elif actual != expected[key]:
        status = "mismatch"
        msg = f"sha256 {actual} differs from the pin {expected[key]} for {key}; different toolchain or a swapped binary"
    else:
        status, msg = "match", ""
    if msg:
        if args.strict:
            sys.exit(f"hecate binary {args.binary}: {msg}. Delete it to rebuild from commit {args.commit}, "
                     f"or update the pin deliberately.")
        print(f"WARNING hecate binary {args.binary}: {msg}. Recorded in the provenance file.", file=sys.stderr)

    info = Path(args.binary + ".build-info")
    build = info.read_text().strip().splitlines() if info.exists() else ["build-info\tmissing (binary not built by the workflow)"]
    lines = [f"commit\t{args.commit}", f"platform\t{key}", f"sha256\t{actual}", f"hash_status\t{status}"] + build
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    Path(args.output).write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
