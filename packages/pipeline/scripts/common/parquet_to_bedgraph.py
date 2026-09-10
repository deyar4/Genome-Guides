#!/usr/bin/env python3
import sys
import argparse
import polars as pl

def parquet_to_bedgraph(parquet_path, bedgraph_path, column="mean_entropy"):
    """
    Converts a track parquet file (with contig, start_pos, end_pos) 
    into a UCSC bedGraph format for a specific column.
    """
    # Use scan_parquet for lazy evaluation in polars, then collect what we need
    # We could also use read_parquet.
    df = pl.read_parquet(parquet_path)
    
    if column not in df.columns:
        print(f"Error: Column {column} not found in parquet file.", file=sys.stderr)
        sys.exit(1)
        
    # bedGraph format: chrom start end value
    bg_df = df.select(['contig', 'start_pos', 'end_pos', column])
    
    with open(bedgraph_path, 'w') as f:
        f.write(f'track type=bedGraph name="{column}" description="Hecate-BWT {column}" visibility=full color=200,100,0 altColor=0,100,200\n')
        
    # Polars write_csv with append doesn't exist directly via standard write, 
    # but we can write to another file or just open the file in append mode.
    # We can write the dataframe to a temp bytes buffer and then append, or use write_csv with a file object.
    with open(bedgraph_path, 'a') as f:
        bg_df.write_csv(f, separator='\t', include_header=False)
        
    print(f"Successfully wrote {column} to {bedgraph_path}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Convert Hecate Parquet track to bedGraph")
    parser.add_argument("-i", "--input", required=True, help="Input parquet file")
    parser.add_argument("-o", "--output", required=True, help="Output bedGraph file")
    parser.add_argument("-c", "--column", default="mean_entropy", help="Column to export as track value")
    args = parser.parse_args()
    
    parquet_to_bedgraph(args.input, args.output, args.column)
