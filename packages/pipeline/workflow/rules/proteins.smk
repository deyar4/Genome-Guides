# Genome Guides - Protein Census Rules

rule parse_protein_census:
    input:
        faa="data/raw/{genome}_protein.faa.gz"
    output:
        aa_comp="data/tracks/{genome}/all/amino_acid_composition.parquet",
        homo_tracts="data/tracks/{genome}/all/protein_homopolymers.parquet",
        lengths="data/tracks/{genome}/all/protein_lengths.parquet",
        richness="data/tracks/{genome}/all/protein_richness.parquet"
    log:
        "logs/{genome}/parse_protein_census.log"
    benchmark:
        "benchmarks/{genome}/parse_protein_census.tsv"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/proteins/parse_proteins_to_parquet.py \
            -i {input.faa} \
            -o data/tracks/{wildcards.genome}/all
        """

rule compute_protein_terminal_bias:
    input:
        faa="data/raw/{genome}_protein.faa.gz"
    output:
        term_bias="data/tracks/{genome}/all/protein_terminal_bias.parquet"
    log:
        "logs/{genome}/compute_protein_terminal_bias.log"
    benchmark:
        "benchmarks/{genome}/compute_protein_terminal_bias.tsv"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/proteins/extract_protein_terminal_bias.py \
            -i {input.faa} \
            -o data/tracks/{wildcards.genome}/all
        """
