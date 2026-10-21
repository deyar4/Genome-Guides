# Genome Guides - Automatic Reference Data Download Rules (T2T-CHM13, hg38, chrM)

rule download_chrm_fasta:
    output:
        fasta="data/raw/chrm.fa"
    params:
        url=config["reference"]["chrm_fasta"]["url"],
        sha256=config["reference"]["chrm_fasta"]["sha256"],
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
        python3 packages/pipeline/scripts/common/fetch_verified.py --url {params.url} --sha256 {params.sha256} --output {output} --gunzip
        """

rule download_t2t_fasta:
    output:
        fasta="data/raw/t2t.fa"
    params:
        url=config["reference"]["t2t_fasta"]["url"],
        sha256=config["reference"]["t2t_fasta"]["sha256"],
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
        python3 packages/pipeline/scripts/common/fetch_verified.py --url {params.url} --sha256 {params.sha256} --output {output} --gunzip
        """

rule download_t2t_gff:
    output:
        gff="data/raw/t2t.gtf.gz"
    params:
        url=config["reference"]["t2t_gtf"]["url"],
        sha256=config["reference"]["t2t_gtf"]["sha256"],
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
        python3 packages/pipeline/scripts/common/fetch_verified.py --url {params.url} --sha256 {params.sha256} --output {output}
        """

rule download_t2t_centromeres:
    output:
        bed="data/raw/t2t_centromeres.bed"
    params:
        url=config["reference"]["t2t_centromeres"]["url"],
        sha256=config["reference"]["t2t_centromeres"]["sha256"],
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
        python3 packages/pipeline/scripts/common/fetch_verified.py --url {params.url} --sha256 {params.sha256} --output {output}
        """

rule download_t2t_telomeres:
    output:
        bed="data/raw/t2t_telomeres.bed"
    params:
        url=config["reference"]["t2t_telomeres"]["url"],
        sha256=config["reference"]["t2t_telomeres"]["sha256"],
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
        python3 packages/pipeline/scripts/common/fetch_verified.py --url {params.url} --sha256 {params.sha256} --output {output}
        """

rule download_t2t_protein:
    output:
        faa="data/raw/t2t_protein.faa.gz"
    params:
        url=config["reference"]["t2t_protein"]["url"],
        sha256=config["reference"]["t2t_protein"]["sha256"],
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
        python3 packages/pipeline/scripts/common/fetch_verified.py --url {params.url} --sha256 {params.sha256} --output {output}
        """

rule download_t2t_repeatmasker:
    output:
        rmsk="data/raw/t2t_repeatmasker.out.gz"
    params:
        url=config["reference"]["t2t_repeatmasker"]["url"],
        sha256=config["reference"]["t2t_repeatmasker"]["sha256"],
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
        python3 packages/pipeline/scripts/common/fetch_verified.py --url {params.url} --sha256 {params.sha256} --output {output}
        """
