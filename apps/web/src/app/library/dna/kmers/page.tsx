"use client"

import React, { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { dnaApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { ResponsiveBar } from '@nivo/bar';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { SectionHeader } from '@/components/library/dna/shared';
import { Database, Activity } from 'lucide-react';
import { formatNumber } from '@/lib/utils';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from "@/components/ui/select";

export default function KmerExplorerPage() {
  const { activeGenome } = useGenomeStore();
  // The backend serves genome-wide k-mer counts only (chrom/start/end are ignored) and k in {3, 6}.
  const [kmerSize, setKmerSize] = useState<number>(6); // Default to hexamers
  const chartTheme = useChartTheme();

  const { data: kmerData, isLoading } = useQuery({
    queryKey: ['kmers', activeGenome, kmerSize],
    queryFn: () => dnaApi.getKmers(activeGenome, 'genome', { 
      k: kmerSize, 
      top_n: 20,
    }),
  });

  return (
    <LibraryShell 
      title="K-Mer Explorer" 
      subtitle="Counts of overlapping k-mers (k = 3, 6) over the whole assembly."
    >
      <div className="space-y-12 pb-20">
        
        {/* FILTERS */}
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
           <h4 className="font-bold text-slate-900 dark:text-zinc-100 text-sm uppercase tracking-widest flex items-center gap-2">
             <Database className="h-4 w-4 text-primary" />
             Dataset Explorer
           </h4>
           <div className="flex flex-wrap gap-4 items-center">
              <div className="flex flex-col gap-1.5">
                <span className="text-[10px] font-bold text-slate-400 uppercase ml-1">K-mer size</span>
                <Select value={kmerSize.toString()} onValueChange={(v) => setKmerSize(parseInt(v))}>
                  <SelectTrigger className="w-40 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800">
                      <SelectValue placeholder="K-Mer Size" />
                  </SelectTrigger>
                  <SelectContent>
                      <SelectItem value="3">Trimers (k=3)</SelectItem>
                      <SelectItem value="6">Hexamers (k=6)</SelectItem>
                  </SelectContent>
                </Select>
              </div>
           </div>
        </div>

        <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-100 dark:border-blue-800 p-4 rounded-lg flex items-start gap-3">
          <Activity className="h-5 w-5 text-blue-500 mt-0.5" />
          <div className="text-sm text-blue-800 dark:text-blue-300">
            <p className="font-bold mb-1">Genome-wide counts</p>
            <p>K-mer counts are precomputed over the whole assembly for k = 3 and k = 6 (all positions, overlapping k-mers, ACGT only). Per-chromosome or per-region counts are not available.</p>
          </div>
        </div>

        {/* METRICS */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
           <GenomicMetricCard 
             label="Total K-Mers Scanned"
             value={kmerData?.total_kmers ? formatNumber(kmerData.total_kmers) : '...'}
             description={`Total overlapping ${kmerSize}-mers counted genome-wide.`}
             color="#3b82f6"
           />
           <GenomicMetricCard 
             label="Unique Motifs Found"
             value={kmerData?.unique_kmers ? formatNumber(kmerData.unique_kmers) : '...'}
             description={`Distinct ${kmerSize}-mers observed (theoretical max: 4^${kmerSize} = ${formatNumber(4 ** kmerSize)}).`}
             color="#8b5cf6"
           />
        </div>

        {/* VISUALIZATION */}
        <section>
          <SectionHeader 
            title={`Top 20 Most Frequent ${kmerSize}-mers`}
            subtitle="Highest-count k-mers in the assembly, sorted by count."
          />
          <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm h-[500px] flex flex-col mt-6">
             <div className="flex-1 min-h-0">
                {isLoading ? (
                   <div className="h-full flex items-center justify-center">
                      <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                   </div>
                ) : kmerData && kmerData.kmers.length > 0 ? (
                   <ResponsiveBar
                     data={kmerData.kmers.map(k => ({ kmer: k.kmer, frequency: k.frequency }))}
                     keys={['frequency']}
                     indexBy="kmer"
                     margin={{ top: 20, right: 20, bottom: 60, left: 60 }}
                     padding={0.3}
                     colors={['#3b82f6']}
                     theme={chartTheme}
                     axisBottom={{
                       tickSize: 5,
                       tickPadding: 15,
                       tickRotation: 45,
                       legend: 'Sequence Motif',
                       legendPosition: 'middle',
                       legendOffset: 50
                     }}
                     axisLeft={{
                       tickSize: 5,
                       tickPadding: 5,
                       tickRotation: 0,
                       legend: 'Share of all k-mers (%)',
                       legendPosition: 'middle',
                       legendOffset: -50,
                       format: d => `${Number(d).toFixed(2)}%`
                     }}
                     enableLabel={false}
                     tooltip={({ value, indexValue }) => (
                        <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-sm">
                           <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-1">{indexValue}</div>
                           <div className="text-sm font-bold text-slate-900 dark:text-zinc-100">
                             {Number(value).toFixed(4)}%
                           </div>
                           <div className="text-[10px] text-slate-500 mt-1 font-mono">
                             Count: {formatNumber(kmerData.kmers.find(k => k.kmer === indexValue)?.count || 0)}
                           </div>
                        </div>
                     )}
                   />
                ) : (
                   <div className="h-full flex items-center justify-center text-slate-400 text-sm">No data available</div>
                )}
             </div>
          </Card>
        </section>

      </div>
    </LibraryShell>
  );
}
