"""Definitions of the window tracks served by /tracks (D17: every track states definition, units, log base, window).

Only tracks whose definition was checked against the pipeline code are listed. Facts come from
docs/audit/03_science.md (A3) and docs/DECISIONS.md (D31, D33); nothing here is inferred from the data.
"""
from typing import Any, Dict

# SQL for exact aggregation of the gc_content track from per-window base counts (D31: coarser bins are exact aggregates).
# gc_pct = 100 * (G+C) / (A+C+G+T); gc_skew = 100 * (G-C) / (G+C). Both are percentages, ACGT only, N excluded.
GC_PCT_SQL = "100.0 * SUM(CAST(g_count AS BIGINT) + c_count) / NULLIF(SUM(CAST(a_count AS BIGINT) + c_count + g_count + t_count), 0)"
GC_SKEW_SQL = "100.0 * (SUM(CAST(g_count AS BIGINT)) - SUM(CAST(c_count AS BIGINT))) / NULLIF(SUM(CAST(g_count AS BIGINT) + c_count), 0)"

# Per-window weight used when averaging window values into a coarser bin (None = plain mean over non-NULL windows).
WEIGHT_COLUMN: Dict[str, str] = {
    "shannon_entropy": "n_acgt",
    "hecate_mprob": "n_valid",
}

# Columns that are counts: binned as a sum (sum_<col>) in addition to the per-window mean.
COUNT_COLUMNS = {"count", "te_count", "telomeric_repeats", "n_valid", "n_acgt"}

# Semantics shared by all binned responses (documented here because they are not column-specific).
BINNING_NOTES = [
    "A bin covers the windows that overlap the requested [start, end); n_windows says how many. The first and last bin of a "
    "non-aligned request can therefore hold fewer windows than bin_size/window.",
    "min_* / max_* are the smallest / largest value of a single base window (1 kb for the window tracks) inside the bin, not of a bin-level statistic.",
    "Interval tracks (window_bp = null) are not binned: the rows overlapping [start, end) are returned in start order, at most 5,000, and the response says truncated when the cap cut them.",
    "avg_min_* / avg_max_* (hecate_mprob) are weighted means of per-window minima / maxima of the per-base probability; "
    "they are not a bin minimum or maximum.",
    "n_null_<col> counts windows whose value is missing (NULL/NaN); they are excluded from the average, never counted as 0.",
]

TRACK_METADATA: Dict[str, Dict[str, Any]] = {
    "gc_content": {
        "window_bp": 1000,
        "columns": {
            "avg_value": "GC content in percent: 100*(G+C)/(A+C+G+T) over the bin, computed from summed counts (N excluded).",
            "avg_skew": "GC skew in percent: 100*(G-C)/(G+C) over the bin, computed from summed counts. NULL when G+C = 0.",
        },
    },
    "shannon_entropy": {
        "window_bp": 1000,
        "columns": {
            "avg_shannon_entropy": (
                "Mean of per-window Shannon entropy of the A/C/G/T composition, log base 2, bits (0..2), "
                "weighted by n_acgt. A window has NULL entropy when n_acgt < min_acgt_fraction * window_size (= 500 with the current config, D33). "
                "This is the entropy of base composition, not a model-based quantity."
            ),
        },
    },
    "hecate_mprob": {
        "window_bp": 1000,
        "columns": {
            "avg_mean_entropy": (
                "Cross-entropy rate in bits per base under the hecate adaptive order-mixture model: mean of "
                "-log2 P(x_i | preceding bases) over valid bases of the window, surprisal clipped at 20 bits, "
                "weighted by n_valid. Forward, prequential: depends on sequence order, and the model context runs across contig boundaries (audit 03, A1, S6). "
                "NOT Shannon entropy of composition."
            ),
            "avg_mean_prob": "Mean of P(x_i | context) per window (probability, 0..1), weighted by n_valid. Not a surprisal.",
            "min_mean_entropy / max_mean_entropy": "Smallest and largest per-window value inside the bin.",
        },
    },
    "cpg_islands": {
        "window_bp": 200,
        "columns": {
            "definition": (
                "Maximal runs of adjacent 200 bp tiled windows (grid starts at 0, partial last window and windows with N excluded) "
                "that each have GC > 50 % and obs/exp CpG > 0.6, kept iff the whole run again satisfies both and length >= 200. "
                "Grid-anchored: boundaries are only determined to the 200 bp grid. Spec docs/analyses/01_cpg_islands.md."
            ),
            "gc_pct": "100*(C+G)/length, percent over the island (N excluded by construction).",
            "obs_exp_ratio": "cpg_count*length/(C*G), dimensionless; cpg_count counts CG dinucleotides fully inside [start, end).",
            "strict": "1 iff length >= 500, gc_pct >= 55 and obs_exp_ratio >= 0.65 (inclusive bounds), evaluated on the merged island.",
        },
    },
    # --- 100 kb count tracks (sparse: windows without a record are absent, not zero) ---
    "gene_density": {
        "window_bp": 100000,
        "columns": {
            "definition": (
                "Genes per 100 kb window [k*100000, (k+1)*100000): each gene_models row is counted once, in the window "
                "that contains its 0-based start. A row is one (contig, gene_id): gene ids with several loci on one contig "
                "are one row (open question Q4). The window end is start + 100000 and is not clipped to the contig length. "
                "Windows without a gene are ABSENT from the track."
            ),
            "sum_count": "Exact number of genes in the bin (sum over its windows).",
            "avg_count": "Mean over the windows PRESENT in the bin only; empty windows are absent, so this overstates density where they occur. Use sum_count / (bin length / 100000) for a density.",
        },
        "source": "packages/pipeline/scripts/genes/extract_gene_models.py (window_size = 100000)",
    },
    "repeatmasker_density": {
        "window_bp": 100000,
        "columns": {
            "definition": (
                "RepeatMasker records per 100 kb window, counted in the window containing the record's 0-based start. "
                "Every line of the .out file is one record, so fragments of one element count separately. "
                "Windows without a record are ABSENT from the track; the window end is not clipped to the contig length."
            ),
            "sum_te_count": "Exact number of records in the bin.",
            "avg_te_count": "Mean over the windows PRESENT in the bin only (see gene_density).",
        },
        "source": "packages/pipeline/scripts/genes/extract_repeatmasker.py (window_size = 100000)",
    },
    # --- interval tracks: raw rows overlapping [start, end), not binned, at most INTERVAL_ROW_LIMIT rows (truncated flag) ---
    "gene_models": {
        "window_bp": None,
        "columns": {
            "definition": (
                "Interval track, one row per (contig, gene_id): the first GTF line of that gene on that contig among feature types "
                "gene, transcript, mRNA, lnc_RNA, ncRNA_gene. Several loci of one gene id on one contig appear as one row (Q4)."
            ),
            "start_pos / end_pos": "0-based half-open [start_pos, end_pos); converted once from the 1-based GTF start (start - 1).",
            "biotype": (
                "Attribute gene_biotype, else gene_type, else gbkey. If none exists: protein_coding when the transcript id starts with NM_ or XM_, "
                "otherwise non_coding (a pipeline inference from the id prefix, not an annotation; see Q5)."
            ),
        },
        "source": "packages/pipeline/scripts/genes/extract_gene_models.py",
    },
    "repeatmasker_families": {
        "window_bp": None,
        "columns": {
            "definition": "Interval track, one row per RepeatMasker .out record (alignment fragments are separate rows).",
            "start_pos / end_pos": "0-based half-open; converted once from the 1-based inclusive .out coordinates (start - 1, end).",
            "strand": "+ as given; C (complement) in the .out file is written as -.",
            "te_name / te_class": "Repeat name and class/family string exactly as in the .out file (columns 10 and 11).",
            "divergence_pct": "Percent divergence from the consensus as reported by RepeatMasker (column 2); not recomputed.",
        },
        "source": "packages/pipeline/scripts/genes/extract_repeatmasker.py",
    },
    "assembly_gaps": {
        "window_bp": None,
        "columns": {
            "definition": (
                "Interval track: maximal runs of N or n in the assembly FASTA (seqkit locate, case-insensitive regex N+). "
                "Other IUPAC codes are not counted as gaps."
            ),
            "start_pos / end_pos": "0-based half-open, as written by seqkit locate --bed.",
            "gap_length": "end_pos - start_pos, in bases.",
            "gap_type": "Constant 'assembly_gap'; the assembly does not classify gaps.",
        },
        "source": "packages/pipeline/workflow/rules/trivial_cli.smk (extract_assembly_gaps), scripts/common/tsv_to_parquet.py",
    },
    "homopolymer_genomic_tracts": {
        "window_bp": None,
        "columns": {
            "definition": (
                "Interval track: runs of at least 10 identical bases, found per base A, C, G, T with seqkit locate "
                "(case-insensitive, so soft-masked bases count) and merged with bedtools merge. N is not searched. "
                "Not independently recounted (UNVERIFIED)."
            ),
            "start_pos / end_pos": "0-based half-open.",
            "base": "A, C, G or T.",
            "run_length": "end_pos - start_pos, in bases (at least 10).",
        },
        "source": "packages/pipeline/workflow/rules/dna.smk (compute_genomic_homopolymers), scripts/common/tsv_to_parquet.py",
    },
}
