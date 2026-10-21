"use client"

import React, { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { dnaApi } from '@/lib/api';
import { useDNAStats } from '@/hooks/use-dna-stats';
import { LibraryShell } from '@/components/layout/library-shell';
import { SectionHeader } from '@/components/library/dna/shared';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Card } from '@/components/ui/card';
import { ResponsiveBar } from '@nivo/bar';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { 
  ResponsiveContainer, 
  AreaChart, 
  Area,
  XAxis, 
  YAxis, 
  Tooltip as RechartsTooltip,
  CartesianGrid
} from 'recharts';
import { 
  MapPin, 
  Database,
  Activity,
  BarChart2
} from 'lucide-react';
import { formatNumber, formatBp } from '@/lib/utils';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from "@/components/ui/select";

export default function SSRAnalysisPage() {
  const { activeGenome } = useGenomeStore();
  const { primaryChroms } = useDNAStats();
  const [selectedChrom, setSelectedChrom] = useState<string>('chr1');
  const chartTheme = useChartTheme();

  const { data: summary, isLoading: summaryLoading } = useQuery({
    queryKey: ['ssr-summary', activeGenome, selectedChrom],
    queryFn: () => dnaApi.getSSRSummary(activeGenome, selectedChrom),
    enabled: !!selectedChrom
  });

  const { data: hits, isLoading: hitsLoading } = useQuery({
    queryKey: ['ssr-hits', activeGenome, selectedChrom],
    queryFn: () => dnaApi.getSSRHits(activeGenome, { 
      chrom: selectedChrom,
      limit: 100 
    }),
    enabled: !!selectedChrom
  });

  const { data: density, isLoading: densityLoading } = useQuery({
    queryKey: ['ssr-density', activeGenome, selectedChrom],
    queryFn: () => dnaApi.getSSRDensity(activeGenome, selectedChrom, 1000000), // 1Mb bins
    enabled: !!selectedChrom
  });
  const { data: coverage = [], isLoading: coverageLoading } = useQuery({
    queryKey: ['ssr-coverage', activeGenome],
    queryFn: () => dnaApi.getSSRCoverage(activeGenome),
  });

  const topHits = useMemo(() => hits || [], [hits]);

  // Derived Summary Stats
  const totalCount = useMemo(() => summary?.reduce((acc, curr) => acc + curr.count, 0) || 0, [summary]);
  const totalLength = useMemo(() => summary?.reduce((acc, curr) => acc + curr.total_length, 0) || 0, [summary]);

  const densityChartData = useMemo(() => {
    if (!density) return [];
    return density.map(d => ({
      bin: d.start,
      count: d.count
    }));
  }, [density]);

  return (
    <LibraryShell 
      title="SSR Analysis" 
      subtitle="Characterization of Simple Sequence Repeats (Microsatellites) and low-complexity regions."
    >
      <div className="space-y-12 pb-20">
        
        {/* FILTERS */}
        <div className="flex items-center justify-between">
           <h4 className="font-bold text-slate-900 dark:text-zinc-100 text-sm uppercase tracking-widest flex items-center gap-2">
             <Database className="h-4 w-4 text-primary" />
             Dataset Explorer
           </h4>
           <div className="w-64">
              <Select value={selectedChrom} onValueChange={setSelectedChrom}>
                 <SelectTrigger className="w-full bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800">
                    <SelectValue placeholder="Select Chromosome" />
                 </SelectTrigger>
                 <SelectContent>
                    {primaryChroms.map(c => <SelectItem key={c.name} value={c.name}>{c.name}</SelectItem>)}
                 </SelectContent>
              </Select>
           </div>
        </div>

        {/* HERO STATS */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
           <GenomicMetricCard 
             label="SSR Tracts"
             value={totalCount ? formatNumber(totalCount) : '...'}
             description="Number of tandem repeat tracts detected on the selected chromosome."
             color="#3b82f6"
           />
           <GenomicMetricCard 
             label="Summed Tract Length"
             value={totalLength ? formatBp(totalLength) : '...'}
             description="Sum of tract lengths in bp. Tracts of different periods may overlap, so this can exceed the number of bases covered."
             color="#10b981"
           />
           <GenomicMetricCard 
             label="Max Period"
             value="6"
             unit="bp"
             description="The upper limit of repeat unit length analyzed (Monomers to Hexamers)."
             color="#8b5cf6"
           />
        </div>

        {/* CHARTS */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
           {/* PERIOD DISTRIBUTION */}
           <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm h-[400px] flex flex-col">
              <div className="flex items-center justify-between mb-6">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                   <BarChart2 className="h-4 w-4 text-primary" />
                   Motif Period Distribution
                 </h3>
              </div>
              <div className="flex-1 min-h-0">
                 {summaryLoading ? (
                    <div className="h-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                    </div>
                 ) : summary && summary.length > 0 ? (
                    <ResponsiveBar
                      data={summary.map(s => ({ name: s.name, count: s.count }))}
                      keys={['count']}
                      indexBy="name"
                      margin={{ top: 10, right: 10, bottom: 40, left: 60 }}
                      padding={0.3}
                      colors={['#8b5cf6']}
                      theme={chartTheme}
                      axisBottom={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                      }}
                      axisLeft={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        format: d => formatNumber(d)
                      }}
                      enableLabel={false}
                      tooltip={({ value, indexValue }) => (
                         <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-sm">
                            <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-1">{indexValue}</div>
                            <div className="text-sm font-bold text-slate-900 dark:text-zinc-100">{formatNumber(value as number)} Hits</div>
                         </div>
                      )}
                    />
                 ) : (
                    <div className="h-full flex items-center justify-center text-slate-400 text-sm">No data available</div>
                 )}
              </div>
           </Card>

           {/* DENSITY LANDSCAPE */}
           <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm h-[400px] flex flex-col">
              <div className="flex items-center justify-between mb-6">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                   <Activity className="h-4 w-4 text-emerald-500" />
                   SSR Density Landscape
                 </h3>
                 <span className="text-[10px] text-slate-400 font-medium">1Mb Bins</span>
              </div>
              <div className="flex-1 min-h-0">
                 {densityLoading ? (
                    <div className="h-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-emerald-500" />
                    </div>
                 ) : densityChartData.length > 0 ? (
                    <ResponsiveContainer width="100%" height="100%">
                      <AreaChart data={densityChartData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                        <defs>
                          <linearGradient id="colorDensity" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="5%" stopColor="#10b981" stopOpacity={0.3}/>
                            <stop offset="95%" stopColor="#10b981" stopOpacity={0}/>
                          </linearGradient>
                        </defs>
                        <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="rgba(150, 150, 150, 0.2)" />
                        <XAxis 
                          dataKey="bin" 
                          tickFormatter={(val) => `${(val / 1000000).toFixed(0)}M`}
                          fontSize={10}
                          tickLine={false}
                          axisLine={false}
                        />
                        <YAxis 
                          fontSize={10}
                          tickLine={false}
                          axisLine={false}
                          tickFormatter={formatNumber}
                        />
                        <RechartsTooltip 
                          content={({ active, payload, label }) => {
                            if (active && payload && payload.length) {
                              const binStart = Number(label);
                              return (
                                <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-xl">
                                  <div className="text-[10px] font-bold text-slate-400 uppercase mb-1">
                                    {formatBp(binStart)} – {formatBp(binStart + 1000000)}
                                  </div>
                                  <div className="text-sm font-bold text-emerald-600 dark:text-emerald-400">
                                    {formatNumber(payload[0].value as number)} SSRs
                                  </div>
                                </div>
                              );
                            }
                            return null;
                          }}
                        />
                        <Area type="monotone" dataKey="count" stroke="#10b981" fillOpacity={1} fill="url(#colorDensity)" />
                      </AreaChart>
                    </ResponsiveContainer>
                 ) : (
                    <div className="h-full flex items-center justify-center text-slate-400 text-sm">No data available</div>
                 )}
              </div>
           </Card>
        </div>

        <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm">
          <div className="flex items-center justify-between mb-6">
            <div>
              <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest">Genome-wide SSR coverage by period</h3>
              <p className="text-xs text-slate-500 dark:text-zinc-400 mt-1">Descriptive only: bases covered by tandem tracts divided by A/C/G/T bases of the assembly. No null model, so not evidence of over- or under-representation.</p>
            </div>
            <MapPin className="h-4 w-4 text-primary" />
          </div>
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3">
            {coverage.map((item) => (
              <div key={item.period} className="rounded-xl border border-border bg-muted/30 p-3 text-center">
                <div className="font-mono text-sm font-bold text-primary">{item.name}</div>
                <div className="mt-1 text-[10px] text-muted-foreground">{formatNumber(item.count)} tracts</div>
                <div className="text-[10px] text-muted-foreground">{item.coverage_fraction === null ? 'n/a' : `${(item.coverage_fraction * 100).toFixed(3)} % of ACGT`}</div>
              </div>
            ))}
          </div>
          {!coverageLoading && coverage.length === 0 && (
            <p className="text-sm text-muted-foreground">No SSR coverage data is available for this build.</p>
          )}
        </Card>

        {/* LEADBOARD TABLE */}
        <section>
           <SectionHeader 
             title="First Tracts" 
             subtitle="The first 100 tracts of the selected chromosome by start position (0-based), not ranked by length or complexity."
             badge="Dataset View"
           />
           <Card className="overflow-hidden border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm mt-6">
                <Table>
                   <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                      <TableRow>
                        <TableHead>Motif</TableHead>
                        <TableHead>Location</TableHead>
                        <TableHead className="text-right">Units</TableHead>
                        <TableHead className="text-right">Length (bp)</TableHead>
                        <TableHead className="text-right">Complexity</TableHead>
                      </TableRow>
                   </TableHeader>
                   <TableBody>
                      {hitsLoading ? (
                        Array.from({ length: 10 }).map((_, i) => (
                           <TableRow key={i}>
                              <TableCell><div className="h-4 w-12 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                              <TableCell><div className="h-4 w-32 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                              <TableCell><div className="h-4 w-8 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                              <TableCell><div className="h-4 w-12 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                              <TableCell><div className="h-4 w-12 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                           </TableRow>
                        ))
                      ) : topHits.length > 0 ? (
                        topHits.map((hit, i) => (
                           <TableRow key={i} className="group hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                              <TableCell>
                                 <div className="flex items-center gap-3">
                                    <div className="px-2 py-0.5 rounded bg-primary/10 text-primary font-mono text-[10px] font-bold">
                                       {hit.motif}
                                    </div>
                                    <span className="text-[10px] font-bold text-slate-400 uppercase tracking-tighter">P{hit.period}</span>
                                 </div>
                              </TableCell>
                              <TableCell className="text-[10px] font-mono text-slate-500">
                                 <div className="flex items-center gap-1.5">
                                    <MapPin className="h-3 w-3 opacity-30" />
                                    {hit.chrom}:{formatNumber(hit.start)}
                                 </div>
                              </TableCell>
                              <TableCell className="text-right font-bold text-slate-900 dark:text-zinc-100 text-xs">
                                 {hit.repeat_count}x
                              </TableCell>
                              <TableCell className="text-right text-[10px] font-mono text-slate-400">
                                 {formatBp(hit.length || (hit.end - hit.start))}
                              </TableCell>
                              <TableCell className="text-right text-[10px] font-mono font-bold text-emerald-500">
                                 {hit.complexity?.toFixed(3) || '0.000'}
                              </TableCell>
                           </TableRow>
                        ))
                      ) : (
                         <TableRow>
                            <TableCell colSpan={5} className="text-center h-24 text-slate-500">No SSR hits found for this region.</TableCell>
                         </TableRow>
                      )}
                   </TableBody>
                </Table>
           </Card>
        </section>

      </div>
    </LibraryShell>
  );
}
