"use client"

import React, { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { dnaApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { formatNumber, cn } from '@/lib/utils';
import { ResponsiveLine } from '@nivo/line';
import { Download, Copy, Target, Activity, Settings2, FileText, ChevronRight } from 'lucide-react';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { GenomicChartContainer } from '@/components/library/dna/genomic-chart-container';
import { SectionHeader } from '@/components/library/dna/shared';

export default function SequenceToolsPage() {
  const { activeGenome } = useGenomeStore();
  const chartTheme = useChartTheme();
  
  const [chrom, setChrom] = useState('chr1');
  const [start, setStart] = useState('1');
  const [end, setEnd] = useState('1000');
  const [fetchTrigger, setFetchTrigger] = useState(0);

  const { data: chromosomesData } = useQuery({
    queryKey: ['chromosomes', activeGenome],
    queryFn: () => dnaApi.getChromosomes(activeGenome),
  });

  const primaryChroms = useMemo(() => {
    if (!chromosomesData?.chromosomes) return [];
    const order = [...Array.from({ length: 22 }, (_, i) => `chr${i + 1}`), 'chrX', 'chrY', 'chrM'];
    return chromosomesData.chromosomes
      .filter(c => order.includes(c.name))
      .sort((a, b) => order.indexOf(a.name) - order.indexOf(b.name));
  }, [chromosomesData]);

  const { data: sequence, isLoading: isSeqLoading } = useQuery({
    queryKey: ['sequence', activeGenome, chrom, start, end, fetchTrigger],
    queryFn: () => dnaApi.getSequence(activeGenome, chrom, parseInt(start), parseInt(end)),
    enabled: fetchTrigger > 0,
  });

  const { data: gcSkew, isLoading: isSkewLoading, isError: isSkewError } = useQuery({
    queryKey: ['gc-skew', activeGenome, chrom, fetchTrigger],
    queryFn: () => dnaApi.getGCSkew(activeGenome, chrom, 100000),
    enabled: fetchTrigger > 0,
  });

  const handleFetch = () => {
    setFetchTrigger(prev => prev + 1);
  };

  const skewData = useMemo(() => {
    if (!gcSkew) return [];
    return [{
      id: 'GC Skew',
      data: gcSkew.map((d, i) => ({
        x: i, // Use index for x-axis to avoid label overcrowding
        y: d.gc_skew,
        pos: d.start
      }))
    }];
  }, [gcSkew]);

  const copyToClipboard = () => {
    if (sequence) {
      navigator.clipboard.writeText(sequence);
    }
  };

  return (
    <LibraryShell 
      title="Sequence Analysis" 
      subtitle="Extract raw genomic substrings and visualize biochemical strand asymmetry."
    >
      <div className="space-y-16 pb-20">
        
        {/* CONTROL PANEL */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Analysis Parameters" 
              subtitle="Specify genomic coordinates and target scaffold for extraction."
            />
            <div className="px-3 py-1 bg-slate-100 dark:bg-zinc-900 rounded-full border border-slate-200 dark:border-zinc-800 text-[10px] font-bold text-slate-500 uppercase tracking-widest flex items-center gap-2">
               <Settings2 className="h-3 w-3" />
               Configuration
            </div>
          </div>

          <Card className="bg-white dark:bg-zinc-900/50 border-slate-200 dark:border-zinc-800 shadow-sm overflow-hidden">
            <div className="p-8">
              <div className="grid grid-cols-1 md:grid-cols-4 gap-8">
                <div className="space-y-3">
                  <label className="text-[10px] font-bold uppercase tracking-widest text-slate-400">Scaffold Focus</label>
                  <Select value={chrom} onValueChange={setChrom}>
                    <SelectTrigger className="bg-white dark:bg-zinc-950 h-11 border-slate-200 dark:border-zinc-800">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {primaryChroms.map(c => (
                        <SelectItem key={c.name} value={c.name}>{c.name}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-3">
                  <label className="text-[10px] font-bold uppercase tracking-widest text-slate-400">Range Start (bp)</label>
                  <Input 
                    type="number" 
                    className="bg-white dark:bg-zinc-950 h-11 border-slate-200 dark:border-zinc-800 font-mono" 
                    value={start} 
                    onChange={e => setStart(e.target.value)} 
                  />
                </div>
                <div className="space-y-3">
                  <label className="text-[10px] font-bold uppercase tracking-widest text-slate-400">Range End (bp)</label>
                  <Input 
                    type="number" 
                    className="bg-white dark:bg-zinc-950 h-11 border-slate-200 dark:border-zinc-800 font-mono" 
                    value={end} 
                    onChange={e => setEnd(e.target.value)} 
                  />
                </div>
                <div className="flex items-end">
                  <Button className="w-full h-11 gap-2 font-bold uppercase text-[11px] tracking-widest shadow-lg shadow-primary/20" onClick={handleFetch}>
                    <Target className="h-4 w-4" />
                    Initialize Load
                  </Button>
                </div>
              </div>
            </div>
            <div className="px-8 py-3 bg-slate-50 dark:bg-zinc-900 border-t border-slate-100 dark:border-zinc-800 flex items-center gap-2">
               <Activity className="h-3 w-3 text-slate-400" />
               <span className="text-[10px] text-slate-400 italic">Analysis limited to 100kb windows for real-time performance.</span>
            </div>
          </Card>
        </section>

        {/* SEQUENCE EXTRACTOR */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Reference String" 
              subtitle="Materialized sequence data extracted directly from the reference assembly."
            />
            <div className="flex gap-3">
              <Button variant="outline" size="sm" className="h-9 gap-2 text-[10px] font-bold uppercase tracking-widest border-slate-200 dark:border-zinc-800" onClick={copyToClipboard} disabled={!sequence}>
                <Copy className="h-3.5 w-3.5" /> Copy
              </Button>
              <Button variant="outline" size="sm" className="h-9 gap-2 text-[10px] font-bold uppercase tracking-widest border-slate-200 dark:border-zinc-800" disabled={!sequence}>
                <Download className="h-3.5 w-3.5" /> Fasta
              </Button>
            </div>
          </div>

          <Card className="bg-white dark:bg-zinc-950 overflow-hidden border-slate-200 dark:border-zinc-800 shadow-sm">
            <div className="p-3 bg-slate-50 dark:bg-zinc-900 border-b border-slate-100 dark:border-zinc-800 flex items-center justify-between px-6">
               <div className="flex items-center gap-2">
                  <FileText className="h-3.5 w-3.5 text-slate-400" />
                  <span className="text-[10px] font-mono text-slate-400 uppercase tracking-widest">{chrom}:{start}-{end}</span>
               </div>
               <Badge variant="outline" className="text-[9px] font-bold text-slate-400 uppercase">{sequence?.length || 0} bp</Badge>
            </div>
            <CardContent className="p-0">
              {isSeqLoading ? (
                <div className="flex flex-col items-center justify-center py-20 text-slate-400 gap-4">
                  <div className="flex items-center gap-3">
                     <Target className="h-5 w-5 animate-pulse text-primary" />
                     <span className="font-bold text-[11px] uppercase tracking-widest">Streaming from index...</span>
                  </div>
                </div>
              ) : sequence ? (
                <div className="font-mono text-sm break-all leading-loose bg-white dark:bg-zinc-950 p-10 select-all max-h-[400px] overflow-y-auto custom-scrollbar text-slate-700 dark:text-zinc-300">
                  {sequence}
                </div>
              ) : (
                <div className="text-center py-20 text-slate-400 italic text-xs flex flex-col items-center gap-2 opacity-50">
                  <ChevronRight className="h-8 w-8 rotate-90" />
                  Configure coordinates and click Load Data
                </div>
              )}
            </CardContent>
          </Card>
        </section>

        {/* GC SKEW ANALYSIS */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Strand Asymmetry (GC Skew)" 
              subtitle="Visualization of (G-C)/(G+C) distribution revealing potential replication origins and mutational bias."
            />
            <div className="px-3 py-1 bg-blue-50 dark:bg-blue-900/20 rounded-full border border-blue-100 dark:border-blue-800 text-[10px] font-bold text-blue-600 dark:text-blue-400 uppercase tracking-widest flex items-center gap-2">
               <Activity className="h-3 w-3" />
               Strand Bias Track
            </div>
          </div>

          <GenomicChartContainer
            title={`GC Skew Profile: ${chrom}`}
            subtitle="Analyzed using 100kb non-overlapping windows across the entire scaffold."
            isLoading={isSkewLoading}
            isError={isSkewError}
            onRetry={() => handleFetch()}
            height="h-[400px]"
          >
            <ResponsiveLine
              data={skewData}
              margin={{ top: 20, right: 30, bottom: 50, left: 60 }}
              xScale={{ type: 'point' }}
              yScale={{ type: 'linear', min: -0.15, max: 0.15, stacked: false }}
              curve="monotoneX"
              axisBottom={null}
              axisLeft={{
                tickSize: 5,
                tickPadding: 5,
                tickRotation: 0,
                legend: 'Skew Index',
                legendPosition: 'middle',
                legendOffset: -45
              }}
              enableGridX={false}
              colors={['#3b82f6']}
              enablePoints={false}
              lineWidth={2}
              enableArea={true}
              areaOpacity={0.1}
              useMesh={true}
              theme={chartTheme}
              tooltip={({ point }) => (
                <div className="bg-white dark:bg-zinc-900 p-3 rounded-lg text-[11px] shadow-xl border border-slate-200 dark:border-zinc-800">
                  <div className="font-bold border-b border-slate-100 dark:border-zinc-800 pb-1 mb-1">Position: {formatNumber((point.data as { pos: number }).pos)} bp</div>
                  <div className="flex items-center justify-between gap-4">
                     <span className="text-slate-400">Skew Index:</span>
                     <span className={cn("font-mono font-bold", (point.data.y as number) >= 0 ? "text-emerald-500" : "text-amber-500")}>
                       {(point.data.y as number).toFixed(4)}
                     </span>
                  </div>
                </div>
              )}
            />
          </GenomicChartContainer>
        </section>

      </div>
    </LibraryShell>
  );
}
