# Genome Guides - Protein Census Rules

rule parse_protein_census:
    input:
        faa="data/raw/{genome}_protein.faa.gz"
    output:
        aa_comp="data/tracks/{genome}/all/amino_acid_composition.parquet",
        homo_tracts="data/tracks/{genome}/all/protein_homopolymers.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/proteins/parse_proteins_to_parquet.py \
            -i {input.faa} \
            -o data/tracks/{wildcards.genome}/all
        """

rule compute_protein_terminal_bias:
    input:
        faa="data/raw/{genome}_protein.faa.gz"
    output:
        term_bias="data/tracks/{genome}/all/protein_terminal_bias.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/proteins/extract_protein_terminal_bias.py \
            -i {input.faa} \
            -o data/tracks/{wildcards.genome}/all
        """
