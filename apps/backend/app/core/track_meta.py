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
                "weighted by n_acgt. A window has NULL entropy when n_acgt < 500 (D33). "
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
                "weighted by n_valid. Forward, prequential: depends on sequence order (audit 03, A1). "
                "NOT Shannon entropy of composition."
            ),
            "avg_mean_prob": "Mean of P(x_i | context) per window (probability, 0..1), weighted by n_valid. Not a surprisal.",
            "min_mean_entropy / max_mean_entropy": "Smallest and largest window value inside the bin.",
        },
    },
}
