# Genome Guides - Automatic Reference Data Download Rules (T2T-CHM13, hg38, chrM)

rule download_chrm_fasta:
    output:
        fasta="data/raw/chrm.fa"
    params:
        url="https://hgdownload.soe.ucsc.edu/goldenPath/hg38/chromosomes/chrM.fa.gz"
    log:
        "logs/download_chrm_fasta.log"
    benchmark:
        "benchmarks/download_chrm_fasta.tsv"
    threads: config["resources"]["downloads"]["threads"]
    resources:
        mem_mb=config["resources"]["downloads"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/raw
        curl -sSL {params.url} | gunzip -c > {output.fasta}
        """

rule download_t2t_fasta:
    output:
        fasta="data/raw/t2t.fa"
    params:
        url="https://s3-us-west-2.amazonaws.com/human-pangenomics/T2T/CHM13/assemblies/analysis_set/chm13v2.0.fa.gz"
    log:
        "logs/t2t/download_t2t_fasta.log"
    benchmark:
        "benchmarks/download_t2t_fasta.tsv"
    threads: config["resources"]["downloads"]["threads"]
    resources:
        mem_mb=config["resources"]["downloads"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/raw
        curl -sSL {params.url} | gunzip -c > {output.fasta}
        """

rule download_t2t_gff:
    output:
        gff="data/raw/t2t.gtf.gz"
    params:
        url="https://hgdownload.soe.ucsc.edu/goldenPath/hs1/bigZips/genes/hs1.ncbiRefSeq.gtf.gz"
    log:
        "logs/t2t/download_t2t_gff.log"
    benchmark:
        "benchmarks/download_t2t_gff.tsv"
    threads: config["resources"]["downloads"]["threads"]
    resources:
        mem_mb=config["resources"]["downloads"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/raw
        curl -sSL {params.url} -o {output.gff}
        """

rule download_t2t_centromeres:
    output:
        bed="data/raw/t2t_centromeres.bed"
    params:
        url="https://s3-us-west-2.amazonaws.com/human-pangenomics/T2T/CHM13/assemblies/annotation/chm13v2.0_GenomeFeature_v1.0.bed"
    log:
        "logs/t2t/download_t2t_centromeres.log"
    benchmark:
        "benchmarks/download_t2t_centromeres.tsv"
    threads: config["resources"]["downloads"]["threads"]
    resources:
        mem_mb=config["resources"]["downloads"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/raw
        curl --fail --location --retry 3 -sS {params.url} -o {output.bed}
        """

rule download_t2t_telomeres:
    output:
        bed="data/raw/t2t_telomeres.bed"
    params:
        url="https://s3-us-west-2.amazonaws.com/human-pangenomics/T2T/CHM13/assemblies/annotation/chm13v2.0_telomere.bed"
    log:
        "logs/t2t/download_t2t_telomeres.log"
    benchmark:
        "benchmarks/download_t2t_telomeres.tsv"
    threads: config["resources"]["downloads"]["threads"]
    resources:
        mem_mb=config["resources"]["downloads"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/raw
        curl --fail --location --retry 3 -sS {params.url} -o {output.bed}
        """

rule download_t2t_protein:
    output:
        faa="data/raw/t2t_protein.faa.gz"
    params:
        url="https://ftp.ncbi.nlm.nih.gov/genomes/all/GCF/009/914/755/GCF_009914755.1_T2T-CHM13v2.0/GCF_009914755.1_T2T-CHM13v2.0_protein.faa.gz"
    log:
        "logs/t2t/download_t2t_protein.log"
    benchmark:
        "benchmarks/download_t2t_protein.tsv"
    threads: config["resources"]["downloads"]["threads"]
    resources:
        mem_mb=config["resources"]["downloads"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/raw
        curl -sSL {params.url} -o {output.faa}
        """

rule download_t2t_repeatmasker:
    output:
        rmsk="data/raw/t2t_repeatmasker.out.gz"
    params:
        url="https://hgdownload.soe.ucsc.edu/goldenPath/hs1/bigZips/hs1.repeatMasker.out.gz"
    log:
        "logs/t2t/download_t2t_repeatmasker.log"
    benchmark:
        "benchmarks/download_t2t_repeatmasker.tsv"
    threads: config["resources"]["downloads"]["threads"]
    resources:
        mem_mb=config["resources"]["downloads"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/raw
        curl -sSL {params.url} -o {output.rmsk}
        """
