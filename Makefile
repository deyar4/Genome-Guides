# Master Makefile for Genome Guides Monorepo

.PHONY: env-create env-update pipeline-chrm pipeline-t2t backend-dev test clean

env-create:
	mamba env create -f environment.yml

env-update:
	mamba env update -f environment.yml --prune

pipeline-chrm:
	mamba run -n genome-guides snakemake -s packages/pipeline/workflow/Snakefile --unlock || true
	mamba run -n genome-guides snakemake -s packages/pipeline/workflow/Snakefile --cores 8 --resources mem_mb=8192 chrm_all

pipeline-t2t:
	mamba run -n genome-guides snakemake -s packages/pipeline/workflow/Snakefile --unlock || true
	mamba run -n genome-guides snakemake -s packages/pipeline/workflow/Snakefile --cores 12 --resources mem_mb=12000 --rerun-incomplete t2t_all

backend-dev:
	cd apps/backend && uv run uvicorn app.main:app --reload --port 8000

test:
	python3 packages/pipeline/scripts/tests/test_pipeline.py
	cd apps/backend && uv run pytest

clean:
	rm -rf data/interim/* data/tracks/* .snakemake
