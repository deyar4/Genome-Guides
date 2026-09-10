"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { genesApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { ResponsiveScatterPlot } from '@nivo/scatterplot';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { Activity, Network } from 'lucide-react';
import { formatNumber } from '@/lib/utils';

export default function GeneCorrelationsPage() {
  const { activeGenome } = useGenomeStore();
  const chartTheme = useChartTheme();

  const { data: exonSizeData, isLoading: exonSizeLoading } = useQuery({
    queryKey: ['correlation-exon-size', activeGenome],
    queryFn: () => genesApi.getCorrelationExonSize(activeGenome),
  });

  const { data: densitySizeData, isLoading: densitySizeLoading } = useQuery({
    queryKey: ['correlation-density-size', activeGenome],
    queryFn: () => genesApi.getCorrelationDensitySize(activeGenome),
  });

  const { data: utrMrnaData, isLoading: utrMrnaLoading } = useQuery({
    queryKey: ['correlation-utr-mrna', activeGenome],
    queryFn: () => genesApi.getCorrelationUtrMrna(activeGenome),
  });

  const formatScatterData = (id: string, sample: number[][] | undefined) => {
    if (!sample) return [];
    return [{
      id,
      data: sample.map(p => ({ x: p[0], y: p[1] }))
    }];
  };

  const exonSizeScatter = useMemo(() => formatScatterData('Exon Count vs Gene Length', exonSizeData?.sample), [exonSizeData]);
  const densitySizeScatter = useMemo(() => formatScatterData('Gene Density vs Avg Gene Length', densitySizeData?.sample), [densitySizeData]);
  const utr5Scatter = useMemo(() => formatScatterData('5\' UTR vs mRNA', utrMrnaData?.utr_5prime_correlation?.sample), [utrMrnaData]);
  const utr3Scatter = useMemo(() => formatScatterData('3\' UTR vs mRNA', utrMrnaData?.utr_3prime_correlation?.sample), [utrMrnaData]);

  return (
    <LibraryShell 
      title="Genomic Correlations" 
      subtitle="Statistical analysis of structural relationships across the genome."
    >
      <div className="space-y-12 pb-20">
        
        {/* HERO METRICS */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <GenomicMetricCard 
            label="Exon vs Gene Size (r)"
            value={exonSizeData?.r_value?.toFixed(3) || '...'}
            description="Correlation between total exons and overall gene length."
            color="#3b82f6"
          />
          <GenomicMetricCard 
            label="Density vs Size (r)"
            value={densitySizeData?.r_value?.toFixed(3) || '...'}
            description="Correlation between gene density per Mb and average gene size."
            color="#10b981"
          />
          <GenomicMetricCard 
            label="5' UTR vs mRNA (r)"
            value={utrMrnaData?.utr_5prime_correlation?.r_value?.toFixed(3) || '...'}
            description="Correlation between 5' UTR length and total mRNA exonic length."
            color="#f59e0b"
          />
          <GenomicMetricCard 
            label="3' UTR vs mRNA (r)"
            value={utrMrnaData?.utr_3prime_correlation?.r_value?.toFixed(3) || '...'}
            description="Correlation between 3' UTR length and total mRNA exonic length."
            color="#ef4444"
          />
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
           {/* EXON VS GENE SIZE SCATTER */}
           <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm h-[500px] flex flex-col">
              <div className="flex items-center justify-between mb-6">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                   <Activity className="h-4 w-4 text-blue-500" />
                   Exon Count vs. Gene Length
                 </h3>
                 <span className="text-[10px] text-slate-400 font-medium">N = {exonSizeData?.n ? formatNumber(exonSizeData.n) : '...'}</span>
              </div>
              <div className="flex-1 min-h-0">
                 {exonSizeLoading ? (
                    <div className="h-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                    </div>
                 ) : exonSizeScatter.length > 0 && exonSizeScatter[0].data.length > 0 ? (
                    <ResponsiveScatterPlot
                      data={exonSizeScatter}
                      margin={{ top: 20, right: 20, bottom: 60, left: 80 }}
                      xScale={{ type: 'linear', min: 0, max: 'auto' }}
                      xFormat=">-.2f"
                      yScale={{ type: 'linear', min: 0, max: 'auto' }}
                      yFormat=">-.2f"
                      blendMode="multiply"
                      colors={['#3b82f6']}
                      theme={chartTheme}
                      axisBottom={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: 'Total Exon Count',
                        legendPosition: 'middle',
                        legendOffset: 40
                      }}
                      axisLeft={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: 'Gene Length (bp)',
                        legendPosition: 'middle',
                        legendOffset: -60,
                        format: d => formatNumber(d as number)
                      }}
                      nodeSize={6}
                      tooltip={({ node }) => (
                         <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-sm text-xs">
                            <span className="font-bold text-slate-900 dark:text-zinc-100">Exons:</span> {formatNumber(node.data.x as number)}<br/>
                            <span className="font-bold text-slate-900 dark:text-zinc-100">Length:</span> {formatNumber(node.data.y as number)} bp
                         </div>
                      )}
                    />
                 ) : (
                    <div className="h-full flex items-center justify-center text-slate-400 text-sm">No data available</div>
                 )}
              </div>
           </Card>

           {/* DENSITY VS SIZE SCATTER */}
           <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm h-[500px] flex flex-col">
              <div className="flex items-center justify-between mb-6">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                   <Activity className="h-4 w-4 text-emerald-500" />
                   Density vs. Avg Gene Size
                 </h3>
                 <span className="text-[10px] text-slate-400 font-medium">N = {densitySizeData?.n ? formatNumber(densitySizeData.n) : '...'}</span>
              </div>
              <div className="flex-1 min-h-0">
                 {densitySizeLoading ? (
                    <div className="h-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                    </div>
                 ) : densitySizeScatter.length > 0 && densitySizeScatter[0].data.length > 0 ? (
                    <ResponsiveScatterPlot
                      data={densitySizeScatter}
                      margin={{ top: 20, right: 20, bottom: 60, left: 80 }}
                      xScale={{ type: 'linear', min: 0, max: 'auto' }}
                      xFormat=">-.2f"
                      yScale={{ type: 'linear', min: 0, max: 'auto' }}
                      yFormat=">-.2f"
                      blendMode="multiply"
                      colors={['#10b981']}
                      theme={chartTheme}
                      axisBottom={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: 'Gene Count',
                        legendPosition: 'middle',
                        legendOffset: 40
                      }}
                      axisLeft={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: 'Avg Gene Size (bp)',
                        legendPosition: 'middle',
                        legendOffset: -60,
                        format: d => formatNumber(d as number)
                      }}
                      nodeSize={6}
                      tooltip={({ node }) => (
                         <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-sm text-xs">
                            <span className="font-bold text-slate-900 dark:text-zinc-100">Count:</span> {formatNumber(node.data.x as number)}<br/>
                            <span className="font-bold text-slate-900 dark:text-zinc-100">Avg Size:</span> {formatNumber(node.data.y as number)} bp
                         </div>
                      )}
                    />
                 ) : (
                    <div className="h-full flex items-center justify-center text-slate-400 text-sm">No data available</div>
                 )}
              </div>
           </Card>

           {/* 5' UTR VS MRNA SCATTER */}
           <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm h-[500px] flex flex-col">
              <div className="flex items-center justify-between mb-6">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                   <Network className="h-4 w-4 text-orange-500" />
                   5&apos; UTR vs mRNA Length
                 </h3>
                 <span className="text-[10px] text-slate-400 font-medium">N = {utrMrnaData?.utr_5prime_correlation?.n ? formatNumber(utrMrnaData.utr_5prime_correlation.n) : '...'}</span>
              </div>
              <div className="flex-1 min-h-0">
                 {utrMrnaLoading ? (
                    <div className="h-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-orange-500" />
                    </div>
                 ) : utr5Scatter.length > 0 && utr5Scatter[0].data.length > 0 ? (
                    <ResponsiveScatterPlot
                      data={utr5Scatter}
                      margin={{ top: 20, right: 20, bottom: 60, left: 80 }}
                      xScale={{ type: 'linear', min: 0, max: 'auto' }}
                      xFormat=">-.2f"
                      yScale={{ type: 'linear', min: 0, max: 'auto' }}
                      yFormat=">-.2f"
                      blendMode="multiply"
                      colors={['#f59e0b']}
                      theme={chartTheme}
                      axisBottom={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: '5&apos; UTR Length (bp)',
                        legendPosition: 'middle',
                        legendOffset: 40
                      }}
                      axisLeft={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: 'mRNA Length (bp)',
                        legendPosition: 'middle',
                        legendOffset: -60,
                        format: d => formatNumber(d as number)
                      }}
                      nodeSize={6}
                      tooltip={({ node }) => (
                         <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-sm text-xs">
                            <span className="font-bold text-slate-900 dark:text-zinc-100">5&apos; UTR:</span> {formatNumber(node.data.x as number)} bp<br/>
                            <span className="font-bold text-slate-900 dark:text-zinc-100">mRNA:</span> {formatNumber(node.data.y as number)} bp
                         </div>
                      )}
                    />
                 ) : (
                    <div className="h-full flex items-center justify-center text-slate-400 text-sm">No data available</div>
                 )}
              </div>
           </Card>

           {/* 3' UTR VS MRNA SCATTER */}
           <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm h-[500px] flex flex-col">
              <div className="flex items-center justify-between mb-6">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                   <Network className="h-4 w-4 text-emerald-500" />
                   3&apos; UTR vs mRNA Length
                 </h3>
                 <span className="text-[10px] text-slate-400 font-medium">N = {utrMrnaData?.utr_3prime_correlation?.n ? formatNumber(utrMrnaData.utr_3prime_correlation.n) : '...'}</span>
              </div>
              <div className="flex-1 min-h-0">
                 {utrMrnaLoading ? (
                    <div className="h-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-emerald-500" />
                    </div>
                 ) : utr3Scatter.length > 0 && utr3Scatter[0].data.length > 0 ? (
                    <ResponsiveScatterPlot
                      data={utr3Scatter}
                      margin={{ top: 20, right: 20, bottom: 60, left: 80 }}
                      xScale={{ type: 'linear', min: 0, max: 'auto' }}
                      xFormat=">-.2f"
                      yScale={{ type: 'linear', min: 0, max: 'auto' }}
                      yFormat=">-.2f"
                      blendMode="multiply"
                      colors={['#10b981']}
                      theme={chartTheme}
                      axisBottom={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: '3&apos; UTR Length (bp)',
                        legendPosition: 'middle',
                        legendOffset: 40
                      }}
                      axisLeft={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: 'mRNA Length (bp)',
                        legendPosition: 'middle',
                        legendOffset: -60,
                        format: d => formatNumber(d as number)
                      }}
                      nodeSize={6}
                      tooltip={({ node }) => (
                         <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-sm text-xs">
                            <span className="font-bold text-slate-900 dark:text-zinc-100">3&apos; UTR:</span> {formatNumber(node.data.x as number)} bp<br/>
                            <span className="font-bold text-slate-900 dark:text-zinc-100">mRNA:</span> {formatNumber(node.data.y as number)} bp
                         </div>
                      )}
                    />
                 ) : (
                    <div className="h-full flex items-center justify-center text-slate-400 text-sm">No data available</div>
                 )}
              </div>
           </Card>
        </div>

      </div>
    </LibraryShell>
  );
}
