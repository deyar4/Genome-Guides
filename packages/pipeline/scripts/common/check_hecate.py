#!/usr/bin/env python3
"""Verify the hecate binary against the sha256 pinned in the config and write a provenance file."""
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
    ap.add_argument("-o", "--output", required=True)
    args = ap.parse_args()

    key = f"{platform.system().lower()}-{platform.machine().lower()}"
    expected = dict(e.split("=", 1) for e in args.expected)
    actual = sha256_of(args.binary)
    if key not in expected:
        sys.exit(f"No pinned sha256 for platform '{key}'. Binary has {actual}. Build it with "
                 f"scripts/common/build_hecate.sh, test it, and add it to hecate_bwt.sha256 in the config.")
    if actual != expected[key]:
        sys.exit(f"hecate binary {args.binary} has sha256 {actual}, config pins {expected[key]} for {key}. "
                 f"Delete the binary to rebuild it from commit {args.commit}, or update the pin deliberately.")
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    Path(args.output).write_text(f"commit\t{args.commit}\nplatform\t{key}\nsha256\t{actual}\n")


if __name__ == "__main__":
    main()
