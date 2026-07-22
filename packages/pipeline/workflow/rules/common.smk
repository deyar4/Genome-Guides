# Genome Guides - Common Snakemake Helper Functions

import os
from pathlib import Path

def get_genome_fasta(wildcards):
    """Return path to input reference FASTA."""
    return f"data/raw/{wildcards.genome}.fa"

def get_contigs_from_fai(fai_path):
    """Extract list of contig names from .fai index file."""
    if not os.path.exists(fai_path):
        return []
    contigs = []
    with open(fai_path, "r") as f:
        for line in f:
            if line.strip():
                contigs.append(line.split("\t")[0])
    return contigs
