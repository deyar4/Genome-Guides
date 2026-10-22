"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { genesApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { formatNumber, geneHref } from '@/lib/utils';
import { GenomicChartContainer } from '@/components/library/dna/genomic-chart-container';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { SectionHeader } from '@/components/library/dna/shared';
import { BiotypeTreemap } from '@/components/library/genes/biotype-treemap';
import { List, Trophy, Maximize, Layers } from 'lucide-react';
import Link from 'next/link';

export default function GeneCensusPage() {
  const { activeGenome } = useGenomeStore();

  const { data: biotypes, isLoading, isError, refetch } = useQuery({
    queryKey: ['biotypes', activeGenome],
    queryFn: () => genesApi.getBiotypes(activeGenome),
  });

  const { data: largest } = useQuery({
    queryKey: ['largest-genes', activeGenome],
    queryFn: () => genesApi.getLargest(activeGenome),
  });

  const { data: mostExons } = useQuery({
    queryKey: ['most-exonic-genes', activeGenome],
    queryFn: () => genesApi.getMostExons(activeGenome),
  });

  const { data: summary } = useQuery({
    queryKey: ['gene-summary', activeGenome],
    queryFn: () => genesApi.getSummary(activeGenome),
  });

  const proteinCodingCount = useMemo(() => 
    biotypes?.find(b => b.biotype === 'protein_coding')?.count || 0
  , [biotypes]);

  return (
    <LibraryShell 
      title="Biotype Census" 
      subtitle="Gene counts by biotype."
    >
      <div className="space-y-12 pb-20">
        
        {/* HERO METRICS */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
           <GenomicMetricCard 
             label="Total Features"
             value={summary?.total_genes ? formatNumber(summary.total_genes) : '...'}
             description="Total annotated genes (gene models)."
             color="#3b82f6"
           />
           <GenomicMetricCard 
             label="Protein Coding"
             value={proteinCodingCount > 0 ? formatNumber(proteinCodingCount) : '...'}
             description="Genes with biotype protein_coding."
             color="#10b981"
           />
           <GenomicMetricCard 
             label="Biotype Diversity"
             value={biotypes?.length.toString() || '0'}
             description="Number of distinct biotype values."
             color="#8b5cf6"
           />
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-12 items-stretch">
          <GenomicChartContainer
            title="Biotype Hierarchy"
            subtitle="Gene count per biotype."
            isLoading={isLoading}
            isError={isError}
            height="h-[500px]"
            onRetry={() => refetch()}
          >
            {biotypes && <BiotypeTreemap data={biotypes} />}
          </GenomicChartContainer>

          <section className="flex flex-col">
            <div className="flex items-center justify-between mb-6">
               <SectionHeader 
                 title="Biotype Registry" 
                 subtitle="Gene count per biotype."
               />
               <div className="px-3 py-1 bg-slate-50 dark:bg-zinc-900 rounded-full border border-slate-200 dark:border-zinc-800 text-[10px] font-bold text-slate-400 uppercase tracking-widest flex items-center gap-2">
                  <List className="h-3 w-3" />
                  Catalog
               </div>
            </div>

            <Card className="flex-1 overflow-hidden border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
              <div className="max-h-[400px] overflow-y-auto custom-scrollbar">
                <Table>
                  <TableHeader className="bg-slate-50 dark:bg-zinc-900/50 sticky top-0 z-10">
                    <TableRow>
                      <TableHead>Category</TableHead>
                      <TableHead className="text-right">Count</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {biotypes?.map((b) => (
                      <TableRow key={b.biotype} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors group">
                        <TableCell className="capitalize font-bold text-slate-700 dark:text-zinc-300 text-xs">
                          {b.biotype.replace(/_/g, ' ')}
                        </TableCell>
                        <TableCell className="text-right font-mono text-xs text-slate-500 group-hover:text-primary transition-colors">
                          {formatNumber(b.count)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            </Card>
          </section>
        </div>

        {/* LEADERBOARDS */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Genomic Leaderboards" 
              subtitle="Largest gene records by span and by exon count, as served by the API. Caution: entries whose span exceeds their chromosome (the chrY entries at the top) merge same-ID genes from several loci (e.g. PAR genes on chrX and chrY); this is a known data issue, not biology."
            />
            <div className="px-3 py-1 bg-amber-50 dark:bg-amber-900/20 rounded-full border border-amber-100 dark:border-amber-800 text-[10px] font-bold text-amber-600 dark:text-amber-400 uppercase tracking-widest flex items-center gap-2">
               <Trophy className="h-3 w-3" />
               Reference Extremes
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-12">
             {/* LARGEST GENES */}
             <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
                <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest mb-6 flex items-center gap-2">
                   <Maximize className="h-4 w-4 text-blue-500" />
                   Top Largest Loci
                </h4>
                <div className="space-y-4">
                   {largest?.slice(0, 5).map((g, i) => (
                      <Link key={g.gene_id} href={geneHref(g.gene_id, g.chrom)} className="flex items-center justify-between p-3 bg-slate-50 dark:bg-zinc-900 rounded-xl hover:ring-1 ring-primary transition-all group">
                         <div className="flex items-center gap-3">
                            <span className="text-xs font-bold text-slate-300">#{i+1}</span>
                            <div className="flex flex-col">
                               <span className="text-sm font-bold text-slate-900 dark:text-zinc-100 group-hover:text-primary transition-colors">{g.gene_name || g.gene_id}</span>
                               <span className="text-[10px] text-slate-500 uppercase font-medium">{g.biotype?.replace('_', ' ')}</span>
                            </div>
                         </div>
                         <div className="text-right">
                            <div className="text-xs font-mono font-bold text-slate-700 dark:text-zinc-300">{(g.value / 1e6).toFixed(2)} Mb</div>
                            <div className="text-[9px] text-slate-400 uppercase font-bold tracking-tighter">{g.chrom}</div>
                         </div>
                      </Link>
                   ))}
                </div>
             </Card>

             {/* MOST EXONS */}
             <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
                <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest mb-6 flex items-center gap-2">
                   <Layers className="h-4 w-4 text-emerald-500" />
                   Most Complex Splicing
                </h4>
                <div className="space-y-4">
                   {mostExons?.slice(0, 5).map((g, i) => (
                      <Link key={g.gene_id} href={geneHref(g.gene_id, g.chrom)} className="flex items-center justify-between p-3 bg-slate-50 dark:bg-zinc-900 rounded-xl hover:ring-1 ring-primary transition-all group">
                         <div className="flex items-center gap-3">
                            <span className="text-xs font-bold text-slate-300">#{i+1}</span>
                            <div className="flex flex-col">
                               <span className="text-sm font-bold text-slate-900 dark:text-zinc-100 group-hover:text-primary transition-colors">{g.gene_name || g.gene_id}</span>
                               <span className="text-[10px] text-slate-500 uppercase font-medium">{g.biotype?.replace('_', ' ')}</span>
                            </div>
                         </div>
                         <div className="text-right">
                            <div className="text-xs font-mono font-bold text-emerald-600 dark:text-emerald-400">{g.value} Exons</div>
                            <div className="text-[9px] text-slate-400 uppercase font-bold tracking-tighter">{g.chrom}</div>
                         </div>
                      </Link>
                   ))}
                </div>
             </Card>
          </div>
        </section>


      </div>
    </LibraryShell>
  );
}
