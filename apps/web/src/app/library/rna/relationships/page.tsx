"use client"

import React from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { rnaApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { cn } from '@/lib/utils';
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
import { ArrowRight, Info } from 'lucide-react';

export default function RnaRelationshipsPage() {
  const { activeGenome } = useGenomeStore();

  const { data: relationships, isLoading } = useQuery({
    queryKey: ['rna-relationships', activeGenome],
    queryFn: () => rnaApi.getRelationships(activeGenome),
  });

  return (
    <LibraryShell 
      title="Nested Genes" 
      subtitle="Exploration of spatial relationships where non-coding RNAs are located within the boundaries of host genes."
    >
      <div className="space-y-8 pb-20">
        
        <div className="p-6 bg-blue-50 dark:bg-blue-900/20 rounded-2xl border border-blue-100 dark:border-blue-800 flex gap-6 items-start">
           <div className="h-10 w-10 rounded-xl bg-blue-500/10 flex items-center justify-center shrink-0">
              <Info className="h-5 w-5 text-blue-600 dark:text-blue-400" />
           </div>
           <div className="space-y-2">
              <h4 className="font-bold text-blue-900 dark:text-blue-100">Genomic Nesting</h4>
              <p className="text-sm text-blue-700/80 dark:text-blue-300/80 leading-relaxed">
                Many ncRNAs (especially miRNAs, snoRNAs, and snRNAs) are found within the introns or even exons of protein-coding &quot;host&quot; genes. 
                These guest genes are often co-transcribed with their host or regulated by the host&apos;s promoter.
              </p>
           </div>
        </div>

        {/* RELATIONSHIPS TABLE */}
        <Card className="border-slate-200 dark:border-zinc-800 overflow-hidden shadow-sm">
          <Table>
            <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
              <TableRow>
                <TableHead>Host Gene</TableHead>
                <TableHead className="w-[50px]"></TableHead>
                <TableHead>Guest ncRNA</TableHead>
                <TableHead>Relationship</TableHead>
                <TableHead>Strand</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {isLoading ? (
                Array.from({ length: 10 }).map((_, i) => (
                  <TableRow key={i}>
                    <TableCell><div className="h-4 w-32 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell><div className="h-4 w-4 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell><div className="h-4 w-32 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell><div className="h-4 w-20 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell><div className="h-4 w-16 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                  </TableRow>
                ))
              ) : relationships?.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-slate-500">
                    No nesting relationships identified.
                  </TableCell>
                </TableRow>
              ) : (
                relationships?.slice(0, 100).map((r, i) => (
                  <TableRow key={i} className="group hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                    <TableCell>
                      <div className="flex flex-col">
                        <span className="font-bold text-slate-900 dark:text-zinc-100">{r.host_gene_name || 'Unnamed'}</span>
                        <span className="text-[10px] font-mono text-slate-400">{r.host_gene_id}</span>
                      </div>
                    </TableCell>
                    <TableCell>
                       <ArrowRight className="h-3 w-3 text-slate-300 group-hover:text-primary transition-colors" />
                    </TableCell>
                    <TableCell>
                      <div className="flex flex-col">
                        <span className="font-bold text-slate-900 dark:text-zinc-100">{r.guest_gene_name || 'Unnamed'}</span>
                        <span className="text-[10px] font-mono text-slate-400">{r.guest_gene_id}</span>
                      </div>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline" className="text-[10px] uppercase font-bold tracking-tight px-1.5 py-0">
                        {r.relationship_type}
                      </Badge>
                      <div className="text-[10px] text-slate-400 mt-0.5">
                         Guest biotype: <span className="text-slate-500 font-medium">{r.guest_biotype}</span>
                      </div>
                    </TableCell>
                    <TableCell>
                       <Badge variant="secondary" className={cn(
                         "text-[10px] uppercase font-bold border-none",
                         r.is_same_strand ? "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400" : "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
                       )}>
                          {r.is_same_strand ? 'Sense' : 'Antisense'}
                       </Badge>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </Card>

      </div>
    </LibraryShell>
  );
}
