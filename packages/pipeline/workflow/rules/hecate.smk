# Genome Guides - Non-Trivial Research Rules (Hecate-BWT Basepair Probabilities)

rule hecate_build_bwt:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        bwt_idx="data/hecate_index/{genome}.hecate"
    threads: 16
    shell:
        """
        hecate-bwt index -i {input.fasta} -o {output.bwt_idx} --threads {threads}
        """

rule hecate_compute_probabilities:
    input:
        bwt_idx="data/hecate_index/{genome}.hecate",
        fasta="data/raw/{genome}.fa"
    output:
        prob_tsv="data/interim/{genome}/hecate_probabilities.tsv"
    threads: 16
    shell:
        """
        hecate-bwt query-prob --index {input.bwt_idx} --window 100 \
            --threads {threads} > {output.prob_tsv}
        """

rule convert_hecate_to_parquet:
    input:
        prob_tsv="data/interim/{genome}/hecate_probabilities.tsv"
    output:
        parquet="data/tracks/{genome}/all/hecate_bwt.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/common/tsv_to_parquet.py \
            -i {input.prob_tsv} \
            -o {output.parquet} \
            -s hecate_bwt
        """
