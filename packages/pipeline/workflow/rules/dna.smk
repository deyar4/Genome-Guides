# Genome Guides - DNA Module Rules (Pure CLI implementations)

rule compute_cpg_islands:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        parquet="data/tracks/{genome}/all/cpg_islands.parquet"
    shell:
        """
        mkdir -p data/tracks/{wildcards.genome}/all data/interim/{wildcards.genome}
        bedtools makewindows -g {input.fai} -w 200 -s 200 | sort -k1,1 -k2,2n > data/interim/{wildcards.genome}/cpg_windows.bed
        bedtools nuc -fi {input.fasta} -bed data/interim/{wildcards.genome}/cpg_windows.bed | grep -v "^#" > data/interim/{wildcards.genome}/cpg_nuc.bed
        seqkit locate -i -P -p "CG" --bed {input.fasta} 2>/dev/null | sort -k1,1 -k2,2n > data/interim/{wildcards.genome}/cpg_locs.bed
        if [ ! -s data/interim/{wildcards.genome}/cpg_locs.bed ]; then
            awk 'BEGIN {{OFS="\\t"}} {{print $0, 0}}' data/interim/{wildcards.genome}/cpg_nuc.bed
        else
            bedtools map -a data/interim/{wildcards.genome}/cpg_nuc.bed -b data/interim/{wildcards.genome}/cpg_locs.bed -c 4 -o count -null 0
        fi > data/interim/{wildcards.genome}/cpg_mapped.bed
        awk 'BEGIN {{OFS="\\t"}} {{
            gc_pct = $5 * 100.0;
            expected = ($7 * $8) / $12;
            ratio = (expected > 0) ? ($13 / expected) : 0.0;
            is_island = (gc_pct >= 50.0 && ratio >= 0.60 && $13 >= 4.0) ? 1 : 0;
            if (is_island == 1) print $1, $2, $3, gc_pct, ratio, $13, is_island
        }}' data/interim/{wildcards.genome}/cpg_mapped.bed > data/interim/{wildcards.genome}/cpg_islands.tsv
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i data/interim/{wildcards.genome}/cpg_islands.tsv -o {output.parquet} -t cpg_islands
        rm -f data/interim/{wildcards.genome}/cpg_windows.bed data/interim/{wildcards.genome}/cpg_nuc.bed data/interim/{wildcards.genome}/cpg_locs.bed data/interim/{wildcards.genome}/cpg_mapped.bed data/interim/{wildcards.genome}/cpg_islands.tsv
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
        window_size=config["tracks"]["ssr"]["window_size"]
    shell:
        """
        python3 packages/pipeline/scripts/dna/extract_ssrs.py \
            -f {input.fasta} -o data/tracks/{wildcards.genome}/all -w {params.window_size}
        """

rule compute_isochores:
    input:
        gc="data/tracks/{genome}/all/gc_content.parquet"
    output:
        parquet="data/tracks/{genome}/all/isochores.parquet"
    shell:
        """
        python3 packages/pipeline/scripts/dna/extract_isochores.py \
            -i {input.gc} -o {output.parquet}
        """

rule compute_kmers_3mer:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/kmers_3mer.parquet"
    shell:
        """
        mkdir -p data/tracks/{wildcards.genome}/all
        python3 packages/pipeline/scripts/common/count_kmers.py -i {input.fasta} -o data/tracks/{wildcards.genome}/all/kmers_3mer.tsv -k 3
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i data/tracks/{wildcards.genome}/all/kmers_3mer.tsv -o {output.parquet} -t kmers_3mer
        rm -f data/tracks/{wildcards.genome}/all/kmers_3mer.tsv
        """

rule compute_kmers_6mer:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/kmers_6mer.parquet"
    shell:
        """
        mkdir -p data/tracks/{wildcards.genome}/all
        python3 packages/pipeline/scripts/common/count_kmers.py -i {input.fasta} -o data/tracks/{wildcards.genome}/all/kmers_6mer.tsv -k 6
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i data/tracks/{wildcards.genome}/all/kmers_6mer.tsv -o {output.parquet} -t kmers_6mer
        rm -f data/tracks/{wildcards.genome}/all/kmers_6mer.tsv
        """

rule compute_shannon_entropy:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        parquet="data/tracks/{genome}/all/shannon_entropy.parquet"
    shell:
        """
        mkdir -p data/tracks/{wildcards.genome}/all data/interim/{wildcards.genome}
        bedtools makewindows -g {input.fai} -w 1000 -s 1000 | sort -k1,1 -k2,2n > data/interim/{wildcards.genome}/entropy_windows.bed
        bedtools nuc -fi {input.fasta} -bed data/interim/{wildcards.genome}/entropy_windows.bed | grep -v "^#" > data/interim/{wildcards.genome}/entropy_nuc.bed
        seqkit locate -i -r -p "TTAGGG|CCCTAA" --bed {input.fasta} 2>/dev/null | sort -k1,1 -k2,2n > data/interim/{wildcards.genome}/entropy_locs.bed
        if [ ! -s data/interim/{wildcards.genome}/entropy_locs.bed ]; then
            awk 'BEGIN {{OFS="\\t"}} {{print $0, 0}}' data/interim/{wildcards.genome}/entropy_nuc.bed
        else
            bedtools map -a data/interim/{wildcards.genome}/entropy_nuc.bed -b data/interim/{wildcards.genome}/entropy_locs.bed -c 4 -o count -null 0
        fi > data/interim/{wildcards.genome}/entropy_mapped.bed
        awk 'BEGIN {{OFS="\\t"; log2=log(2)}} {{
            na=$6; nc=$7; ng=$8; nt=$9;
            tot = na + nc + ng + nt;
            entropy = 0;
            if (tot > 0) {{
                pa = na / tot; if (pa > 0) entropy -= pa * log(pa)/log2;
                pc = nc / tot; if (pc > 0) entropy -= pc * log(pc)/log2;
                pg = ng / tot; if (pg > 0) entropy -= pg * log(pg)/log2;
                pt = nt / tot; if (pt > 0) entropy -= pt * log(pt)/log2;
            }}
            print $1, $2, $3, entropy, $13
        }}' data/interim/{wildcards.genome}/entropy_mapped.bed > data/interim/{wildcards.genome}/entropy.tsv
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i data/interim/{wildcards.genome}/entropy.tsv -o {output.parquet} -t shannon_entropy
        rm -f data/interim/{wildcards.genome}/entropy_windows.bed data/interim/{wildcards.genome}/entropy_nuc.bed data/interim/{wildcards.genome}/entropy_locs.bed data/interim/{wildcards.genome}/entropy_mapped.bed data/interim/{wildcards.genome}/entropy.tsv
        """

rule compute_dinucleotides:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        parquet="data/tracks/{genome}/all/dinucleotide_frequencies.parquet"
    shell:
        """
        mkdir -p data/tracks/{wildcards.genome}/all data/interim/{wildcards.genome}
        python3 packages/pipeline/scripts/common/count_kmers.py -i {input.fasta} -o data/interim/{wildcards.genome}/dinucleotides.tsv -k 2 --dinucleotide
        python3 packages/pipeline/scripts/common/duckdb_to_parquet.py -i data/interim/{wildcards.genome}/dinucleotides.tsv -o {output.parquet} -t dinucleotides
        rm -f data/interim/{wildcards.genome}/dinucleotides.tsv
        """

rule compute_genomic_homopolymers:
    input:
        fasta="data/raw/{genome}.fa"
    output:
        bed="data/interim/{genome}/homopolymers.bed"
    shell:
        """
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
    shell:
        """
        python3 packages/pipeline/scripts/common/tsv_to_parquet.py \
            -i {input.bed} \
            -o {output.parquet} \
            -s homopolymers
        """

rule compute_hecate_mprob:
    input:
        fasta="data/raw/{genome}.fa",
        fai="data/raw/{genome}.fa.fai"
    output:
        parquet="data/tracks/{genome}/all/hecate_mprob.parquet"
    shell:
        """
        mkdir -p data/tracks/{wildcards.genome}/all data/interim/{wildcards.genome}
        rm -f data/interim/{wildcards.genome}/temp.he data/interim/{wildcards.genome}/mprob.xz
        packages/hecate/target/release/hecate --force -e -M markov --mprob data/interim/{wildcards.genome}/mprob.xz {input.fasta} -o data/interim/{wildcards.genome}/temp.he
        python3 packages/pipeline/scripts/common/process_hecate_mprob.py -f {input.fai} -i data/interim/{wildcards.genome}/mprob.xz -o {output.parquet} -w 1000
        rm -f data/interim/{wildcards.genome}/mprob.xz data/interim/{wildcards.genome}/temp.he
        """
