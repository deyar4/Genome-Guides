# Genome Guides - Genes & Annotation Rules

rule parse_gene_annotations:
    input:
        gtf="data/raw/{genome}.gtf.gz"
    output:
        gene_models="data/tracks/{genome}/all/gene_models.parquet",
        gene_density="data/tracks/{genome}/all/gene_density.parquet",
        biotype_census="data/tracks/{genome}/all/gene_biotype_census.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/genes/extract_gene_models.py \
            -i {input.gtf} \
            -o data/tracks/{wildcards.genome}/all
        """

rule compute_gene_spacing:
    input:
        gene_models="data/tracks/{genome}/all/gene_models.parquet"
    output:
        gene_spacing="data/tracks/{genome}/all/gene_spacing.parquet",
        gene_overlaps="data/tracks/{genome}/all/gene_overlaps.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/genes/extract_gene_spacing.py \
            -i {input.gene_models} \
            -o data/tracks/{wildcards.genome}/all
        """

rule parse_splice_and_ncrna:
    input:
        gtf="data/raw/{genome}.gtf.gz"
    output:
        ncrna_census="data/tracks/{genome}/all/ncrna_detailed_census.parquet",
        gene_isoforms="data/tracks/{genome}/all/gene_isoform_counts.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/rna/extract_splice_and_ncrna.py \
            -g {input.gtf} \
            -o data/tracks/{wildcards.genome}/all
        """

rule compute_repeatmasker_te:
    input:
        rmsk="data/raw/{genome}_repeatmasker.out.gz"
    output:
        te_families="data/tracks/{genome}/all/repeatmasker_families.parquet",
        te_density="data/tracks/{genome}/all/repeatmasker_density.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/genes/extract_repeatmasker.py \
            -i {input.rmsk} \
            -o data/tracks/{wildcards.genome}/all
        """
