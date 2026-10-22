"use client"

import React from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { proteinsApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { 
  Table, 
  TableBody, 
  TableCell, 
  TableHead, 
  TableHeader, 
  TableRow 
} from "@/components/ui/table";
import { Card } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { formatNumber } from '@/lib/utils';
import { Target, Zap, Hash } from 'lucide-react';

export default function ProteinFeaturesPage() {
  const { activeGenome } = useGenomeStore();

  const { data: largest, isLoading: largestLoading } = useQuery({
    queryKey: ['proteins-largest', activeGenome],
    queryFn: () => proteinsApi.getLargest(activeGenome, 20),
  });

  const { data: tracts, isLoading: tractsLoading } = useQuery({
    queryKey: ['proteins-tracts', activeGenome],
    queryFn: () => proteinsApi.getHomopolymerTracts(activeGenome),
  });

  // The API returns every tract of length >= 5 (min_length / limit are ignored), so select client-side.
  const topTracts = React.useMemo(
    () => [...(tracts ?? [])].filter(t => t.length >= 10).sort((a, b) => b.length - a.length).slice(0, 100),
    [tracts]
  );

  return (
    <LibraryShell 
      title="Protein Feature Discovery" 
      subtitle="Longest protein records and longest single-residue tracts."
    >
      <div className="space-y-8 pb-20">
        
        <Tabs defaultValue="largest" className="w-full">
           <TabsList className="grid w-full max-w-[400px] grid-cols-2 mb-8 h-12 bg-slate-100 dark:bg-zinc-900 rounded-2xl p-1">
              <TabsTrigger value="largest" className="rounded-xl data-[state=active]:bg-white dark:data-[state=active]:bg-zinc-800 data-[state=active]:shadow-sm">
                 Largest Proteins
              </TabsTrigger>
              <TabsTrigger value="tracts" className="rounded-xl data-[state=active]:bg-white dark:data-[state=active]:bg-zinc-800 data-[state=active]:shadow-sm">
                 Homopolymer Tracts
              </TabsTrigger>
           </TabsList>

           <TabsContent value="largest" className="space-y-6 outline-none">
              <div className="p-6 bg-slate-900 text-slate-300 rounded-2xl border border-slate-800 flex gap-6 items-center">
                 <div className="h-10 w-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
                    <Target className="h-5 w-5 text-primary" />
                 </div>
                 <p className="text-sm leading-relaxed">
                    Protein records sorted by length, longest first (top 20).
                 </p>
              </div>

              <Card className="border-slate-200 dark:border-zinc-800 overflow-hidden shadow-sm">
                <Table>
                  <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                    <TableRow>
                      <TableHead>Protein ID</TableHead>
                      <TableHead>Gene</TableHead>
                      <TableHead className="text-right">Length (aa)</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {largestLoading ? (
                      Array.from({ length: 10 }).map((_, i) => (
                        <TableRow key={i}>
                          <TableCell><div className="h-4 w-32 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell><div className="h-4 w-24 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell className="text-right"><div className="h-4 w-16 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        </TableRow>
                      ))
                    ) : (
                      largest?.map((p) => (
                        <TableRow key={p.protein_id} className="group hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                          <TableCell className="font-mono text-[10px] font-bold text-slate-900 dark:text-zinc-100">
                            {p.protein_id}
                          </TableCell>
                          <TableCell>
                            <div className="flex flex-col">
                               <span className="font-bold text-slate-900 dark:text-zinc-100">{p.gene_name || 'not mapped'}</span>
                            </div>
                          </TableCell>
                          <TableCell className="text-right font-bold text-slate-900 dark:text-zinc-100">
                             <div className="flex items-center justify-end gap-1.5">
                                <Hash className="h-3 w-3 text-slate-400" />
                                {formatNumber(p.length)}
                             </div>
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </Card>
           </TabsContent>

           <TabsContent value="tracts" className="space-y-6 outline-none">
              <div className="p-6 bg-blue-50 dark:bg-blue-900/20 rounded-2xl border border-blue-100 dark:border-blue-800 flex gap-6 items-start">
                 <div className="h-10 w-10 rounded-xl bg-blue-500/10 flex items-center justify-center shrink-0">
                    <Zap className="h-5 w-5 text-blue-600 dark:text-blue-400" />
                 </div>
                 <div className="space-y-2">
                    <h4 className="font-bold text-blue-900 dark:text-blue-100">Low Complexity Sequences</h4>
                    <p className="text-sm text-blue-700/80 dark:text-blue-300/80 leading-relaxed">
                       Homopolymer tracts are continuous runs of a single amino acid. Shown: the 100 longest tracts of length 10 or more, sorted by length.
                    </p>
                 </div>
              </div>

              <Card className="border-slate-200 dark:border-zinc-800 overflow-hidden shadow-sm">
                <Table>
                  <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                    <TableRow>
                      <TableHead>Residue</TableHead>
                      <TableHead className="text-right">Length</TableHead>
                      <TableHead>Protein</TableHead>
                      <TableHead className="text-right">Position (residues)</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {tractsLoading ? (
                      Array.from({ length: 10 }).map((_, i) => (
                        <TableRow key={i}>
                          <TableCell><div className="h-4 w-12 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell className="text-right"><div className="h-4 w-12 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell className="text-right"><div className="h-4 w-24 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell className="text-right"><div className="h-4 w-8 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        </TableRow>
                      ))
                    ) : (
                      topTracts.map((t, i) => (
                        <TableRow key={i} className="group hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                          <TableCell>
                             <div className="flex items-center gap-3">
                                <div className="h-8 w-8 rounded bg-primary/10 flex items-center justify-center font-bold text-primary">
                                   {t.amino_acid}
                                </div>
                                <span className="text-xs text-slate-500 font-medium">Poly-{t.amino_acid}</span>
                             </div>
                          </TableCell>
                          <TableCell className="text-right font-bold text-slate-900 dark:text-zinc-100">
                             {t.length} residues
                          </TableCell>
                          <TableCell className="text-xs font-mono text-slate-500">{t.protein_id}</TableCell>
                          <TableCell className="text-right text-xs font-mono text-slate-500">
                             {formatNumber(t.start)} - {formatNumber(t.end)}
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </Card>
           </TabsContent>
        </Tabs>

      </div>
    </LibraryShell>
  );
}
