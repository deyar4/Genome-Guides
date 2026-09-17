#!/usr/bin/env python3
import argparse
from pathlib import Path

import pyarrow.parquet as pq

def parquet_to_bedgraph(parquet_path, bedgraph_path, column="mean_entropy"):
    parquet = pq.ParquetFile(parquet_path)
    required = {"contig", "start_pos", "end_pos", column}
    missing = required.difference(parquet.schema_arrow.names)
    if missing:
        names = ", ".join(sorted(missing))
        raise SystemExit(f"Error: Parquet track is missing required columns: {names}")

    Path(bedgraph_path).parent.mkdir(parents=True, exist_ok=True)
    with open(bedgraph_path, "w") as output:
        output.write(
            f'track type=bedGraph name="{column}" description="Hecate-BWT '
            f'{column}" visibility=full color=200,100,0 altColor=0,100,200\n'
        )
        for batch in parquet.iter_batches(
            batch_size=65_536, columns=["contig", "start_pos", "end_pos", column]
        ):
            for contig, start, end, value in zip(
                batch.column(0), batch.column(1), batch.column(2), batch.column(3)
            ):
                output.write(
                    f"{contig.as_py()}\t{start.as_py()}\t{end.as_py()}\t"
                    f"{value.as_py()}\n"
                )
        
    print(f"Successfully wrote {column} to {bedgraph_path}")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Convert Hecate Parquet track to bedGraph")
    parser.add_argument("-i", "--input", required=True, help="Input parquet file")
    parser.add_argument("-o", "--output", required=True, help="Output bedGraph file")
    parser.add_argument("-c", "--column", default="mean_entropy", help="Column to export as track value")
    args = parser.parse_args()
    
    parquet_to_bedgraph(args.input, args.output, args.column)
