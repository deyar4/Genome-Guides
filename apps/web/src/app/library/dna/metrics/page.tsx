"use client"

import React, { useMemo } from 'react';
import { useDNAStats } from '@/hooks/use-dna-stats';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { formatNumber, formatGb, formatMb } from '@/lib/utils';
import { SectionHeader } from '@/components/library/dna/shared';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { Info, BarChart3 } from 'lucide-react';
import { cn } from '@/lib/utils';
import { ChromosomeMap } from '@/components/library/dna/chromosome-map';

export default function ChromosomeMetricsPage() {
  const { 
    summary, 
    primaryChroms, 
    totalPrimaryLength, 
    selectedChrom, 
    setSelectedChrom,
    centromeres,
    telomeres,
    isLoading
  } = useDNAStats();


  const maxLen = useMemo(() => 
    primaryChroms.length > 0 ? Math.max(...primaryChroms.map(c => c.length)) : 0, 
  [primaryChroms]);

  const assembly = summary.data?.assembly;

  return (
    <LibraryShell 
      title="Chromosome Metrics" 
      subtitle="Length, GC content and GC skew per sequence of T2T-CHM13v2.0."
    >
      <div className="space-y-16 pb-20">
        
        {/* Core Stats Section */}
        <section>
          <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
            <GenomicMetricCard 
              label="Primary Sequence" 
              value={formatGb(totalPrimaryLength)}
              unit="Gb"
              description={`Sum of ${primaryChroms.length} primary sequences (incl. chrM).`}
              color="#3b82f6"
            />
            <GenomicMetricCard 
              label="Karyotype" 
              value={primaryChroms.length.toString()}
              description="Primary scaffolds (1-22, X, Y, M)."
              color="#10b981"
            />
            <GenomicMetricCard 
              label="Average Scaffold" 
              value={(totalPrimaryLength / (primaryChroms.length || 1) / 1e6).toFixed(1)}
              unit="Mb"
              description="Mean length of primary scaffolds."
              color="#8b5cf6"
            />
            <GenomicMetricCard 
              label="Total Assembly" 
              value={assembly?.contig_count ? formatNumber(assembly.contig_count) : '0'}
              description="Sequences listed in the assembly index."
              color="#94a3b8"
            />
          </div>
        </section>

        {/* Visual Karyotype Map */}
        <section>
          <SectionHeader 
            title="Karyotype Landscape" 
            subtitle="Sequence lengths with centromere regions and telomere records from the annotation."
          />
          <Card className="p-8 border-slate-200 dark:border-zinc-800 shadow-sm bg-white dark:bg-zinc-950 overflow-hidden">
             {isLoading ? (
               <div className="h-[350px] flex items-center justify-center">
                  <div className="flex gap-4 items-end h-full">
                    {[40, 60, 80, 50, 90, 30, 70, 100, 60, 40, 80, 50].map((h, i) => (
                      <div key={i} className="w-4 bg-slate-100 dark:bg-zinc-900 animate-pulse rounded-full" style={{ height: `${h}%` }} />
                    ))}
                  </div>
               </div>
             ) : (
               <ChromosomeMap 
                chromosomes={primaryChroms} 
                centromeres={centromeres.data}
                telomeres={telomeres.data}
                selectedChrom={selectedChrom}
                onSelect={setSelectedChrom}
               />
             )}
          </Card>
        </section>

        {/* Unified Karyotype Registry */}
        <section>
          <SectionHeader 
            title="Karyotype Registry" 
            subtitle="Length (bp), GC content (%) and GC skew 100*(G-C)/(G+C) (%) per sequence."
          />
          <Card className="border-slate-200 dark:border-zinc-800 shadow-sm overflow-hidden bg-white dark:bg-zinc-950">
            <div className="max-h-[600px] overflow-auto custom-scrollbar">
              <Table>
                <TableHeader className="bg-slate-50 dark:bg-zinc-900 sticky top-0 z-20 border-b">
                  <TableRow>
                    <TableHead className="w-24">Scaffold</TableHead>
                    <TableHead className="w-48">Relative Scale (Mb)</TableHead>
                    <TableHead className="text-right">Absolute Length</TableHead>
                    <TableHead className="text-right">GC Content</TableHead>
                    <TableHead className="text-right">GC Skew (%)</TableHead>
                    <TableHead className="text-right">Type</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {isLoading ? (
                    [...Array(10)].map((_, i) => (
                      <TableRow key={i}>
                        <TableCell><div className="h-4 w-12 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        <TableCell><div className="h-2 w-full bg-slate-100 dark:bg-zinc-800 animate-pulse rounded-full" /></TableCell>
                        <TableCell><div className="h-4 w-24 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        <TableCell><div className="h-4 w-16 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        <TableCell><div className="h-4 w-12 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        <TableCell><div className="h-4 w-16 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                      </TableRow>
                    ))
                  ) : (
                    primaryChroms.map((c) => {
                      const isSelected = selectedChrom === c.name;
                      const widthPercent = (c.length / maxLen) * 100;
                      return (
                        <TableRow 
                          key={c.name} 
                          onClick={() => setSelectedChrom(c.name)}
                          className={cn(
                            "cursor-pointer group/row transition-colors",
                            isSelected 
                              ? "bg-primary/5 hover:bg-primary/10" 
                              : "hover:bg-slate-50/50 dark:hover:bg-zinc-900/50"
                          )}
                        >
                          <TableCell className={cn("font-bold", isSelected ? "text-primary" : "text-slate-900 dark:text-zinc-100")}>
                            {c.name}
                          </TableCell>
                          <TableCell>
                             <div className="flex items-center gap-3">
                                <div className="flex-1 h-2 bg-slate-100 dark:bg-zinc-900 rounded-full overflow-hidden border border-slate-200 dark:border-zinc-800">
                                   <div 
                                     className={cn("h-full transition-all duration-500 rounded-full", isSelected ? "bg-primary" : "bg-slate-300 dark:bg-zinc-700")}
                                     style={{ width: `${widthPercent}%` }}
                                   />
                                </div>
                                <span className="text-[10px] font-mono text-slate-400 w-12 text-right">
                                  {formatMb(c.length)}
                                </span>
                             </div>
                          </TableCell>
                          <TableCell className="text-right font-mono text-[11px] text-slate-500">
                             {formatNumber(c.length)} <span className="text-[9px] opacity-50 uppercase">bp</span>
                          </TableCell>
                          <TableCell className="text-right">
                             <div className="inline-flex items-center gap-2">
                                <div className={cn(
                                  "w-1 h-3 rounded-full",
                                  c.gc_content > 45 ? "bg-emerald-500" : c.gc_content < 40 ? "bg-amber-500" : "bg-blue-500"
                                )} />
                                <span className="font-mono text-[11px] font-bold">{c.gc_content.toFixed(2)}%</span>
                             </div>
                          </TableCell>
                          <TableCell className="text-right font-mono text-[11px] text-slate-400">
                             {c.gc_skew != null ? `${c.gc_skew.toFixed(2)}%` : "N/A"}
                          </TableCell>
                          <TableCell className="text-right">
                             <span className="px-2 py-0.5 rounded bg-slate-100 dark:bg-zinc-800 text-[9px] font-bold uppercase tracking-tighter text-slate-500">
                                {c.is_mitochondrial ? 'MT' : 'Nuclear'}
                             </span>
                          </TableCell>
                        </TableRow>
                      );
                    })
                  )}
                </TableBody>
              </Table>
            </div>
          </Card>
        </section>

        {/* Assembly Quality Analysis */}
        <section>
          <SectionHeader 
            title="Assembly Quality Metrics" 
            subtitle="N50 and L50 computed over all sequences of the assembly."
          />
          <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
             <Card className="p-8 bg-primary text-primary-foreground border-none shadow-2xl relative overflow-hidden group">
                <div className="relative z-10">
                   <div className="text-[10px] font-bold uppercase tracking-widest text-slate-400 mb-2">Contiguity (N50)</div>
                   <div className="text-4xl font-mono font-bold mb-4">{formatMb(assembly?.n50 || 0)}</div>
                   <p className="text-xs text-slate-400 leading-relaxed italic">
                      The N50 represents the length of the shortest scaffold such that 50% of the total assembly length is contained in scaffolds of this size or larger.
                   </p>
                </div>
                <BarChart3 className="absolute -right-8 -bottom-8 w-48 h-48 text-primary-foreground/10 pointer-events-none transition-transform group-hover:scale-110 duration-700" />
             </Card>

             <Card className="p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm flex flex-col justify-between relative group">
                <div>
                   <div className="text-[10px] font-bold uppercase tracking-widest text-slate-400 mb-2">L50 Statistic</div>
                   <div className="text-3xl font-mono font-bold text-primary mb-4">{assembly?.l50 || '0'}</div>
                   <p className="text-xs text-slate-500 dark:text-zinc-400 leading-relaxed italic">
                      Smallest number of scaffolds whose lengths sum to at least 50% of the assembly. A lower L50 indicates higher contiguity.
                   </p>
                </div>
                <div className="absolute top-4 right-4 text-slate-300 dark:text-zinc-700">
                   <Info className="h-4 w-4" />
                </div>
             </Card>

             <Card className="p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm flex flex-col justify-between">
                <div>
                   <div className="text-[10px] font-bold uppercase tracking-widest text-slate-400 mb-2">Global GC Consistency</div>
                   <div className="text-3xl font-mono font-bold text-emerald-500 mb-4">
                      {(() => {
                        const val = assembly?.gc_content ?? summary.data?.nuclear_composition?.gc_content;
                        return val ? `${val.toFixed(2)}%` : '0.00%';
                      })()}
                   </div>
                   <p className="text-xs text-slate-500 dark:text-zinc-400 leading-relaxed italic">
                      GC content: 100*(G+C)/(A+C+G+T) over the whole assembly.
                   </p>
                </div>
             </Card>
          </div>
        </section>
      </div>
    </LibraryShell>
  );
}
