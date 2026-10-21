#!/usr/bin/env bash
# Build hecate at a pinned upstream commit into a path-independent binary (same sha256 from any directory).
# Usage: build_hecate.sh REPO COMMIT OUTPUT_BINARY [SUBDIR [PATCH...]]
# PATCH files (git diff format, paths relative to the cloned repo root) are applied in the order given.
# REPO is a git URL or a local path (cloned from its history); SUBDIR is the Cargo project inside it (default: root).
# Needs: git, a Rust toolchain (cargo), a C compiler with OpenMP (libgomp). See upstream INSTALL.
set -euo pipefail
repo="$1"; commit="$2"; out="$3"; subdir="${4:-.}"; shift $(( $# < 4 ? $# : 4 ))
patches=()
for p in "$@"; do patches+=("$(realpath "$p")"); done
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone --quiet "$repo" "$work/src"
git -C "$work/src" checkout --quiet "$commit"
[ "$(git -C "$work/src" rev-parse HEAD)" = "$commit" ] || { echo "commit mismatch: expected a full 40-char hash" >&2; exit 1; }
for p in "${patches[@]}"; do git -C "$work/src" apply --check "$p" && git -C "$work/src" apply "$p"; done
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
# -fopenmp: the linker does not pull in libgomp by itself on some systems (undefined GOMP_parallel).
# remap-path-prefix: keeps build directories out of the binary so the hash is reproducible.
(cd "$work/src/$subdir" && RUSTFLAGS="-C link-arg=-fopenmp --remap-path-prefix=$work/src=/build --remap-path-prefix=$cargo_home=/cargo" \
    cargo build --release --locked)
mkdir -p "$(dirname "$out")"
cp "$work/src/$subdir/target/release/hecate" "$out"
# Build provenance next to the binary (read by check_hecate.py).
{
    echo "commit	$commit"
    echo "rustc	$(rustc --version)"
    echo "cc	$(${CC:-cc} --version | head -n 1)"
    for p in "${patches[@]}"; do echo "patch	$(basename "$p")	$(sha256sum "$p" | cut -d" " -f1)"; done
} > "$out.build-info"
