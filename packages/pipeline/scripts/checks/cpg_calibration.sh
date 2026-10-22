#!/usr/bin/env bash
# Reproduces the CpG calibration on UCSC hg38 chr21+chr22 (inputs pinned in config.yaml, section "calibration").
# Downloads about 182 MB (rmsk.txt.gz is 156 MB) into data/raw/calibration/, verifies sha256, runs the extractor with the
# default parameters and writes artifacts/checks/cpg_calibration_hg38_chr21_22.tsv. Needs the conda env (samtools, yaml).
set -euo pipefail
C=data/raw/calibration
O=artifacts/checks
mkdir -p "$C" "$O"
cfg() { python3 -c "import yaml,sys; print(yaml.safe_load(open('packages/pipeline/config.yaml'))['calibration'][sys.argv[1]][sys.argv[2]])" "$1" "$2"; }
fetch() {  # name, output file
  if [ ! -s "$C/$2" ]; then
    curl -sSf -o "$C/$2" "$(cfg "$1" url)"
  fi
  echo "$(cfg "$1" sha256)  $C/$2" | sha256sum -c --quiet -
}
fetch hg38_chr21 chr21.fa.gz
fetch hg38_chr22 chr22.fa.gz
fetch hg38_cpg_islands cpgIslandExt.txt.gz
fetch hg38_rmsk rmsk.txt.gz
gunzip -c "$C/chr21.fa.gz" "$C/chr22.fa.gz" > "$C/hg38_chr21_22.fa"
samtools faidx "$C/hg38_chr21_22.fa"
zcat "$C/rmsk.txt.gz" | awk -F'\t' '$6=="chr21"||$6=="chr22"' > "$C/rmsk_chr21_22.txt"
zcat "$C/cpgIslandExt.txt.gz" | awk -F'\t' '$2=="chr21"||$2=="chr22"' > "$C/cpgIslandExt_chr21_22.txt"
python3 packages/pipeline/scripts/dna/extract_cpg_islands.py --fasta "$C/hg38_chr21_22.fa" --fai "$C/hg38_chr21_22.fa.fai" \
  --out-parquet "$O/hg38_chr21_22_cpg.parquet" --out-summary "$O/hg38_chr21_22_cpg_summary.tsv" --out-meta "$O/hg38_chr21_22_cpg_meta.json"
python3 packages/pipeline/scripts/checks/cpg_calibration.py --islands "$O/hg38_chr21_22_cpg.parquet" --rmsk "$C/rmsk_chr21_22.txt" \
  --contigs chr21 chr22 --ucsc-cpg "$C/cpgIslandExt_chr21_22.txt" --out "$O/cpg_calibration_hg38_chr21_22.tsv"
