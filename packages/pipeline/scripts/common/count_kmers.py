import sys
import argparse
import re
from collections import Counter

def read_fasta(file_path):
    seq_id = ""
    seq = []
    with open(file_path, 'r') as f:
        for line in f:
            if line.startswith(">"):
                if seq_id:
                    yield seq_id, "".join(seq)
                seq_id = line[1:].strip()
                seq = []
            else:
                seq.append(line.strip().upper())
        if seq_id:
            yield seq_id, "".join(seq)

def count_kmers(fasta_file, k, output_file, is_dinucleotide=False):
    valid_pattern = re.compile(r'[^ACGT]')
    
    kmer_counts = Counter()
    single_counts = Counter()
    total_kmers = 0
    total_singles = 0
    
    for seq_id, seq in read_fasta(fasta_file):
        valid_blocks = valid_pattern.split(seq)
        for block in valid_blocks:
            block_len = len(block)
            if block_len < k:
                if is_dinucleotide:
                    # Even if block is too short for a kmer, it still has singles
                    single_counts.update(block)
                    total_singles += block_len
                continue
            
            if is_dinucleotide:
                single_counts.update(block)
                total_singles += block_len
                
            kmers = (block[i:i+k] for i in range(block_len - k + 1))
            kmer_counts.update(kmers)
            total_kmers += block_len - k + 1

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
    args = parser.parse_args()
    count_kmers(args.input, args.k, args.output, args.dinucleotide)
