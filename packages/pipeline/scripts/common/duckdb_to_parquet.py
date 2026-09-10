#!/usr/bin/env python3
"""
Genome Guides - Convert TSV to Parquet using DuckDB SQL schemas
"""
import sys
import argparse
from pathlib import Path
import duckdb

QUERIES = {
    "cpg_islands": """
        COPY (
            SELECT 
                column0 AS contig, 
                column1::UBIGINT AS start_pos, 
                column2::UBIGINT AS end_pos, 
                column3::FLOAT AS gc_pct, 
                column4::FLOAT AS obs_exp_ratio, 
                column5::UBIGINT AS cpg_count, 
                column6::UTINYINT AS is_cpg_island 
            FROM read_csv('{input_tsv}', sep='\t', header=false)
        ) TO '{output_parquet}' (FORMAT 'PARQUET', COMPRESSION 'ZSTD')
    """,
    "kmers_3mer": """
        COPY (
            SELECT 
                column0 AS kmer, 
                column1::UBIGINT AS count, 
                column2::FLOAT AS frequency_pct 
            FROM read_csv('{input_tsv}', sep='\t', header=false)
        ) TO '{output_parquet}' (FORMAT 'PARQUET', COMPRESSION 'ZSTD')
    """,
    "kmers_6mer": """
        COPY (
            SELECT 
                column0 AS kmer, 
                column1::UBIGINT AS count, 
                column2::FLOAT AS frequency_pct 
            FROM read_csv('{input_tsv}', sep='\t', header=false)
        ) TO '{output_parquet}' (FORMAT 'PARQUET', COMPRESSION 'ZSTD')
    """,
    "shannon_entropy": """
        COPY (
            SELECT 
                column0 AS contig, 
                column1::UBIGINT AS start_pos, 
                column2::UBIGINT AS end_pos, 
                column3::FLOAT AS shannon_entropy, 
                column4::UBIGINT AS telomeric_repeats 
            FROM read_csv('{input_tsv}', sep='\t', header=false)
        ) TO '{output_parquet}' (FORMAT 'PARQUET', COMPRESSION 'ZSTD')
    """,
    "dinucleotides": """
        COPY (
            SELECT 
                column0 AS dinucleotide, 
                column1::UBIGINT AS count, 
                column2::FLOAT AS frequency_pct, 
                column3::FLOAT AS odds_ratio 
            FROM read_csv('{input_tsv}', sep='\t', header=false)
        ) TO '{output_parquet}' (FORMAT 'PARQUET', COMPRESSION 'ZSTD')
    """
}

def main():
    parser = argparse.ArgumentParser(description="Convert TSV to Parquet using DuckDB")
    parser.add_argument("-i", "--input", required=True, help="Input TSV file")
    parser.add_argument("-o", "--output", required=True, help="Output Parquet file")
    parser.add_argument("-t", "--track", required=True, choices=list(QUERIES.keys()), help="Track type")
    args = parser.parse_args()

    input_path = Path(args.input)
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    if not input_path.exists() or input_path.stat().st_size == 0:
        empty_queries = {
            "cpg_islands": """
                COPY (
                    SELECT CAST(NULL AS VARCHAR) AS contig,
                           CAST(NULL AS UBIGINT) AS start_pos,
                           CAST(NULL AS UBIGINT) AS end_pos,
                           CAST(NULL AS FLOAT) AS gc_pct,
                           CAST(NULL AS FLOAT) AS obs_exp_ratio,
                           CAST(NULL AS UBIGINT) AS cpg_count,
                           CAST(NULL AS UTINYINT) AS is_cpg_island
                    WHERE FALSE
                ) TO '{output_parquet}' (FORMAT 'PARQUET', COMPRESSION 'ZSTD')
            """
        }
        if args.track in empty_queries:
            duckdb.sql(empty_queries[args.track].format(output_parquet=str(output_path)))
            return

    query = QUERIES[args.track].format(input_tsv=str(input_path), output_parquet=str(output_path))
    duckdb.sql(query)

if __name__ == "__main__":
    main()
