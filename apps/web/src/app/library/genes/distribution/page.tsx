"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { genesApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import { ResponsiveBar } from '@nivo/bar';
import { ResponsivePie } from '@nivo/pie';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { GenomicChartContainer } from '@/components/library/dna/genomic-chart-container';
import { SectionHeader } from '@/components/library/dna/shared';
import { GeneDensityMap } from '@/components/library/genes/gene-density-map';
import { Info, Map, Layers, Fingerprint, Anchor } from 'lucide-react';
import { cn, formatNumber } from '@/lib/utils';

export default function GeneDistributionPage() {
  const { activeGenome } = useGenomeStore();
  const chartTheme = useChartTheme();

  const { data: chromStats, isLoading: statsLoading, isError: statsError, refetch: refetchStats } = useQuery({
    queryKey: ['gene-chrom-stats', activeGenome],
    queryFn: () => genesApi.getChromosomeStats(activeGenome),
  });

  const { data: nested, isLoading: nestedLoading } = useQuery({
    queryKey: ['nested-genes', activeGenome],
    queryFn: () => genesApi.getNested(activeGenome),
  });

  const { data: cpgAssoc, isLoading: cpgLoading } = useQuery({
    queryKey: ['cpg-association', activeGenome],
    queryFn: () => genesApi.getCpgAssociation(activeGenome, 2000),
  });

  const barData = useMemo(() => {
    if (!chromStats) return [];
    return chromStats
      .filter(c => !c.chrom.includes('_'))
      .map(c => ({
        chromosome: c.chrom.replace('chr', ''),
        density: c.gene_count,
        normalized: c.density,
      }));
  }, [chromStats]);

  const cpgPieData = useMemo(() => {
    if (!cpgAssoc) return [];
    return [
      { id: 'With CpG', label: 'With CpG', value: cpgAssoc.with_cpg, color: '#10b981' },
      { id: 'Without CpG', label: 'Without CpG', value: cpgAssoc.without_cpg, color: '#94a3b8' }
    ];
  }, [cpgAssoc]);

  return (
    <LibraryShell 
      title="Genomic Distribution" 
      subtitle="Spatial organization of genes and intronic discovery patterns."
    >
      <div className="space-y-16 pb-20">
        
        {/* DENSITY CHART */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Global Gene Density" 
              subtitle="Comparison of gene richness across the primary human chromosomes."
            />
            <div className="px-3 py-1 bg-slate-50 dark:bg-zinc-900 rounded-full border border-slate-200 dark:border-zinc-800 text-[10px] font-bold text-slate-400 uppercase tracking-widest flex items-center gap-2">
               <Map className="h-3 w-3" />
               Spatial Track
            </div>
          </div>

          <GenomicChartContainer
            title="Genes per Chromosome"
            subtitle="Raw count of annotated features for each canonical scaffold."
            isLoading={statsLoading}
            isError={statsError}
            onRetry={() => refetchStats()}
          >
            <ResponsiveBar
              data={barData}
              keys={['density']}
              indexBy="chromosome"
              margin={{ top: 20, right: 20, bottom: 50, left: 60 }}
              padding={0.3}
              valueScale={{ type: 'linear' }}
              indexScale={{ type: 'band', round: true }}
              colors="#3b82f6"
              theme={chartTheme}
              axisBottom={{
                tickSize: 5,
                tickPadding: 5,
                tickRotation: 0,
                legend: 'Chromosome',
                legendPosition: 'middle',
                legendOffset: 40
              }}
              axisLeft={{
                tickSize: 5,
                tickPadding: 5,
                tickRotation: 0,
                legend: 'Gene Count',
                legendPosition: 'middle',
                legendOffset: -50
              }}
              enableLabel={false}
              animate={true}
              tooltip={({ value, indexValue }) => (
                <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-sm">
                   <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-1">Chr{indexValue}</div>
                   <div className="text-sm font-bold text-slate-900 dark:text-zinc-100">{formatNumber(value as number)} Genes</div>
                </div>
              )}
            />
          </GenomicChartContainer>

          <div className="mt-12">
             <GeneDensityMap />
          </div>
        </section>

        {/* CPG ASSOCIATION */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Promoter CpG Islands" 
              subtitle="Genes associated with CpG islands in their promoter regions (-2000bp to +200bp of TSS)."
            />
            <div className="px-3 py-1 bg-emerald-50 dark:bg-emerald-900/20 rounded-full border border-emerald-100 dark:border-emerald-800 text-[10px] font-bold text-emerald-600 dark:text-emerald-400 uppercase tracking-widest flex items-center gap-2">
               <Anchor className="h-3 w-3" />
               Regulatory Anchors
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
            <Card className="col-span-1 lg:col-span-1 p-6 border-slate-200 dark:border-zinc-800 flex flex-col items-center justify-center min-h-[300px]">
               {cpgLoading ? (
                  <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
               ) : cpgAssoc ? (
                  <>
                     <div className="h-[200px] w-full">
                       <ResponsivePie
                         data={cpgPieData}
                         margin={{ top: 10, right: 10, bottom: 10, left: 10 }}
                         innerRadius={0.7}
                         padAngle={2}
                         cornerRadius={4}
                         colors={{ datum: 'data.color' }}
                         enableArcLinkLabels={false}
                         enableArcLabels={false}
                         theme={chartTheme}
                       />
                     </div>
                     <div className="mt-4 text-center">
                        <div className="text-3xl font-mono font-bold text-slate-900 dark:text-zinc-100">{cpgAssoc.percentage.toFixed(1)}%</div>
                        <div className="text-xs font-bold text-slate-400 uppercase tracking-widest">Promoters with CpG</div>
                     </div>
                  </>
               ) : (
                  <div className="text-sm text-slate-500">Data not available</div>
               )}
            </Card>

            <Card className="col-span-1 lg:col-span-2 border-slate-200 dark:border-zinc-800 overflow-hidden">
               <Table>
                 <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                   <TableRow>
                     <TableHead>Biotype</TableHead>
                     <TableHead className="text-right">With CpG</TableHead>
                     <TableHead className="text-right">Total Genes</TableHead>
                     <TableHead className="text-right">Enrichment %</TableHead>
                   </TableRow>
                 </TableHeader>
                 <TableBody>
                   {cpgLoading ? (
                      [...Array(4)].map((_, i) => <TableRow key={i}><TableCell colSpan={4}><div className="h-8 w-full bg-slate-50 dark:bg-zinc-900/50 animate-pulse rounded" /></TableCell></TableRow>)
                   ) : cpgAssoc && cpgAssoc.by_biotype ? (
                      Object.entries(cpgAssoc.by_biotype)
                        .sort(([, a], [, b]) => (b as { percentage: number }).percentage - (a as { percentage: number }).percentage)
                        .slice(0, 8)
                        .map(([biotype, stats]: [string, { with_cpg: number, without_cpg: number, percentage: number }]) => (
                        <TableRow key={biotype} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                           <TableCell>
                             <Badge variant="outline" className="text-[10px] uppercase font-bold tracking-tight px-1.5 py-0">
                               {biotype.replace('_', ' ')}
                             </Badge>
                           </TableCell>
                           <TableCell className="text-right font-mono text-xs font-bold text-emerald-500">{formatNumber(stats.with_cpg)}</TableCell>
                           <TableCell className="text-right font-mono text-xs text-slate-500">{formatNumber(stats.with_cpg + stats.without_cpg)}</TableCell>
                           <TableCell className="text-right font-mono text-xs font-bold text-slate-900 dark:text-zinc-100">{stats.percentage.toFixed(1)}%</TableCell>
                        </TableRow>
                      ))
                   ) : (
                      <TableRow><TableCell colSpan={4} className="text-center h-24 text-slate-500">No data</TableCell></TableRow>
                   )}
                 </TableBody>
               </Table>
            </Card>
          </div>
        </section>

        {/* NESTED GENES */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Nested Gene Discoveries" 
              subtitle="Rare instances where small genes (guests) reside entirely within the introns of larger genes (hosts)."
            />
            <div className="px-3 py-1 bg-emerald-50 dark:bg-emerald-900/20 rounded-full border border-emerald-100 dark:border-emerald-800 text-[10px] font-bold text-emerald-600 dark:text-emerald-400 uppercase tracking-widest flex items-center gap-2">
               <Fingerprint className="h-3 w-3" />
               Intronic Residency
            </div>
          </div>

          <Card className="overflow-hidden border-slate-200 dark:border-zinc-800 shadow-sm">
            <Table>
              <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                <TableRow>
                  <TableHead className="w-48">Guest Gene (Nested)</TableHead>
                  <TableHead className="w-48">Host Gene (Container)</TableHead>
                  <TableHead>Biotype</TableHead>
                  <TableHead>Scaffold</TableHead>
                  <TableHead className="text-right">Strand Relation</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {nestedLoading ? (
                  [...Array(10)].map((_, i) => <TableRow key={i}><TableCell colSpan={5}><div className="h-10 w-full bg-slate-50 dark:bg-zinc-900/50 animate-pulse rounded" /></TableCell></TableRow>)
                ) : (
                  nested?.sample.slice(0, 15).map((pair, i) => (
                    <TableRow key={i} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors group">
                      <TableCell>
                         <div className="flex flex-col">
                            <span className="font-bold text-primary">{pair.guest_gene_name || pair.guest_gene_id}</span>
                            <span className="text-[10px] text-slate-400 font-mono">{pair.guest_gene_id}</span>
                         </div>
                      </TableCell>
                      <TableCell>
                         <div className="flex flex-col">
                            <span className="font-bold text-slate-900 dark:text-zinc-100">{pair.host_gene_name || pair.host_gene_id}</span>
                            <span className="text-[10px] text-slate-400 font-mono">{pair.host_gene_id}</span>
                         </div>
                      </TableCell>
                      <TableCell>
                         <Badge variant="outline" className="text-[9px] font-bold uppercase tracking-tighter">
                            {pair.guest_biotype?.replace('_', ' ')}
                         </Badge>
                      </TableCell>
                      <TableCell className="font-mono text-xs font-bold text-slate-500">{pair.chrom}</TableCell>
                      <TableCell className="text-right">
                         <Badge variant="secondary" className={cn(
                           "text-[9px] font-bold uppercase tracking-tight px-1.5 py-0",
                           pair.strand_relation === 'same' ? "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/20" : "bg-amber-50 text-amber-700 dark:bg-amber-900/20"
                         )}>
                            {pair.strand_relation} Strand
                         </Badge>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
            {!nestedLoading && nested?.total_pairs && nested.total_pairs > 15 && (
              <div className="p-4 bg-slate-50 dark:bg-zinc-900/50 border-t border-slate-100 dark:border-zinc-800 text-center">
                 <p className="text-[10px] text-slate-400 font-bold uppercase tracking-widest">
                   Showing 15 of {formatNumber(nested.total_pairs)} detected nested gene pairs
                 </p>
              </div>
            )}
          </Card>
        </section>

        {/* INFO PANELS */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
           <div className="flex gap-4 p-6 bg-slate-50 dark:bg-zinc-900/50 rounded-2xl border border-slate-200/50 dark:border-zinc-800/50">
              <div className="h-10 w-10 shrink-0 rounded-xl bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center">
                 <Info className="h-5 w-5 text-blue-600 dark:text-blue-400" />
              </div>
              <div>
                 <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-tight mb-1">Gene Richness</h4>
                 <p className="text-[11px] text-slate-500 dark:text-zinc-400 leading-relaxed">
                   Gene density varies significantly across chromosomes. **Chromosome 19** is exceptionally gene-rich (~23 genes/Mb), while **Chromosome 13** and **18** are relatively gene-poor.
                 </p>
              </div>
           </div>
           
           <div className="flex gap-4 p-6 bg-slate-50 dark:bg-zinc-900/50 rounded-2xl border border-slate-200/50 dark:border-zinc-800/50">
              <div className="h-10 w-10 shrink-0 rounded-xl bg-emerald-100 dark:bg-emerald-900/30 flex items-center justify-center">
                 <Layers className="h-5 w-5 text-emerald-600 dark:text-emerald-400" />
              </div>
              <div>
                 <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-tight mb-1">Intronic Discovery</h4>
                 <p className="text-[11px] text-slate-500 dark:text-zinc-400 leading-relaxed">
                   Nested genes demonstrate the extreme spatial efficiency of the genome. Many small non-coding RNAs and even small protein-coding genes are embedded within the massive introns of host genes.
                 </p>
              </div>
           </div>
        </div>

      </div>
    </LibraryShell>
  );
}
