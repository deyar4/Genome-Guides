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