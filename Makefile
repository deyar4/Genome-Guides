# Master Makefile for Genome Guides Monorepo
# Resources are variables: make pipeline-chrm CORES=6 MEM_MB=12000

CORES  ?= $(shell nproc)
MEM_MB ?= 8192
RUN    := mamba run -n genome-guides
SMK    := $(RUN) snakemake -s packages/pipeline/workflow/Snakefile --cores $(CORES) --resources mem_mb=$(MEM_MB) --rerun-incomplete

.PHONY: env-create env-update unlock pipeline-chrm pipeline-t2t backend-dev test check check-coverage clean clean-chrm clean-t2t

env-create:
	mamba env create -f environment.yml

env-update:
	mamba env update -f environment.yml --prune

# Run only after a crashed run left a stale lock; never part of a normal run (D22).
unlock:
	$(RUN) snakemake -s packages/pipeline/workflow/Snakefile --unlock

pipeline-chrm:
	$(SMK) -- chrm_all

pipeline-t2t:
	$(SMK) -- t2t_all

backend-dev:
	cd apps/backend && $(RUN) uv run uvicorn app.main:app --reload --port 8000

test:
	$(RUN) python3 packages/pipeline/scripts/tests/test_pipeline.py
	cd apps/backend && $(RUN) uv run pytest

# Everything that can be checked without a long run: unit tests, then the tiling check of the t2t window tracks when present.
check: test check-coverage

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
