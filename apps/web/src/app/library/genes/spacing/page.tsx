"use client"

import React, { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { genesApi } from '@/lib/api';
import { useDNAStats } from '@/hooks/use-dna-stats';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { SectionHeader } from '@/components/library/dna/shared';
import { formatNumber, cn } from '@/lib/utils';
import { AlignJustify, ArrowRightLeft, Database } from 'lucide-react';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from "@/components/ui/select";

export default function GeneSpacingPage() {
  const { activeGenome } = useGenomeStore();
  const { primaryChroms } = useDNAStats();
  const [selectedChrom, setSelectedChrom] = useState<string>('chr1');

  const { data: spacingResp, isLoading: spacingLoading } = useQuery({
    queryKey: ['gene-spacing', activeGenome, selectedChrom],
    queryFn: () => genesApi.getSpacing(activeGenome, selectedChrom),
    enabled: !!selectedChrom,
  });

  const { data: overlapsResp, isLoading: overlapsLoading } = useQuery({
    queryKey: ['gene-overlaps', activeGenome, selectedChrom],
    queryFn: () => genesApi.getOverlaps(activeGenome, selectedChrom),
    enabled: !!selectedChrom,
  });
  const spacing = spacingResp?.items;
  const overlaps = overlapsResp?.items;

  return (
    <LibraryShell 
      title="Spacing & Overlaps" 
      subtitle="Distances between adjacent genes and overlapping gene pairs."
    >
      <div className="space-y-12 pb-20">
        
        {/* FILTERS */}
        <div className="flex items-center justify-between">
           <h4 className="font-bold text-slate-900 dark:text-zinc-100 text-sm uppercase tracking-widest flex items-center gap-2">
             <Database className="h-4 w-4 text-primary" />
             Scaffold Selection
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

        <Tabs defaultValue="spacing" className="w-full">
          <TabsList className="mb-6 bg-slate-100 dark:bg-zinc-900 p-1 rounded-xl h-11">
            <TabsTrigger value="spacing" className="rounded-lg px-6 data-[state=active]:bg-white dark:data-[state=active]:bg-zinc-800 data-[state=active]:shadow-sm">Intergenic Spacing</TabsTrigger>
            <TabsTrigger value="overlaps" className="rounded-lg px-6 data-[state=active]:bg-white dark:data-[state=active]:bg-zinc-800 data-[state=active]:shadow-sm">Gene Overlaps</TabsTrigger>
          </TabsList>
          
          <TabsContent value="spacing" className="mt-0 space-y-6">
             <SectionHeader 
               title="Gene Deserts & Clusters" 
               subtitle={`The 100 largest gaps between adjacent genes on the selected chromosome, sorted by distance (${spacingResp ? spacingResp.total.toLocaleString() : "..."} gaps on this chromosome; the API returns at most the 1,000 largest).`}
             />
             <Card className="overflow-hidden border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
               <Table>
                 <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                   <TableRow>
                     <TableHead>Gene 1</TableHead>
                     <TableHead className="w-[50px]"></TableHead>
                     <TableHead>Gene 2</TableHead>
                     <TableHead>Location</TableHead>
                     <TableHead className="text-right">Distance (bp)</TableHead>
                   </TableRow>
                 </TableHeader>
                 <TableBody>
                   {spacingLoading ? (
                     [...Array(10)].map((_, i) => <TableRow key={i}><TableCell colSpan={5}><div className="h-10 w-full bg-slate-50 dark:bg-zinc-900/50 animate-pulse rounded" /></TableCell></TableRow>)
                   ) : spacing?.length === 0 ? (
                     <TableRow>
                       <TableCell colSpan={5} className="h-32 text-center text-slate-500">No spacing data available.</TableCell>
                     </TableRow>
                   ) : (
                     spacing?.slice(0, 100).map((s, i) => (
                       <TableRow key={i} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                         <TableCell className="font-mono text-xs text-slate-600 dark:text-slate-400">{s.gene_id_1}</TableCell>
                         <TableCell><AlignJustify className="h-4 w-4 text-slate-300" /></TableCell>
                         <TableCell className="font-mono text-xs text-slate-600 dark:text-slate-400">{s.gene_id_2}</TableCell>
                         <TableCell className="font-mono text-[10px] text-slate-500">{s.chrom}:{formatNumber(s.start)}-{formatNumber(s.end)}</TableCell>
                         <TableCell className="text-right font-mono text-xs font-bold text-blue-500">{formatNumber(s.distance)}</TableCell>
                       </TableRow>
                     ))
                   )}
                 </TableBody>
               </Table>
             </Card>
          </TabsContent>

          <TabsContent value="overlaps" className="mt-0 space-y-6">
             <SectionHeader 
               title="Overlapping Genes" 
               subtitle={`Gene pairs that physically overlap on the same or opposite strands: the first 100 in order of gene ID (the API returns at most 1,000 per request; total on this chromosome: ${overlapsResp ? overlapsResp.total.toLocaleString() : "..."}), not ranked.`}
             />
             <Card className="overflow-hidden border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
               <Table>
                 <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                   <TableRow>
                     <TableHead>Gene 1</TableHead>
                     <TableHead className="w-[50px]"></TableHead>
                     <TableHead>Gene 2</TableHead>
                     <TableHead>Overlap Type</TableHead>
                     <TableHead>Scaffold</TableHead>
                   </TableRow>
                 </TableHeader>
                 <TableBody>
                   {overlapsLoading ? (
                     [...Array(10)].map((_, i) => <TableRow key={i}><TableCell colSpan={5}><div className="h-10 w-full bg-slate-50 dark:bg-zinc-900/50 animate-pulse rounded" /></TableCell></TableRow>)
                   ) : overlaps?.length === 0 ? (
                     <TableRow>
                       <TableCell colSpan={5} className="h-32 text-center text-slate-500">No overlapping genes found.</TableCell>
                     </TableRow>
                   ) : (
                     overlaps?.slice(0, 100).map((o, i) => (
                       <TableRow key={i} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                         <TableCell className="font-mono text-xs text-slate-600 dark:text-slate-400">{o.gene_id_1}</TableCell>
                         <TableCell><ArrowRightLeft className="h-4 w-4 text-slate-300" /></TableCell>
                         <TableCell className="font-mono text-xs text-slate-600 dark:text-slate-400">{o.gene_id_2}</TableCell>
                         <TableCell>
                           <Badge variant="secondary" className={cn(
                             "text-[10px] uppercase font-bold border-none",
                             o.overlap_type === 'same_strand' ? "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400" : "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
                           )}>
                              {o.overlap_type === 'same_strand' ? 'same strand' : 'opposite strand'}{o.contained ? ' · contained' : ''}
                           </Badge>
                         </TableCell>
                         <TableCell className="font-mono text-xs text-slate-500">{o.chrom}</TableCell>
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
