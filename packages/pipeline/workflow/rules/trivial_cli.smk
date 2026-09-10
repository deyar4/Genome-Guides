# Genome Guides - Fast CLI Trivial Extraction Rules (seqkit, samtools, bedtools)

rule samtools_faidx:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        fai="data/raw/{genome}.fa.fai"
    shell:
        """
        samtools faidx {input.fasta}
        """

rule compute_gc_and_skew:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        tsv="data/interim/{genome}/gc_skew.tsv"
    params:
        window=1000,
        step=1000
    shell:
        """
        seqkit sliding -g -W {params.window} -s {params.step} {input.fasta} | \
        seqkit fx2tab -n -g -G -C A,C,G,T,N -B A,C,G,T,N -H > {output.tsv}
        """

rule extract_assembly_gaps:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        bed="data/interim/{genome}/assembly_gaps.bed"
    shell:
        """
        seqkit locate -i -r -P -p '"N+"' --bed {input.fasta} > {output.bed}
        """

rule convert_gc_skew_to_parquet:
    input:
        tsv="data/interim/{genome}/gc_skew.tsv"
    output:
        parquet="data/tracks/{genome}/all/gc_content.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/common/tsv_to_parquet.py \
            -i {input.tsv} \
            -o {output.parquet} \
            -s gc_content
        """

rule convert_gaps_to_parquet:
    input:
        bed="data/interim/{genome}/assembly_gaps.bed"
    output:
        parquet="data/tracks/{genome}/all/assembly_gaps.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/common/tsv_to_parquet.py \
            -i {input.bed} \
            -o {output.parquet} \
            -s assembly_gaps
        """

rule seqkit_stats:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        stats="data/tracks/{genome}/all/sequence_stats.txt"
    shell:
        """
        seqkit stats -a -T {input.fasta} > {output.stats}
        """

