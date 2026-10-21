# Genome Guides - DNA Module Rules (Pure CLI implementations)

rule compute_cpg_islands:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        parquet="data/tracks/{genome}/all/cpg_islands.parquet",
        windows=temp("data/interim/{genome}/cpg_windows.bed"),
        nuc=temp("data/interim/{genome}/cpg_nuc.bed"),
        locs=temp("data/interim/{genome}/cpg_locs.bed"),
        mapped=temp("data/interim/{genome}/cpg_mapped.bed"),
        tsv=temp("data/interim/{genome}/cpg_islands.tsv")
    log:
        "logs/{genome}/compute_cpg_islands.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/tracks/{wildcards.genome}/all data/interim/{wildcards.genome}
        bedtools makewindows -g {input.fai} -w 200 -s 200 | sort -k1,1 -k2,2n > {output.windows}
        bedtools nuc -fi {input.fasta} -bed {output.windows} | grep -v "^#" > {output.nuc}
        seqkit locate -i -P -p "CG" --bed {input.fasta} 2>/dev/null | sort -k1,1 -k2,2n > {output.locs}
        if [ ! -s {output.locs} ]; then
            awk 'BEGIN {{OFS="\\t"}} {{print $0, 0}}' {output.nuc}
        else
            bedtools map -a {output.nuc} -b {output.locs} -c 4 -o count -null 0
        fi > {output.mapped}
        awk 'BEGIN {{OFS="\\t"}} {{
            gc_pct = $5 * 100.0;
            expected = ($7 * $8) / $12;
            ratio = (expected > 0) ? ($13 / expected) : 0.0;
            is_island = (gc_pct >= 50.0 && ratio >= 0.60 && $13 >= 4.0) ? 1 : 0;
            if (is_island == 1) print $1, $2, $3, gc_pct, ratio, $13, is_island
        }}' {output.mapped} > {output.tsv}
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i {output.tsv} -o {output.parquet} -t cpg_islands
        """

rule compute_ssrs:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        hits="data/tracks/{genome}/all/ssr_hits.parquet",
        summary="data/tracks/{genome}/all/ssr_summary.parquet",
        density="data/tracks/{genome}/all/ssr_density.parquet",
        enrichment="data/tracks/{genome}/all/ssr_enrichment.parquet"
    params:
        window_size=config["tracks"]["ssr"]["window_size"],
        chunk_bases=config["tracks"]["ssr"]["chunk_bases"]
    log:
        "logs/{genome}/compute_ssrs.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/dna/extract_ssrs.py \
            -f {input.fasta} -o data/tracks/{wildcards.genome}/all \
            -w {params.window_size} --chunk-bases {params.chunk_bases}
        """

rule compute_isochores:
    input:
        gc="data/tracks/{genome}/all/gc_content.parquet"
    output:
        parquet="data/tracks/{genome}/all/isochores.parquet"
    log:
        "logs/{genome}/compute_isochores.log"
    threads: 1
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/dna/extract_isochores.py \
            -i {input.gc} -o {output.parquet}
        """

rule compute_kmers_3mer:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/kmers_3mer.parquet",
        tsv=temp("data/tracks/{genome}/all/kmers_3mer.tsv")
    log:
        "logs/{genome}/compute_kmers_3mer.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/tracks/{wildcards.genome}/all
        python3 packages/pipeline/scripts/common/count_kmers.py -i {input.fasta} -o {output.tsv} -k 3
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i {output.tsv} -o {output.parquet} -t kmers_3mer
        """

rule compute_kmers_6mer:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/kmers_6mer.parquet",
        tsv=temp("data/tracks/{genome}/all/kmers_6mer.tsv")
    log:
        "logs/{genome}/compute_kmers_6mer.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/tracks/{wildcards.genome}/all
        python3 packages/pipeline/scripts/common/count_kmers.py -i {input.fasta} -o {output.tsv} -k 6
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i {output.tsv} -o {output.parquet} -t kmers_6mer
        """

rule compute_shannon_entropy:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        parquet="data/tracks/{genome}/all/shannon_entropy.parquet",
        windows=temp("data/interim/{genome}/entropy_windows.bed"),
        nuc=temp("data/interim/{genome}/entropy_nuc.bed"),
        locs=temp("data/interim/{genome}/entropy_locs.bed"),
        mapped=temp("data/interim/{genome}/entropy_mapped.bed"),
        tsv=temp("data/interim/{genome}/entropy.tsv")
    log:
        "logs/{genome}/compute_shannon_entropy.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    params:
        window=config["tracks"]["shannon_entropy"]["window_size"],
        min_acgt=int(config["tracks"]["shannon_entropy"]["min_acgt_fraction"] * config["tracks"]["shannon_entropy"]["window_size"])
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/tracks/{wildcards.genome}/all data/interim/{wildcards.genome}
        bedtools makewindows -g {input.fai} -w {params.window} -s {params.window} | sort -k1,1 -k2,2n > {output.windows}
        bedtools nuc -fi {input.fasta} -bed {output.windows} | grep -v "^#" > {output.nuc}
        seqkit locate -P -i -r -p "TTAGGG|CCCTAA" --bed {input.fasta} 2>/dev/null | sort -k1,1 -k2,2n > {output.locs}
        if [ ! -s {output.locs} ]; then
            awk 'BEGIN {{OFS="\\t"}} {{print $0, 0}}' {output.nuc}
        else
            bedtools map -a {output.nuc} -b {output.locs} -c 4 -o count -null 0
        fi > {output.mapped}
        awk -v min_acgt={params.min_acgt} 'BEGIN {{OFS="\\t"; log2=log(2)}} {{
            na=$6; nc=$7; ng=$8; nt=$9;
            tot = na + nc + ng + nt;
            entropy = "NA";
            if (tot >= min_acgt && tot > 0) {{
                entropy = 0;
                pa = na / tot; if (pa > 0) entropy -= pa * log(pa)/log2;
                pc = nc / tot; if (pc > 0) entropy -= pc * log(pc)/log2;
                pg = ng / tot; if (pg > 0) entropy -= pg * log(pg)/log2;
                pt = nt / tot; if (pt > 0) entropy -= pt * log(pt)/log2;
            }}
            print $1, $2, $3, entropy, $13, tot
        }}' {output.mapped} > {output.tsv}
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i {output.tsv} -o {output.parquet} -t shannon_entropy
        """

rule compute_dinucleotides:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/dinucleotide_frequencies.parquet",
        tsv=temp("data/interim/{genome}/dinucleotides.tsv")
    log:
        "logs/{genome}/compute_dinucleotides.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/tracks/{wildcards.genome}/all data/interim/{wildcards.genome}
        python3 packages/pipeline/scripts/common/count_kmers.py -i {input.fasta} -o {output.tsv} -k 2 --dinucleotide
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i {output.tsv} -o {output.parquet} -t dinucleotides
        """

rule compute_genomic_homopolymers:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        bed=temp("data/interim/{genome}/homopolymers.bed")
    log:
        "logs/{genome}/compute_genomic_homopolymers.log"
    threads: config["resources"]["analytical"]["threads"]
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/interim/{wildcards.genome}
        > {output.bed}
        for base in A C G T; do
            seqkit locate -i -r -P -p '"['$base']{{10,}}"' --bed {input.fasta} 2>/dev/null | \
            bedtools merge -i - | \
            awk -v b="$base" '{{print $1 "\\t" $2 "\\t" $3 "\\t" b "\\t" ($3-$2)}}' >> {output.bed}
        done
        """

rule convert_homopolymers_to_parquet:
    input:
        bed="data/interim/{genome}/homopolymers.bed"
    output:
        parquet="data/tracks/{genome}/all/homopolymer_genomic_tracts.parquet"
    log:
        "logs/{genome}/convert_homopolymers_to_parquet.log"
    threads: 1
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    shell:
        """
        exec > {log} 2>&1
        python3 packages/pipeline/scripts/common/tsv_to_parquet.py \
            -i {input.bed} \
            -o {output.parquet} \
            -s homopolymers
        """

rule build_hecate:
    output:
        binary=config["tracks"]["hecate_bwt"]["binary"]
    log:
        "logs/build_hecate.log"
    threads: 4
    resources:
        mem_mb=config["resources"]["analytical"]["mem_mb"]
    params:
        repo=config["tracks"]["hecate_bwt"]["repo"],
        commit=config["tracks"]["hecate_bwt"]["commit"]
    shell:
        """
        exec > {log} 2>&1
        packages/pipeline/scripts/common/build_hecate.sh {params.repo} {params.commit} {output.binary}
        """

rule check_hecate:
    input:
        binary=config["tracks"]["hecate_bwt"]["binary"]
    output:
        info="data/interim/hecate_binary.txt"
    params:
        commit=config["tracks"]["hecate_bwt"]["commit"],
        expected=[f"{k}={v}" for k, v in config["tracks"]["hecate_bwt"]["sha256"].items()],
        strict="--strict" if config["tracks"]["hecate_bwt"].get("strict_hash", False) else ""
    run:
        expected = " ".join(f"--expected {e}" for e in params.expected)
        shell("python3 packages/pipeline/scripts/common/check_hecate.py --binary {input.binary} "
              "--commit {params.commit} {expected} {params.strict} -o {output.info}")

rule compute_hecate_mprob:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai",
        hecate_info="data/interim/hecate_binary.txt"
    output:
        parquet="data/tracks/{genome}/all/hecate_mprob.parquet",
        he=temp("data/interim/{genome}/temp.he"),
        mprob=temp("data/interim/{genome}/mprob.xz"),
        nonacgt=temp("data/interim/{genome}/nonacgt.bed")
    log:
        "logs/{genome}/compute_hecate_mprob.log"
    threads: config["resources"]["hecate"]["threads"]
    resources:
        mem_mb=config["resources"]["hecate"]["mem_mb"]
    params:
        chunk_windows=config["tracks"]["hecate_bwt"]["chunk_windows"],
        hecate=config["tracks"]["hecate_bwt"]["binary"]
    shell:
        """
        exec > {log} 2>&1
        mkdir -p data/tracks/{wildcards.genome}/all data/interim/{wildcards.genome}
        seqkit locate -P -i -r -p "[^ACGT]+" --bed {input.fasta} 2>/dev/null | sort -k1,1 -k2,2n | bedtools merge -i - > {output.nonacgt}
        {params.hecate} --force -e -M markov --mprob {output.mprob} {input.fasta} -o {output.he}
        python3 packages/pipeline/scripts/common/process_hecate_mprob.py \
            -f {input.fai} -i {output.mprob} \
            -o {output.parquet} -w 1000 --chunk-windows {params.chunk_windows} \
            --nonacgt-bed {output.nonacgt}
        """
