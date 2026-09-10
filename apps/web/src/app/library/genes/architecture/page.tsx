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
import { Layers, Info, Trophy, Layout } from 'lucide-react';

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
      subtitle="Structural properties and extreme dimensions of the functional genome."
    >
      <div className="space-y-16 pb-20">
        
        {/* TYPICAL DIMENSIONS */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Canonical Scale" 
              subtitle="The average dimensions of genomic features across the reference assembly."
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
              description="Total genomic footprint from transcription start to end."
              color="blue"
            />
            <DimensionCard 
              title="Exon Length" 
              stats={sizeStats?.exons} 
              unit="bp" 
              description="The functional segments that remain after splicing."
              color="emerald"
            />
            <DimensionCard 
              title="Intron Length" 
              stats={sizeStats?.introns} 
              unit="bp" 
              description="Non-coding regions removed during maturation."
              color="amber"
            />
          </div>
        </section>

        {/* LEADERBOARDS */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Genomic Leaderboards" 
              subtitle="The outliers and giants that define the physical limits of the assembly."
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

        {/* FOOTER INFO */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
           <div className="flex gap-4 p-6 bg-slate-50 dark:bg-zinc-900/50 rounded-2xl border border-slate-200/50 dark:border-zinc-800/50">
              <div className="h-10 w-10 shrink-0 rounded-xl bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center">
                 <Info className="h-5 w-5 text-blue-600 dark:text-blue-400" />
              </div>
              <div>
                 <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-tight mb-1">Scale Variation</h4>
                 <p className="text-[11px] text-slate-500 dark:text-zinc-400 leading-relaxed">
                   While the average human gene is ~30kb, some like **DMD (Dystrophin)** span over 2 megabases, taking the cell several hours to transcribe a single molecule.
                 </p>
              </div>
           </div>
           
           <div className="flex gap-4 p-6 bg-slate-50 dark:bg-zinc-900/50 rounded-2xl border border-slate-200/50 dark:border-zinc-800/50">
              <div className="h-10 w-10 shrink-0 rounded-xl bg-emerald-100 dark:bg-emerald-900/30 flex items-center justify-center">
                 <Layers className="h-5 w-5 text-emerald-600 dark:text-emerald-400" />
              </div>
              <div>
                 <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-tight mb-1">Exonic Density</h4>
                 <p className="text-[11px] text-slate-500 dark:text-zinc-400 leading-relaxed">
                   Most human exons are remarkably short (~150bp), while introns can be hundreds of kilobases long, requiring precise machinery for accurate removal.
                 </p>
              </div>
           </div>
        </div>

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
