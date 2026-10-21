"use client"

import React, { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { proteinsApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import { SectionHeader } from '@/components/library/dna/shared';
import { formatNumber } from '@/lib/utils';
import { Activity, Target } from 'lucide-react';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";

const AMINO_ACIDS = [
  { code: 'A', name: 'Alanine' },
  { code: 'C', name: 'Cysteine' },
  { code: 'D', name: 'Aspartic Acid' },
  { code: 'E', name: 'Glutamic Acid' },
  { code: 'F', name: 'Phenylalanine' },
  { code: 'G', name: 'Glycine' },
  { code: 'H', name: 'Histidine' },
  { code: 'I', name: 'Isoleucine' },
  { code: 'K', name: 'Lysine' },
  { code: 'L', name: 'Leucine' },
  { code: 'M', name: 'Methionine' },
  { code: 'N', name: 'Asparagine' },
  { code: 'P', name: 'Proline' },
  { code: 'Q', name: 'Glutamine' },
  { code: 'R', name: 'Arginine' },
  { code: 'S', name: 'Serine' },
  { code: 'T', name: 'Threonine' },
  { code: 'V', name: 'Valine' },
  { code: 'W', name: 'Tryptophan' },
  { code: 'Y', name: 'Tyrosine' },
];

export default function RichProteinsPage() {
  const { activeGenome } = useGenomeStore();
  const [selectedAA, setSelectedAA] = useState<string>('P');
  const [threshold, setThreshold] = useState<number[]>([15]);

  const { data: richProteins, isLoading } = useQuery({
    queryKey: ['rich-proteins', activeGenome, selectedAA, threshold[0]],
    queryFn: () => proteinsApi.getRich(activeGenome, { aa: selectedAA, threshold: threshold[0], limit: 50 }),
  });

  const getAAName = (code: string) => AMINO_ACIDS.find(a => a.code === code)?.name || code;

  return (
    <LibraryShell 
      title="Amino Acid Richness" 
      subtitle="Identify proteins with extreme compositional bias toward specific residues."
    >
      <div className="space-y-12 pb-20">
        
        {/* FILTERS */}
        <div className="flex flex-col md:flex-row gap-8 items-start md:items-center justify-between p-6 bg-slate-50 dark:bg-zinc-900/50 rounded-2xl border border-slate-200 dark:border-zinc-800">
           
           <div className="flex-1 space-y-3 max-w-sm">
              <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-widest px-1 flex items-center gap-2">
                 <Target className="h-3 w-3 text-primary" />
                 Target Amino Acid
              </label>
              <Select value={selectedAA} onValueChange={setSelectedAA}>
                 <SelectTrigger className="w-full bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 h-11">
                    <SelectValue placeholder="Select Amino Acid" />
                 </SelectTrigger>
                 <SelectContent className="max-h-[300px]">
                    {AMINO_ACIDS.map(aa => (
                       <SelectItem key={aa.code} value={aa.code}>
                          <div className="flex items-center gap-2">
                             <Badge variant="outline" className="w-6 justify-center bg-slate-100 dark:bg-zinc-800">{aa.code}</Badge>
                             <span>{aa.name}</span>
                          </div>
                       </SelectItem>
                    ))}
                 </SelectContent>
              </Select>
           </div>

           <div className="flex-1 space-y-5 max-w-sm w-full">
              <div className="flex justify-between items-center px-1">
                 <label className="text-[10px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-widest flex items-center gap-2">
                    <Activity className="h-3 w-3 text-emerald-500" />
                    Enrichment Threshold
                 </label>
                 <span className="text-xs font-bold text-slate-900 dark:text-zinc-100">
                    &gt; {threshold[0]}%
                 </span>
              </div>
              <div className="px-2">
                 <Slider 
                   value={threshold} 
                   onValueChange={setThreshold} 
                   max={50} 
                   min={5} 
                   step={1} 
                   className="py-1"
                 />
              </div>
              <div className="flex justify-between text-[10px] text-slate-400 font-mono px-1">
                 <span>5%</span>
                 <span>50%</span>
              </div>
           </div>

        </div>

        {/* RESULTS */}
        <section>
          <SectionHeader 
            title={`${getAAName(selectedAA)} (${selectedAA})-Rich Proteins`}
            subtitle={`Up to 50 proteins whose most frequent residue is ${getAAName(selectedAA)} at ${threshold[0]}% of the sequence or more, sorted by that percentage (highest first).`}
            badge={`${richProteins?.length || 0} Found`}
          />
          <Card className="overflow-hidden border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm mt-6">
            <Table>
              <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                <TableRow>
                  <TableHead className="w-[100px]">Protein ID</TableHead>
                  <TableHead>Gene</TableHead>
                  <TableHead className="text-right">Length (AA)</TableHead>
                  <TableHead className="text-right">Distinct residues</TableHead>
                  <TableHead className="text-right">{selectedAA} share (count)</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {isLoading ? (
                  [...Array(10)].map((_, i) => (
                     <TableRow key={i}>
                        <TableCell><div className="h-4 w-20 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        <TableCell><div className="h-4 w-32 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        <TableCell><div className="h-4 w-12 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        <TableCell><div className="h-4 w-16 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                        <TableCell><div className="h-4 w-12 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                     </TableRow>
                  ))
                ) : richProteins?.length === 0 ? (
                  <TableRow>
                     <TableCell colSpan={5} className="h-32 text-center text-slate-500">
                        No proteins found meeting this threshold. Try lowering the percentage.
                     </TableCell>
                  </TableRow>
                ) : (
                  richProteins?.map((protein) => {
                     // The API sends the dominant residue's percentage; the sequence field is empty.
                     const pct = protein.dominant_frequency_pct ?? 0;
                     const count = Math.round((pct / 100) * protein.length);
                     const percentage = pct.toFixed(1);

                     return (
                        <TableRow key={protein.protein_id} className="group hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                           <TableCell className="font-mono text-xs font-bold text-slate-500">
                              {protein.protein_id}
                           </TableCell>
                           <TableCell>
                              <div className="flex flex-col">
                                 <span className="font-bold text-slate-900 dark:text-zinc-100">{protein.gene_name || 'not mapped'}</span>
                              </div>
                           </TableCell>
                           <TableCell className="text-right font-mono text-xs font-bold text-slate-600 dark:text-slate-400">
                              {formatNumber(protein.length)}
                           </TableCell>
                           <TableCell className="text-right">
                              {protein.distinct_amino_acids ?? 'n/a'}
                           </TableCell>
                           <TableCell className="text-right">
                              <div className="flex flex-col items-end">
                                 <span className="font-mono text-sm font-black text-primary">{percentage}%</span>
                                 <span className="text-[10px] text-slate-400 font-medium">{formatNumber(count)} residues</span>
                              </div>
                           </TableCell>
                        </TableRow>
                     );
                  })
                )}
              </TableBody>
            </Table>
          </Card>
        </section>

      </div>
    </LibraryShell>
  );
}
