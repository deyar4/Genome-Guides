#!/usr/bin/env python3
"""
Genome Guides - Assembly Gap & Satellite Region Extractor
Locates continuous runs of Ns in reference genome FASTA files.
"""

import sys
import argparse
import re
from pathlib import Path

def parse_args():
    parser = argparse.ArgumentParser(description="Extract Assembly Gaps (N-runs).")
    parser.add_argument("--fasta", "-f", required=True, help="Input FASTA file")
    parser.add_argument("--out", "-o", required=True, help="Output TSV file")
    return parser.parse_args()

def extract_gaps(fasta_path: str, out_tsv: str):
    out_p = Path(out_tsv)
    out_p.parent.mkdir(parents=True, exist_ok=True)

    current_contig = None
    seq_chunks = []
    n_pattern = re.compile(r'N+', re.IGNORECASE)

    with open(fasta_path, "r") as fa_f, open(out_tsv, "w") as out_f:
        def scan_sequence(contig_name, sequence):
            for match in n_pattern.finditer(sequence):
                start = match.start()
                end = match.end()
                gap_length = end - start
                out_f.write(f"{contig_name}\t{start}\t{end}\t{gap_length}\tassembly_gap\n")

        for line in fa_f:
            line = line.strip()
            if not line:
                continue
            if line.startswith(">"):
                if current_contig and seq_chunks:
                    scan_sequence(current_contig, "".join(seq_chunks))
                    seq_chunks = []
                current_contig = line[1:].split()[0]
            else:
                seq_chunks.append(line)
        if current_contig and seq_chunks:
            scan_sequence(current_contig, "".join(seq_chunks))

if __name__ == "__main__":
    args = parse_args()
    extract_gaps(args.fasta, args.out)
