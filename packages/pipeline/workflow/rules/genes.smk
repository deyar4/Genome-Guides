# Genome Guides - Genes & Annotation Rules

rule parse_gene_annotations:
    input:
        gtf="data/raw/{genome}.gtf.gz"
    output:
        gene_models="data/tracks/{genome}/all/gene_models.parquet",
        gene_density="data/tracks/{genome}/all/gene_density.parquet",
        biotype_census="data/tracks/{genome}/all/gene_biotype_census.parquet",
        transcripts="data/tracks/{genome}/all/transcripts.parquet",
        exons="data/tracks/{genome}/all/exons.parquet",
        introns="data/tracks/{genome}/all/introns.parquet",
        utrs="data/tracks/{genome}/all/utrs.parquet",
        architecture="data/tracks/{genome}/all/gene_architecture.parquet"
    log:
        "logs/{genome}/parse_gene_annotations.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/genes/extract_annotation_tracks.py \
            -i {input.gtf} \
            -o data/tracks/{wildcards.genome}/all
        """

rule compute_gene_spacing:
    input:
        gene_models="data/tracks/{genome}/all/gene_models.parquet"
    output:
        gene_spacing="data/tracks/{genome}/all/gene_spacing.parquet",
        gene_overlaps="data/tracks/{genome}/all/gene_overlaps.parquet"
    log:
        "logs/{genome}/compute_gene_spacing.log"
    threads: 1
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/genes/extract_gene_spacing.py \
            -i {input.gene_models} \
            -o data/tracks/{wildcards.genome}/all
        """

rule parse_splice_and_ncrna:
    input:
        gtf="data/raw/{genome}.gtf.gz",
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        ncrna_census="data/tracks/{genome}/all/ncrna_detailed_census.parquet",
        gene_isoforms="data/tracks/{genome}/all/gene_isoform_counts.parquet",
        splice_sites="data/tracks/{genome}/all/splice_sites.parquet",
        splice_consensus="data/tracks/{genome}/all/splice_site_consensus.parquet",
        rna_clusters="data/tracks/{genome}/all/rna_clusters.parquet",
        rna_cluster_summary="data/tracks/{genome}/all/rna_cluster_summary.parquet"
    log:
        "logs/{genome}/parse_splice_and_ncrna.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/rna/extract_rna_tracks.py \
            -g {input.gtf} \
            -f {input.fasta} \
            -o data/tracks/{wildcards.genome}/all
        """

rule convert_t2t_centromeres:
    input:
        bed="data/raw/t2t_centromeres.bed"
    output:
        parquet="data/tracks/t2t/all/centromeres.parquet"
    params:
        source_url="https://s3-us-west-2.amazonaws.com/human-pangenomics/T2T/CHM13/assemblies/annotation/chm13v2.0_GenomeFeature_v1.0.bed"
    log:
        "logs/t2t/convert_t2t_centromeres.log"
    threads: 1
    resources:
        mem_mb=config["resources"]["lightweight"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/dna/bed_features_to_parquet.py \
            -i {input.bed} -o {output.parquet} --source-url {params.source_url} --feature CEN
        """

rule convert_t2t_telomeres:
    input:
        bed="data/raw/t2t_telomeres.bed"
    output:
        parquet="data/tracks/t2t/all/telomeres.parquet"
    params:
        source_url="https://s3-us-west-2.amazonaws.com/human-pangenomics/T2T/CHM13/assemblies/annotation/chm13v2.0_telomere.bed"
    log:
        "logs/t2t/convert_t2t_telomeres.log"
    threads: 1
    resources:
        mem_mb=config["resources"]["lightweight"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/dna/bed_features_to_parquet.py \
            -i {input.bed} -o {output.parquet} --source-url {params.source_url}
        """

rule compute_repeatmasker_te:
    input:
        rmsk="data/raw/{genome}_repeatmasker.out.gz"
    output:
        te_families="data/tracks/{genome}/all/repeatmasker_families.parquet",
        te_density="data/tracks/{genome}/all/repeatmasker_density.parquet"
    log:
        "logs/{genome}/compute_repeatmasker_te.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/genes/extract_repeatmasker.py \
            -i {input.rmsk} \
            -o data/tracks/{wildcards.genome}/all
        """
