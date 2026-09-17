# Genome-Guides

## Pipeline resource policy

The Snakemake workflow uses conservative defaults suitable for a 16 GB
laptop while allowing larger machines to increase throughput. Override the
scheduler budget at runtime rather than changing analytical code:

```bash
mamba run -n genome-guides snakemake \
  -s packages/pipeline/workflow/Snakefile \
  --cores 12 --resources mem_mb=24000 t2t_all
```

Genome-scale converters use bounded batches. SSR and Hecate batch sizes are
configurable in `packages/pipeline/config.yaml`; increasing them trades memory
for throughput. Window sizes are analytical parameters, so changing them
changes the biological resolution of the output.

## Compliance checklist

- **Architecture:** Snakemake rules orchestrate external tools and analytical
  scripts; helper modules expose standard input/output and resource flags.
- **Resources:** Every workflow rule declares threads, `mem_mb`, and separate
  stdout/stderr logs. Runtime memory is detected for direct script use and can
  be overridden with `--memory-mb` or Snakemake resources.
- **Memory safety:** FASTA, TSV, Parquet, annotation, and RNA processing use
  bounded batches, row groups, or temporary SQLite spill tables. No complete
  chromosome or genome-wide matrix is materialized.
- **Errors:** Missing files, invalid schemas, invalid resource values, and
  malformed input records produce explicit failures or stderr warnings.
- **Style:** Comments state biological or computational rationale; module
  descriptions are concise and avoid generic template text.
- **Validation:** Use the smoke test, Snakemake dry-run, backend tests, Python
  compilation, and `git diff --check` before committing changes.