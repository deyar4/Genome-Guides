"use client"

import React from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { genesApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { formatNumber } from '@/lib/utils';
import { SectionHeader } from '@/components/library/dna/shared';
import { Trophy, Layout } from 'lucide-react';

export default function GeneArchitecturePage() {
  const { activeGenome } = useGenomeStore();

  const { data: sizeStats } = useQuery({
    queryKey: ['gene-size-stats', activeGenome],
    queryFn: () => genesApi.getSizeStats(activeGenome),
  });

  const { data: largestGenes, isLoading: largestLoading } = useQuery({
    queryKey: ['largest-genes', activeGenome],
    queryFn: () => genesApi.getLargest(activeGenome),
  });

  const { data: complexGenes, isLoading: complexLoading } = useQuery({
    queryKey: ['most-exons', activeGenome],
    queryFn: () => genesApi.getMostExons(activeGenome),
  });

  return (
    <LibraryShell 
      title="Architecture & Size" 
      subtitle="Length statistics of gene models, exons and introns."
    >
      <div className="space-y-16 pb-20">
        
        {/* TYPICAL DIMENSIONS */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Canonical Scale" 
              subtitle="Median, mean and maximum length in bp."
            />
            <div className="px-3 py-1 bg-slate-50 dark:bg-zinc-900 rounded-full border border-slate-200 dark:border-zinc-800 text-[10px] font-bold text-slate-400 uppercase tracking-widest flex items-center gap-2">
               <Layout className="h-3 w-3" />
               Structural Metrics
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
            <DimensionCard 
              title="Whole Gene Span" 
              stats={sizeStats?.genes} 
              unit="bp" 
              description="Gene model end minus start."
              color="blue"
            />
            <DimensionCard 
              title="Exon Length" 
              stats={sizeStats?.exons} 
              unit="bp" 
              description="Length of each annotated exon."
              color="emerald"
            />
            <DimensionCard 
              title="Intron Length" 
              stats={sizeStats?.introns} 
              unit="bp" 
              description="Length of each annotated intron."
              color="amber"
            />
          </div>
        </section>

        {/* LEADERBOARDS */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Genomic Leaderboards" 
              subtitle="Largest gene records by span and by exon count, as served by the API. Caution: entries whose span exceeds their chromosome (the chrY entries at the top) merge same-ID genes from several loci (e.g. PAR genes on chrX and chrY); this is a known data issue, not biology."
            />
            <div className="px-3 py-1 bg-amber-50 dark:bg-amber-900/20 rounded-full border border-amber-100 dark:border-amber-800 text-[10px] font-bold text-amber-600 dark:text-amber-400 uppercase tracking-widest flex items-center gap-2">
               <Trophy className="h-3 w-3" />
               Extreme Dimensions
            </div>
          </div>
          
          <Tabs defaultValue="giants" className="w-full">
            <TabsList className="mb-6 bg-slate-100 dark:bg-zinc-900 p-1 rounded-xl h-11">
              <TabsTrigger value="giants" className="rounded-lg px-6 data-[state=active]:bg-white dark:data-[state=active]:bg-zinc-800 data-[state=active]:shadow-sm">The Giants (Span)</TabsTrigger>
              <TabsTrigger value="complex" className="rounded-lg px-6 data-[state=active]:bg-white dark:data-[state=active]:bg-zinc-800 data-[state=active]:shadow-sm">High Complexity (Exons)</TabsTrigger>
            </TabsList>
            
            <TabsContent value="giants" className="mt-0">
              <Card className="overflow-hidden border-slate-200 dark:border-zinc-800">
                <Table>
                  <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                    <TableRow>
                      <TableHead className="w-[100px]">Rank</TableHead>
                      <TableHead>Gene Identity</TableHead>
                      <TableHead>Biotype</TableHead>
                      <TableHead>Scaffold</TableHead>
                      <TableHead className="text-right">Span (kb)</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {largestLoading ? (
                      [...Array(5)].map((_, i) => <TableRow key={i}><TableCell colSpan={5}><div className="h-10 w-full bg-slate-50 dark:bg-zinc-900/50 animate-pulse rounded" /></TableCell></TableRow>)
                    ) : (
                      largestGenes?.map((gene) => (
                        <TableRow key={gene.gene_id} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                          <TableCell className="font-mono text-[10px] font-bold text-slate-400">#{gene.rank}</TableCell>
                          <TableCell>
                            <div className="flex flex-col">
                               <span className="font-bold text-slate-900 dark:text-zinc-100">{gene.gene_name || gene.gene_id}</span>
                               <span className="text-[10px] text-slate-400 font-mono">{gene.gene_id}</span>
                            </div>
                          </TableCell>
                          <TableCell>
                            <Badge variant="outline" className="text-[9px] font-bold uppercase tracking-tighter px-1.5 py-0">
                              {gene.biotype?.replace('_', ' ')}
                            </Badge>
                          </TableCell>
                          <TableCell className="font-mono text-xs font-bold text-slate-500">{gene.chrom}</TableCell>
                          <TableCell className="text-right font-mono text-xs font-bold text-blue-500">
                            {formatNumber(Math.round(gene.value / 1000))} kb
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </Card>
            </TabsContent>

            <TabsContent value="complex" className="mt-0">
              <Card className="overflow-hidden border-slate-200 dark:border-zinc-800">
                <Table>
                  <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                    <TableRow>
                      <TableHead className="w-[100px]">Rank</TableHead>
                      <TableHead>Gene Identity</TableHead>
                      <TableHead>Biotype</TableHead>
                      <TableHead>Scaffold</TableHead>
                      <TableHead className="text-right">Exon Count</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {complexLoading ? (
                      [...Array(5)].map((_, i) => <TableRow key={i}><TableCell colSpan={5}><div className="h-10 w-full bg-slate-50 dark:bg-zinc-900/50 animate-pulse rounded" /></TableCell></TableRow>)
                    ) : (
                      complexGenes?.map((gene) => (
                        <TableRow key={gene.gene_id} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                          <TableCell className="font-mono text-[10px] font-bold text-slate-400">#{gene.rank}</TableCell>
                          <TableCell>
                            <div className="flex flex-col">
                               <span className="font-bold text-slate-900 dark:text-zinc-100">{gene.gene_name || gene.gene_id}</span>
                               <span className="text-[10px] text-slate-400 font-mono">{gene.gene_id}</span>
                            </div>
                          </TableCell>
                          <TableCell>
                            <Badge variant="outline" className="text-[9px] font-bold uppercase tracking-tighter px-1.5 py-0">
                              {gene.biotype?.replace('_', ' ')}
                            </Badge>
                          </TableCell>
                          <TableCell className="font-mono text-xs font-bold text-slate-500">{gene.chrom}</TableCell>
                          <TableCell className="text-right font-mono text-xs font-bold text-emerald-500">
                            {formatNumber(gene.value)}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </Card>
            </TabsContent>
          </Tabs>
        </section>


      </div>
    </LibraryShell>
  );
}

function DimensionCard({ title, stats, unit, description, color }: { title: string, stats: Record<string, number> | undefined, unit: string, description: string, color: 'blue' | 'emerald' | 'amber' }) {
  const colorMap = {
    blue: 'text-blue-600 bg-blue-50 dark:bg-blue-900/20 border-blue-100 dark:border-blue-800',
    emerald: 'text-emerald-600 bg-emerald-50 dark:bg-emerald-900/20 border-emerald-100 dark:border-emerald-800',
    amber: 'text-amber-600 bg-amber-50 dark:bg-amber-900/20 border-amber-100 dark:border-amber-800'
  };

  const valColorMap = {
    blue: 'text-blue-500',
    emerald: 'text-emerald-500',
    amber: 'text-amber-500'
  };

  return (
    <Card className="flex flex-col p-6 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm relative group hover:shadow-md transition-all">
      <div className="flex items-center justify-between mb-6">
        <h3 className="text-[10px] font-bold uppercase tracking-widest text-slate-400">{title}</h3>
        <Badge variant="outline" className={`text-[9px] font-bold ${colorMap[color]}`}>MEDIAN STAT</Badge>
      </div>

      <div className="flex items-baseline gap-2 mb-6">
        <div className={`text-4xl font-mono font-bold text-slate-900 dark:text-zinc-100`}>
          {stats ? formatNumber(Math.round(stats.median)) : '...'}
        </div>
        <div className="text-[10px] font-bold text-slate-400 uppercase">{unit}</div>
      </div>
      
      <div className="grid grid-cols-2 gap-4 pt-4 border-t border-slate-100 dark:border-zinc-800 mb-6">
        <div>
          <div className="text-[9px] font-bold text-slate-400 uppercase tracking-tighter">Average</div>
          <div className="text-sm font-mono font-bold text-slate-700 dark:text-zinc-300">
            {stats ? formatNumber(Math.round(stats.mean)) : '...'}
          </div>
        </div>
        <div className="text-right">
          <div className="text-[9px] font-bold text-slate-400 uppercase tracking-tighter">Maximum</div>
          <div className={`text-sm font-mono font-bold ${valColorMap[color]}`}>
            {stats ? formatNumber(Math.round(stats.max)) : '...'}
          </div>
        </div>
      </div>
      
      <p className="text-[11px] text-slate-500 dark:text-zinc-500 leading-relaxed italic">
        {description}
      </p>
    </Card>
  );
}
