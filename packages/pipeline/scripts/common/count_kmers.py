import sys
import argparse
import re
from collections import Counter
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from resource_args import add_resource_args

def count_kmers(fasta_file, k, output_file, is_dinucleotide=False):
    if k < 1:
        raise ValueError("k must be at least 1")

    valid_pattern = re.compile(r'[^ACGT]')
    
    kmer_counts = Counter()
    single_counts = Counter()
    total_kmers = 0
    total_singles = 0

    def consume_block(block, carry, final=False):
        nonlocal total_kmers, total_singles
        if is_dinucleotide:
            single_counts.update(block)
            total_singles += len(block)

        combined = carry + block
        if final:
            start = 0
            next_carry = ""
        else:
            keep = min(k - 1, len(combined))
            next_carry = combined[-keep:] if keep else ""
            start = max(0, len(carry) - k + 1)

        if len(combined) - start >= k:
            process = combined[start:]
            kmer_counts.update(
                process[i:i + k] for i in range(len(process) - k + 1)
            )
            total_kmers += len(process) - k + 1
        return next_carry

    # Keep at most k-1 bases between chunks so k-mers spanning FASTA lines survive
    # without materializing an entire chromosome.
    with open(fasta_file, "r") as handle:
        carry = ""
        for line in handle:
            if line.startswith(">"):
                if carry:
                    carry = consume_block("", carry, final=True)
                continue

            blocks = valid_pattern.split(line.strip().upper())
            for index, block in enumerate(blocks):
                if index and carry:
                    consume_block("", carry, final=True)
                    carry = ""
                carry = consume_block(block, carry)

        if carry:
            consume_block("", carry, final=True)

    with open(output_file, 'w') as f:
        for kmer, count in kmer_counts.most_common():
            pct = (count / total_kmers) * 100.0 if total_kmers > 0 else 0.0
            if is_dinucleotide:
                f_din = count / total_kmers
                b1, b2 = kmer[0], kmer[1]
                f_b1 = single_counts[b1] / total_singles if total_singles > 0 else 0
                f_b2 = single_counts[b2] / total_singles if total_singles > 0 else 0
                odds = (f_din / (f_b1 * f_b2)) if (f_b1 * f_b2) > 0 else 0.0
                f.write(f"{kmer}\t{count}\t{pct:.6f}\t{odds:.6f}\n")
            else:
                f.write(f"{kmer}\t{count}\t{pct:.6f}\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-i", "--input", required=True)
    parser.add_argument("-o", "--output", required=True)
    parser.add_argument("-k", type=int, required=True)
    parser.add_argument("--dinucleotide", action="store_true")
    add_resource_args(parser)
    args = parser.parse_args()
    count_kmers(args.input, args.k, args.output, args.dinucleotide)
