"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { rnaApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { formatNumber, cn } from '@/lib/utils';
import { Card } from '@/components/ui/card';
import { ResponsivePie } from '@nivo/pie';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { Network, Wind, Layers, Shapes, Target, Activity } from 'lucide-react';

export default function RnaOverview() {
  const { activeGenome } = useGenomeStore();
  const chartTheme = useChartTheme();

  const { data: summary, isLoading } = useQuery({
    queryKey: ['rna-summary', activeGenome],
    queryFn: () => rnaApi.getSummary(activeGenome),
  });

  const pieData = useMemo(() => {
    if (!summary?.overall_stats?.biotype_counts) return [];
    return Object.entries(summary.overall_stats.biotype_counts)
      .map(([id, value]) => ({
        id,
        label: id.replace(/_/g, ' '),
        value,
      }))
      .sort((a, b) => b.value - a.value);
  }, [summary]);

  const stats = summary?.overall_stats;

  return (
    <LibraryShell 
      title="RNA Transcriptomics" 
      subtitle="Non-coding RNA census, gene clusters and splice-site probabilities."
    >
      <div className="space-y-12 pb-20">
        
        {/* METRICS */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <GenomicMetricCard 
            label="Non-coding RNA records"
            value={stats?.total_nc_rna ? formatNumber(stats.total_nc_rna) : '...'}
            description="Records in the ncRNA census (unit not documented by the API; larger than the 37,360 non-coding genes of the Biotype Census, so probably transcripts)."
            color="#ec4899"
          />
          <GenomicMetricCard 
            label="Gene Clusters"
            value={stats?.total_clusters ? formatNumber(stats.total_clusters) : '...'}
            description="Clusters of nearby genes, summed over all cluster types listed under Structural Hubs (including protein_coding)."
            color="#8b5cf6"
          />
          <GenomicMetricCard 
            label="RNA Types"
            value={pieData.length || '...'}
            description="Number of distinct non-coding subclasses in the census (see the distribution below)."
            color="#3b82f6"
          />
          <GenomicMetricCard 
            label="Largest Cluster"
            value={summary?.cluster_summaries?.length ? Math.max(...summary.cluster_summaries.map(c => c.max_cluster_size)) : '...'}
            description="Largest number of genes in a single cluster, over all cluster types."
            color="#10b981"
          />
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
          {/* BIOTYPE DISTRIBUTION PIE */}
          <Card className="lg:col-span-2 p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm overflow-hidden flex flex-col h-[500px]">
            <div className="flex items-center justify-between mb-8">
              <div>
                <h3 className="text-lg font-bold text-slate-900 dark:text-zinc-100 flex items-center gap-2">
                  <Layers className="h-5 w-5 text-pink-500" />
                  ncRNA Biotype Distribution
                </h3>
                <p className="text-xs text-slate-500 dark:text-zinc-400 mt-1">Global census of non-coding transcript varieties.</p>
              </div>
            </div>
            
            <div className="flex-1 min-h-0">
              {isLoading ? (
                <div className="h-full w-full flex items-center justify-center">
                  <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                </div>
              ) : (
                <ResponsivePie
                  data={pieData}
                  margin={{ top: 40, right: 80, bottom: 80, left: 80 }}
                  innerRadius={0.6}
                  padAngle={1}
                  cornerRadius={4}
                  activeOuterRadiusOffset={8}
                  colors={{ scheme: 'nivo' }}
                  borderWidth={1}
                  borderColor={{ from: 'color', modifiers: [['darker', 0.2]] }}
                  arcLinkLabelsSkipAngle={10}
                  arcLinkLabelsTextColor={chartTheme.text?.fill || '#333'}
                  arcLinkLabelsThickness={2}
                  arcLinkLabelsColor={{ from: 'color' }}
                  arcLabelsSkipAngle={10}
                  arcLabelsTextColor={{ from: 'color', modifiers: [['darker', 2]] }}
                  theme={chartTheme}
                  legends={[
                    {
                      anchor: 'bottom',
                      direction: 'row',
                      justify: false,
                      translateX: 0,
                      translateY: 56,
                      itemsSpacing: 0,
                      itemWidth: 100,
                      itemHeight: 18,
                      itemTextColor: chartTheme.text?.fill || '#999',
                      itemDirection: 'left-to-right',
                      itemOpacity: 1,
                      symbolSize: 12,
                      symbolShape: 'circle',
                    }
                  ]}
                />
              )}
            </div>
          </Card>

          {/* CLUSTER SUMMARY LIST */}
          <Card className="p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm">
            <h3 className="text-lg font-bold text-slate-900 dark:text-zinc-100 mb-6 flex items-center gap-2">
              <Network className="h-5 w-5 text-violet-500" />
              Structural Hubs
            </h3>
            <div className="space-y-6">
              {summary?.cluster_summaries.map((c) => (
                <div key={c.rna_type} className="group">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm font-bold text-slate-700 dark:text-zinc-300 uppercase tracking-tight">{c.rna_type}</span>
                    <span className="text-xs font-mono font-bold px-2 py-0.5 bg-slate-100 dark:bg-zinc-800 rounded text-slate-600 dark:text-zinc-400">
                      {c.num_clusters} clusters
                    </span>
                  </div>
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <span className="text-[10px] text-slate-400 uppercase font-bold tracking-widest">Avg Size</span>
                      <p className="text-sm font-bold text-slate-900 dark:text-zinc-100">{c.avg_cluster_size.toFixed(1)} genes</p>
                    </div>
                    <div>
                      <span className="text-[10px] text-slate-400 uppercase font-bold tracking-widest">Max Size</span>
                      <p className="text-sm font-bold text-slate-900 dark:text-zinc-100">{c.max_cluster_size} genes</p>
                    </div>
                  </div>
                  <div className="mt-2 p-2 bg-slate-50 dark:bg-zinc-900/50 rounded border border-slate-100 dark:border-zinc-800">
                    <span className="text-[9px] text-slate-400 uppercase font-bold">Largest Hub Location</span>
                    <p className="text-[10px] font-mono text-primary truncate">{c.largest_cluster_location}</p>
                  </div>
                </div>
              ))}
            </div>
          </Card>
        </div>

        {/* SECTION NAVIGATION */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
           {[
             { title: 'RNA Clusters', desc: 'Gene clusters by RNA type.', href: '/library/rna/clusters', icon: Shapes, color: 'text-blue-500' },
             { title: 'Nested Genes', desc: 'Host and guest gene pairs (not available yet).', href: '/library/rna/relationships', icon: Target, color: 'text-emerald-500' },
             { title: 'Splice Junctions', desc: 'Donor and acceptor site nucleotide probabilities.', href: '/library/rna/splice-sites', icon: Activity, color: 'text-amber-500' },
           ].map((section, i) => (
             <Card key={i} className="p-8 group hover:border-primary/50 transition-all cursor-pointer overflow-hidden relative">
               <div className={cn("h-12 w-12 rounded-2xl bg-slate-100 dark:bg-zinc-900 flex items-center justify-center mb-6 group-hover:scale-110 transition-transform")}>
                 <section.icon className={cn("h-6 w-6", section.color)} />
               </div>
               <h4 className="text-xl font-bold text-slate-900 dark:text-zinc-100 mb-2">{section.title}</h4>
               <p className="text-sm text-slate-500 dark:text-zinc-400 leading-relaxed mb-6">{section.desc}</p>
               <a href={section.href} className="text-xs font-bold text-primary flex items-center gap-2 group-hover:gap-3 transition-all">
                 Explore Module
                 <Wind className="h-3 w-3" />
               </a>
             </Card>
           ))}
        </div>

      </div>
    </LibraryShell>
  );
}
