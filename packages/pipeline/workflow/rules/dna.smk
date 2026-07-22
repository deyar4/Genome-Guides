# Genome Guides - DNA Module Rules

rule compute_cpg_islands:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        parquet="data/tracks/{genome}/all/cpg_islands.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/dna/extract_cpg_islands.py \
            -f {input.fasta} \
            -o data/tracks/{wildcards.genome}/all \
            -w 200 -s 200
        """

rule compute_kmers_3mer:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/kmers_3mer.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/dna/extract_kmers.py \
            -f {input.fasta} \
            -o data/tracks/{wildcards.genome}/all \
            -k 3
        """

rule compute_kmers_6mer:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/kmers_6mer.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/dna/extract_kmers.py \
            -f {input.fasta} \
            -o data/tracks/{wildcards.genome}/all \
            -k 6
        """

rule compute_shannon_entropy:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/shannon_entropy.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/dna/extract_shannon_entropy.py \
            -f {input.fasta} \
            -o data/tracks/{wildcards.genome}/all \
            -w 1000 -s 1000
        """

rule compute_dinucleotides:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/dinucleotide_frequencies.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/dna/extract_dinucleotides.py \
            -f {input.fasta} \
            -o data/tracks/{wildcards.genome}/all
        """

rule compute_genomic_homopolymers:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/homopolymer_genomic_tracts.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/dna/extract_homopolymers.py \
            -f {input.fasta} \
            -o data/tracks/{wildcards.genome}/all \
            -m 10
        """
