# Master Makefile for Genome Guides Monorepo
# Resources are variables: make pipeline-chrm CORES=6 MEM_MB=12000

CORES  ?= $(shell nproc)
MEM_MB ?= 8192
RUN    := mamba run -n genome-guides
SMK    := $(RUN) snakemake -s packages/pipeline/workflow/Snakefile --cores $(CORES) --resources mem_mb=$(MEM_MB) --rerun-incomplete

.NOTPARALLEL:

.PHONY: check-benchmarks package-tracks check-fast check-numbers check-calibration env-create env-update unlock pipeline-tiny pipeline-satslice golden pipeline-chrm pipeline-t2t backend-dev test check check-coverage dry-run web-build clean clean-chrm clean-t2t

env-create:
	mamba env create -f environment.yml

env-update:
	mamba env update -f environment.yml --prune

# Run only after a crashed run left a stale lock; never part of a normal run (D22).
unlock:
	$(RUN) snakemake -s packages/pipeline/workflow/Snakefile --unlock

pipeline-chrm:
	$(SMK) -- chrm_all

pipeline-tiny:
	$(SMK) -- all

# 1 Mb satellite slice (tests/fixtures/satslice.fa): runs the real gc, entropy and hecate rules, about 1 min
pipeline-satslice:
	$(SMK) -- satslice_all

golden: pipeline-satslice
	$(RUN) python3 -m pytest -q tests/golden

pipeline-t2t:
	$(SMK) -- t2t_all

backend-dev:
	cd apps/backend && $(RUN) uv run uvicorn app.main:app --reload --port 8000

test:
	$(RUN) python3 packages/pipeline/scripts/tests/test_pipeline.py
	$(RUN) python3 -m pytest -q -rs packages/pipeline/scripts/tests
	cd apps/backend && $(RUN) uv run pytest -rs

# check-fast: no pipeline run, no tool builds (what CI can run): unit tests (skips are listed with -rs), workflow dry run,
# frontend lint, typecheck, tests and build.
check-fast: test dry-run web-build

# check: check-fast plus the tiny and 1 Mb satellite runs with golden comparison (need seqkit, bedtools and the pinned
# hecate build) and the tiling check of the t2t window tracks when present. Opt-in, longer: check-numbers, check-calibration.
check: check-fast pipeline-tiny golden check-coverage

dry-run:
	$(RUN) snakemake -s packages/pipeline/workflow/Snakefile --cores 1 --dry-run --quiet -- all chrm_all t2t_all

web-build:
	$(RUN) pnpm --filter web run lint
	$(RUN) pnpm --filter web run typecheck
	$(RUN) pnpm --filter web run test
	$(RUN) pnpm --filter web run build

# Recomputes the numbers quoted in the logbook/specs for the CpG and gene-unit analyses; each check writes a TSV to
# artifacts/checks/. The legacy-equality check re-runs the old bedtools/seqkit pipeline: several minutes, about 8 GB disk.
check-numbers:
	mkdir -p artifacts/checks
	$(RUN) python3 packages/pipeline/scripts/checks/gene_unit_checks.py --tracks-dir data/tracks/t2t/all --out artifacts/checks/gene_unit_checks.tsv
	$(RUN) python3 packages/pipeline/scripts/checks/cpg_island_recount.py --fasta data/raw/t2t.fa --parquet data/tracks/t2t/all/cpg_islands.parquet --out artifacts/checks/cpg_island_recount.tsv
	$(RUN) python3 packages/pipeline/scripts/checks/cpg_grid_shift.py --fasta data/raw/t2t.fa --fai data/raw/t2t.fa.fai --baseline-parquet data/tracks/t2t/all/cpg_islands.parquet --out artifacts/checks/cpg_grid_shift.tsv
	mkdir -p artifacts/checks/cpg_legacy_work
	$(RUN) python3 packages/pipeline/scripts/checks/cpg_legacy_equality.py --fasta data/raw/t2t.fa --fai data/raw/t2t.fa.fai --workdir artifacts/checks/cpg_legacy_work --out artifacts/checks/cpg_legacy_equality.tsv
	rm -rf artifacts/checks/cpg_legacy_work

# CpG calibration on hg38 chr21/22 (downloads about 182 MB once) and the T2T island breakdown by RepeatMasker class.
check-calibration:
	$(RUN) bash packages/pipeline/scripts/checks/cpg_calibration.sh
	$(RUN) python3 packages/pipeline/scripts/checks/cpg_calibration.py --islands data/tracks/t2t/all/cpg_islands.parquet --rmsk data/raw/t2t_repeatmasker.out.gz --contigs $$(cut -f1 data/raw/t2t.fa.fai) --out artifacts/checks/cpg_calibration_t2t.tsv

# Tracks (and with INCLUDE_RAW=1 the FASTA and index the sequence endpoints read) as one tarball with sha256 MANIFEST and the commit.
package-tracks:
	@test -d data/tracks/t2t/all || { echo "package-tracks: no data/tracks/t2t/all"; exit 1; }
	mkdir -p artifacts/package
	cd data && { find tracks -type f \( -name '*.parquet' -o -name 'sequence_stats.txt' \); $(if $(INCLUDE_RAW),ls raw/t2t.fa raw/t2t.fa.fai;) } | LC_ALL=C sort > ../artifacts/package/FILES
	cd data && xargs -a ../artifacts/package/FILES sha256sum > ../artifacts/package/MANIFEST.sha256
	git rev-parse HEAD > artifacts/package/COMMIT
	tar -czf artifacts/package/genome-guides-tracks.tar.gz -C data -T artifacts/package/FILES -C ../artifacts/package MANIFEST.sha256 COMMIT
	@echo "wrote artifacts/package/ (extract the tarball, then: sha256sum -c MANIFEST.sha256)"

# Wall time and peak RSS per rule from benchmarks/t2t next to the declared mem_mb (needs a previous t2t run, no data files).
check-benchmarks:
	mkdir -p artifacts/checks
	$(RUN) python3 packages/pipeline/scripts/checks/benchmark_table.py --bench-dir benchmarks/t2t --out artifacts/checks/benchmark_t2t.tsv

check-coverage:
	@if [ -f data/raw/t2t.fa.fai ] && [ -f data/tracks/t2t/all/gc_content.parquet ]; then \
		$(RUN) python3 packages/pipeline/scripts/common/check_coverage.py --fai data/raw/t2t.fa.fai --tracks-dir data/tracks/t2t/all; \
	else echo "check-coverage: no t2t tracks yet, skipped"; fi

# Smoke-test outputs only; the t2t tracks and .snakemake metadata are kept.
clean-chrm:
	rm -rf data/interim/chrm data/tracks/chrm benchmarks/chrm

# Deletes the full-genome results (hours to recompute). Needs CONFIRM=yes.
clean-t2t:
	@test "$(CONFIRM)" = "yes" || { echo "clean-t2t deletes t2t results; rerun with CONFIRM=yes"; exit 1; }
	rm -rf data/interim/t2t data/tracks/t2t benchmarks/t2t

clean:
	@echo "Use clean-chrm or clean-t2t CONFIRM=yes (D22)."; exit 1
