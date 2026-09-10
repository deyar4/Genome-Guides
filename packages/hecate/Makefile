.PHONY: build test lint cov cov-html cov-summary cov-gaps clean

build:
	cargo build --release

test:
	cargo test

lint:
	cargo clippy

cov-html:
	cargo llvm-cov --html --output-dir target/cov
	@echo "Report: target/cov/html/index.html"

cov-summary:
	@cargo llvm-cov --json --output-path target/cov.json 2>/dev/null
	@python3 scripts/cov-gaps.py summary target/cov.json

cov-gaps:
	@cargo llvm-cov --text --output-path /tmp/hecate-cov-text.txt 2>/dev/null
	@python3 scripts/cov-gaps.py gaps /tmp/hecate-cov-text.txt

cov: cov-summary
	@echo
	@cargo llvm-cov --text --output-path /tmp/hecate-cov-text.txt 2>/dev/null
	@echo "=== UNCOVERED LINES PER FILE ==="
	@python3 scripts/cov-gaps.py gaps /tmp/hecate-cov-text.txt

clean:
	cargo clean
