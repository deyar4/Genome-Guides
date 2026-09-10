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
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn, formatNumber } from '@/lib/utils';
import { Target, Zap, Hash, ExternalLink } from 'lucide-react';

export default function ProteinFeaturesPage() {
  const { activeGenome } = useGenomeStore();

  const { data: largest, isLoading: largestLoading } = useQuery({
    queryKey: ['proteins-largest', activeGenome],
    queryFn: () => proteinsApi.getLargest(activeGenome, 20),
  });

  const { data: tracts, isLoading: tractsLoading } = useQuery({
    queryKey: ['proteins-tracts', activeGenome],
    queryFn: () => proteinsApi.getHomopolymerTracts(activeGenome, { min_length: 10, limit: 100 }),
  });

  return (
    <LibraryShell 
      title="Protein Feature Discovery" 
      subtitle="Identification of proteomic outliers: from massive polypeptides to low-complexity homopolymer tracts."
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
                    Polypeptide length is a fundamental property of proteins. The largest proteins, like <span className="text-white font-bold">Titin</span>, 
                    can span tens of thousands of residues and play critical structural roles in the cell.
                 </p>
              </div>

              <Card className="border-slate-200 dark:border-zinc-800 overflow-hidden shadow-sm">
                <Table>
                  <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                    <TableRow>
                      <TableHead>Protein ID</TableHead>
                      <TableHead>Gene</TableHead>
                      <TableHead className="text-right">Length (aa)</TableHead>
                      <TableHead>System</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {largestLoading ? (
                      Array.from({ length: 10 }).map((_, i) => (
                        <TableRow key={i}>
                          <TableCell><div className="h-4 w-32 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell><div className="h-4 w-24 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell className="text-right"><div className="h-4 w-16 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell><div className="h-4 w-20 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                          <TableCell className="text-right"><div className="h-4 w-8 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
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
                               <span className="font-bold text-slate-900 dark:text-zinc-100">{p.gene_name || 'Unnamed'}</span>
                               <span className="text-[10px] font-mono text-slate-400">{p.gene_id}</span>
                            </div>
                          </TableCell>
                          <TableCell className="text-right font-bold text-slate-900 dark:text-zinc-100">
                             <div className="flex items-center justify-end gap-1.5">
                                <Hash className="h-3 w-3 text-slate-400" />
                                {formatNumber(p.length)}
                             </div>
                          </TableCell>
                          <TableCell>
                             <Badge variant="outline" className={cn(
                               "text-[10px] uppercase font-bold border-none",
                               p.is_mitochondrial ? "bg-pink-100 text-pink-700 dark:bg-pink-900/30 dark:text-pink-400" : "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400"
                             )}>
                                {p.is_mitochondrial ? 'Mitochondrial' : 'Nuclear'}
                             </Badge>
                          </TableCell>
                          <TableCell className="text-right">
                             <button className="p-1.5 hover:bg-slate-200 dark:hover:bg-zinc-800 rounded-md transition-colors text-slate-400 hover:text-primary">
                                <ExternalLink className="h-4 w-4" />
                             </button>
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
                       Homopolymer tracts are continuous runs of a single amino acid. Large tracts can be associated with specific structural properties or disease states (e.g., Poly-Q expansions).
                    </p>
                 </div>
              </div>

              <Card className="border-slate-200 dark:border-zinc-800 overflow-hidden shadow-sm">
                <Table>
                  <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                    <TableRow>
                      <TableHead>Residue</TableHead>
                      <TableHead className="text-right">Length</TableHead>
                      <TableHead className="text-right">Position</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
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
                      tracts?.map((t, i) => (
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
                          <TableCell className="text-right text-xs font-mono text-slate-500">
                             {formatNumber(t.start)} - {formatNumber(t.end)}
                          </TableCell>
                          <TableCell className="text-right">
                             <button className="p-1.5 hover:bg-slate-200 dark:hover:bg-zinc-800 rounded-md transition-colors text-slate-400 hover:text-primary">
                                <ExternalLink className="h-4 w-4" />
                             </button>
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
