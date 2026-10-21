# Genome-Guides

Genome-Guides is a Snakemake pipeline and web application for human-reference
genome analysis. It produces sequence-complexity, mappability, entropy,
repeat, gene, RNA, and protein tracks for browser and API access.

## Repository layout

| Path | Purpose |
| --- | --- |
| `packages/pipeline/workflow/` | Snakemake entrypoint and rule files |
| `packages/pipeline/scripts/` | Standalone analytical and conversion scripts |
| `packages/pipeline/config.yaml` | Input paths, thresholds, windows, and resource defaults |
| `apps/backend/` | FastAPI service and DuckDB/Parquet access |
| `apps/web/` | Next.js frontend |
| `docs/` | Architecture and data-reference documents |

Generated genome data belongs under `data/` and is ignored by Git.

## Requirements

- Linux or macOS
- Conda or micromamba
- Git, network access for the first run (reference downloads and the hecate build)
- A C compiler with OpenMP (system `gcc`); Rust, Node.js and pnpm come from the conda environment

The supported Python environment is defined in `environment.yml`.

## Environment setup

```bash
mamba env create -f environment.yml
# or, for an existing environment:
mamba env update -f environment.yml --prune
```

All versions are pinned in `environment.yml` (exact solve: `environment.lock.yml`). Backend Python
packages are managed by `uv` in `apps/backend` only.

## Running the pipeline

The workflow is under `packages/pipeline/workflow/Snakefile`.

Quickstart (`CORES` and `MEM_MB` are Make variables, see the Makefile):

```bash
make env-create       # once
make check            # unit tests (+ tiling check when t2t tracks exist)
make pipeline-chrm    # small development run (hg38 chrM, smoke test only)
make pipeline-t2t     # full T2T-CHM13v2.0 run; long, hecate peaks near 7.4 GB RAM
```

Reference downloads are verified against the sha256 values in `packages/pipeline/config.yaml`
(`reference:`); a mismatch stops the run and keeps no output file. Equivalent direct calls:

```bash
mamba run -n genome-guides snakemake \
  -s packages/pipeline/workflow/Snakefile \
  --cores 8 \
  --resources mem_mb=8192 \
  chrm_all

mamba run -n genome-guides snakemake \
  -s packages/pipeline/workflow/Snakefile \
  --cores 8 \
  --resources mem_mb=8192 \
  t2t_all
```

Use `--dry-run` before a large execution. The 8192 MB resource budget is
intentional: the Hecate rule declares the full budget, so Snakemake runs it
alone instead of overlapping it with other analytical jobs. Increase the
budget only when the host has enough free memory and the Hecate rule's
declared resource is increased accordingly.

## Resource and memory policy

Every Snakemake rule declares `threads`, `resources: mem_mb`, and a log path
with separate stdout and stderr files. Intermediate files are marked with
`temp()` where Snakemake owns their lifecycle.

Genome-scale processing uses one or more of:

- FASTA chunking with boundary carry-over for k-mers and repeat detection.
- Bounded Arrow/Parquet batches for tabular tracks.
- DuckDB queries for sorted/windowed operations.
- Temporary SQLite tables for transcript, exon, UTR, and CDS relationships.

Directly executed scripts accept `--threads` and `--memory-mb`. If
`--memory-mb` is omitted, a conservative value is derived from available
system memory. Window and chunk settings are configurable in
`packages/pipeline/config.yaml`; changing a window changes the output's
biological resolution.

## Backend and frontend

Run the backend from its directory:

```bash
cd apps/backend
uv sync
uv run uvicorn app.main:app --reload --port 8000
```

Run the frontend from the repository root:

```bash
pnpm install
pnpm --filter web dev
```

The frontend expects the backend API configuration defined in the web
application's environment and API client files.

## Validation

Run everything that needs no long run:

```bash
make check
```

Run only the backend tests:

```bash
cd apps/backend
uv run --group dev pytest -q
```

Before committing pipeline changes, also run:

```bash
python -m compileall -q packages/pipeline/scripts apps/backend/app
git diff --check
mamba run -n genome-guides snakemake \
  -s packages/pipeline/workflow/Snakefile \
  --cores 1 \
  --resources mem_mb=8192 \
  --dry-run
```

## Error handling

Missing input files, missing required columns, invalid schemas, and invalid
resource values fail with explicit messages. Malformed annotation records that
can be skipped are counted and reported to stderr.

## Contribution conventions

- Keep orchestration in Snakemake rules and analysis in standalone scripts.
- Use streaming, chunking, or indexed storage for genome-scale data.
- Add concise comments only for biological thresholds, coordinate systems, or
  non-obvious memory behavior.
- Make small commits with one logical change each.
- Do not commit generated data, local environments, credentials, or secrets.
