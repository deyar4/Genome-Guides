"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { proteinsApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { formatNumber } from '@/lib/utils';
import { Card } from '@/components/ui/card';
import { ResponsiveBar } from '@nivo/bar';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { Activity, Target, Zap, Microscope } from 'lucide-react';

export default function ProteinsOverview() {
  const { activeGenome } = useGenomeStore();
  const chartTheme = useChartTheme();

  const { data: stats } = useQuery({
    queryKey: ['protein-stats', activeGenome],
    queryFn: () => proteinsApi.getSizeStats(activeGenome),
  });

  const { data: composition, isLoading: compLoading } = useQuery({
    queryKey: ['protein-composition-global', activeGenome],
    queryFn: () => proteinsApi.getComposition(activeGenome),
  });

  const compositionData = useMemo(() => {
    if (!composition) return [];
    return composition
      .map(c => ({
        aa: c.amino_acid,
        percentage: c.percentage,
      }))
      .sort((a, b) => b.percentage - a.percentage);
  }, [composition]);

  return (
    <LibraryShell 
      title="Protein Census" 
      subtitle="Comprehensive analysis of the proteome: from global composition to structural motifs and length distributions."
    >
      <div className="space-y-12 pb-20">
        
        {/* METRICS */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <GenomicMetricCard 
            label="Total Proteins"
            value={stats?.total_proteins ? formatNumber(stats.total_proteins) : '...'}
            description="Total number of annotated protein-coding transcripts. This represents the diversity of the primary proteome before post-translational modifications."
            color="#3b82f6"
          />
          <GenomicMetricCard 
            label="Avg Length"
            value={stats?.avg_length ? stats.avg_length.toFixed(0) : '...'}
            unit="aa"
            description="The arithmetic mean of protein sequence lengths. Most human proteins fall within a range suitable for stable folding into three-dimensional domains."
            color="#10b981"
          />
          <GenomicMetricCard 
            label="Median Length"
            value={stats?.median_length ? stats.median_length.toFixed(0) : '...'}
            unit="aa"
            description="The 50th percentile of protein size. This value is typically more representative of 'typical' proteins as it is less influenced by massive outliers."
            color="#f59e0b"
          />
          <GenomicMetricCard 
            label="Largest Protein"
            value={stats?.max_length ? formatNumber(stats.max_length) : '...'}
            unit="aa"
            description="The longest polypeptide chain found in the proteome. In humans, this is Titin, which provides elastic support in muscle sarcomeres."
            color="#ef4444"
          />
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
           {/* AA COMPOSITION BAR CHART */}
           <Card className="lg:col-span-2 p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm h-[500px] flex flex-col">
              <div className="flex items-center justify-between mb-8">
                <div>
                  <h3 className="text-lg font-bold text-slate-900 dark:text-zinc-100 flex items-center gap-2">
                    <Activity className="h-5 w-5 text-blue-500" />
                    Global Amino Acid Composition
                  </h3>
                  <p className="text-xs text-slate-500 dark:text-zinc-400 mt-1">Relative frequency of residues across the full human proteome.</p>
                </div>
              </div>
              
              <div className="flex-1 min-h-0">
                {compLoading ? (
                  <div className="h-full w-full flex items-center justify-center">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                  </div>
                ) : (
                  <ResponsiveBar
                    data={compositionData}
                    keys={['percentage']}
                    indexBy="aa"
                    margin={{ top: 20, right: 30, bottom: 50, left: 60 }}
                    padding={0.3}
                    colors={['#3b82f6']}
                    borderRadius={4}
                    theme={chartTheme}
                    axisBottom={{
                      tickSize: 5,
                      tickPadding: 5,
                      tickRotation: 0,
                      legend: 'Amino Acid',
                      legendPosition: 'middle',
                      legendOffset: 40
                    }}
                    axisLeft={{
                      tickSize: 5,
                      tickPadding: 5,
                      tickRotation: 0,
                      legend: 'Frequency (%)',
                      legendPosition: 'middle',
                      legendOffset: -50,
                      format: '.1f'
                    }}
                    enableLabel={false}
                  />
                )}
              </div>
           </Card>

           {/* PROTEOMIC NARRATIVE */}
           <div className="space-y-6">
              <Card className="p-8 bg-primary text-primary-foreground border-none shadow-xl relative overflow-hidden group">
                 <div className="relative z-10">
                    <h4 className="font-bold text-xs uppercase tracking-widest text-primary mb-4">Biological Insight</h4>
                    <p className="text-sm text-slate-300 leading-relaxed mb-6">
                       The human proteome is characterized by high levels of Leucine (L) and Serine (S), reflecting the chemical constraints of protein folding and cellular function.
                    </p>
                    <ul className="space-y-3">
                       <li className="flex items-start gap-3 text-xs text-slate-400">
                          <div className="h-1.5 w-1.5 rounded-full bg-primary mt-1.5" />
                          Average protein is ~515 aa
                       </li>
                       <li className="flex items-start gap-3 text-xs text-slate-400">
                          <div className="h-1.5 w-1.5 rounded-full bg-emerald-400 mt-1.5" />
                          Titin is the outlier at 34,350 aa
                       </li>
                       <li className="flex items-start gap-3 text-xs text-slate-400">
                          <div className="h-1.5 w-1.5 rounded-full bg-blue-400 mt-1.5" />
                          MT proteins use a distinct code
                       </li>
                    </ul>
                 </div>
                 <Zap className="absolute -right-12 -bottom-12 w-40 h-40 text-primary-foreground/10 group-hover:text-primary-foreground/20 transition-colors pointer-events-none" />
              </Card>

              <Card className="p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm">
                 <h4 className="font-bold text-slate-900 dark:text-zinc-100 text-sm mb-6 flex items-center gap-2">
                    <Microscope className="h-4 w-4 text-emerald-500" />
                    Module Navigation
                 </h4>
                 <div className="space-y-6">
                    <a href="/library/proteins/composition" className="block group">
                       <div className="flex items-center justify-between mb-1">
                          <span className="text-sm font-bold text-slate-700 dark:text-zinc-300 group-hover:text-primary transition-colors">AA Composition</span>
                          <Activity className="h-4 w-4 text-slate-300 group-hover:text-primary transition-colors" />
                       </div>
                       <p className="text-xs text-slate-400">Nuclear vs. Mitochondrial frequencies.</p>
                    </a>
                    <a href="/library/proteins/features" className="block group">
                       <div className="flex items-center justify-between mb-1">
                          <span className="text-sm font-bold text-slate-700 dark:text-zinc-300 group-hover:text-primary transition-colors">Protein Features</span>
                          <Target className="h-4 w-4 text-slate-300 group-hover:text-primary transition-colors" />
                       </div>
                       <p className="text-xs text-slate-400">Largest proteins and homopolymer tracts.</p>
                    </a>
                 </div>
              </Card>
           </div>
        </div>

      </div>
    </LibraryShell>
  );
}
