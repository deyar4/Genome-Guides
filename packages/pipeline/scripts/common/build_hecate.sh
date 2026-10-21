#!/usr/bin/env bash
# Build hecate at a pinned upstream commit into a path-independent binary (same sha256 from any directory).
# Usage: build_hecate.sh REPO_URL COMMIT OUTPUT_BINARY
# Needs: git, a Rust toolchain (cargo), a C compiler with OpenMP (libgomp). See upstream INSTALL.
set -euo pipefail
repo="$1"; commit="$2"; out="$3"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone --quiet "$repo" "$work/src"
git -C "$work/src" checkout --quiet "$commit"
[ "$(git -C "$work/src" rev-parse HEAD)" = "$commit" ] || { echo "commit mismatch: expected a full 40-char hash" >&2; exit 1; }
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
# -fopenmp: the linker does not pull in libgomp by itself on some systems (undefined GOMP_parallel).
# remap-path-prefix: keeps build directories out of the binary so the hash is reproducible.
(cd "$work/src" && RUSTFLAGS="-C link-arg=-fopenmp --remap-path-prefix=$work/src=/build --remap-path-prefix=$cargo_home=/cargo" \
    cargo build --release --locked)
mkdir -p "$(dirname "$out")"
cp "$work/src/target/release/hecate" "$out"
# Build provenance next to the binary (read by check_hecate.py).
{
    echo "commit	$commit"
    echo "rustc	$(rustc --version)"
    echo "cc	$(${CC:-cc} --version | head -n 1)"
} > "$out.build-info"
