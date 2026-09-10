#!/usr/bin/env python3
import sys
import argparse
import numpy as np
import polars as pl

def process_mprob(fai_path, xz_path, out_path, window_size=1000):
    contigs = []
    with open(fai_path, 'r') as f:
        for line in f:
            parts = line.strip().split('\t')
            if parts:
                contigs.append((parts[0], int(parts[1])))
                
    schema = {
        "contig": pl.Utf8,
        "start_pos": pl.UInt32,
        "end_pos": pl.UInt32,
        "mean_prob": pl.Float32,
        "min_prob": pl.Float32,
        "max_prob": pl.Float32,
        "mean_entropy": pl.Float32
    }

    # Polars read_csv_batched does not support .xz files natively.
    # We use pandas C-engine to stream the .xz in chunks, then process the numpy arrays.
    import pandas as pd
    try:
        chunk_iterator = pd.read_csv(
            xz_path, 
            header=None, 
            names=['prob'], 
            chunksize=window_size * 5000,
            engine='c'
        )
    except Exception as e:
        print(f"Error opening batched reader via pandas: {e}")
        raise

    contig_idx = 0
    pos_in_contig = 0
    
    df_chunks = []
    
    for chunk in chunk_iterator:
        probs = chunk['prob'].to_numpy()
        
        # Vectorized entropy calculation
        with np.errstate(divide='ignore'):
            entropies = -np.log2(probs)
        entropies = np.clip(entropies, 0, 20.0)
        
        chunk_pos = 0
        chunk_len = len(probs)
        
        records = {
            "contig": [], "start_pos": [], "end_pos": [],
            "mean_prob": [], "min_prob": [], "max_prob": [], "mean_entropy": []
        }
        
        while chunk_pos < chunk_len:
            if contig_idx >= len(contigs):
                break
                
            current_contig, current_contig_len = contigs[contig_idx]
            remaining_in_contig = current_contig_len - pos_in_contig
            
            # Determine how much of the chunk belongs to the current contig
            take = min(chunk_len - chunk_pos, remaining_in_contig)
            
            p_slice = probs[chunk_pos:chunk_pos+take]
            e_slice = entropies[chunk_pos:chunk_pos+take]
            
            remainder = take % window_size
            pad = (window_size - remainder) if remainder else 0
            
            if pad:
                p_slice = np.pad(p_slice, (0, pad), constant_values=np.nan)
                e_slice = np.pad(e_slice, (0, pad), constant_values=np.nan)
                
            p_2d = p_slice.reshape(-1, window_size)
            e_2d = e_slice.reshape(-1, window_size)
            
            mean_p = np.nanmean(p_2d, axis=1)
            min_p = np.nanmin(p_2d, axis=1)
            max_p = np.nanmax(p_2d, axis=1)
            mean_e = np.nanmean(e_2d, axis=1)
            
            num_windows = len(mean_p)
            starts = pos_in_contig + np.arange(num_windows) * window_size
            ends = starts + window_size
            if remainder:
                ends[-1] = pos_in_contig + take
                
            records["contig"].extend([current_contig] * num_windows)
            records["start_pos"].extend(starts)
            records["end_pos"].extend(ends)
            records["mean_prob"].extend(mean_p)
            records["min_prob"].extend(min_p)
            records["max_prob"].extend(max_p)
            records["mean_entropy"].extend(mean_e)
            
            chunk_pos += take
            pos_in_contig += take
            
            if pos_in_contig >= current_contig_len:
                contig_idx += 1
                pos_in_contig = 0
                
        if records["contig"]:
            df_chunk = pl.DataFrame(records, schema=schema)
            df_chunks.append(df_chunk)

    if df_chunks:
        final_df = pl.concat(df_chunks)
        final_df.write_parquet(out_path, compression='zstd')
    else:
        pl.DataFrame(schema=schema).write_parquet(out_path, compression='zstd')

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-f", "--fai", required=True)
    parser.add_argument("-i", "--input", required=True)
    parser.add_argument("-o", "--output", required=True)
    parser.add_argument("-w", "--window", type=int, default=1000)
    args = parser.parse_args()
    process_mprob(args.fai, args.input, args.output, args.window)
