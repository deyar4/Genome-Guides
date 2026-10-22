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
import { ArrowRight } from 'lucide-react';

export default function RnaRelationshipsPage() {
  const { activeGenome } = useGenomeStore();

  const { data: result, isLoading } = useQuery({
    queryKey: ['rna-relationships', activeGenome],
    queryFn: () => rnaApi.getRelationships(activeGenome, { limit: 100 }),
  });

  return (
    <LibraryShell 
      title="Nested Genes" 
      subtitle="Genes without an annotated CDS (guests) located entirely inside the span of another gene (host)."
    >
      <div className="space-y-8 pb-20">
        
        {/* RELATIONSHIPS TABLE */}
        <Card className="border-slate-200 dark:border-zinc-800 overflow-hidden shadow-sm">
          <Table>
            <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
              <TableRow>
                <TableHead>Host Gene</TableHead>
                <TableHead className="w-[50px]"></TableHead>
                <TableHead>Guest (no annotated CDS)</TableHead>
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
              ) : result?.total === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-32 text-center text-slate-500">
                    No nested gene without annotated CDS was found in this annotation.
                  </TableCell>
                </TableRow>
              ) : (
                result?.relationships.map((r, i) => (
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
                        {r.relationship_type.replace('_', ' ')}
                      </Badge>
                      <div className="text-[10px] text-slate-400 mt-0.5 font-mono">
                         {r.contig}:{r.guest_start.toLocaleString()}-{r.guest_end.toLocaleString()}
                      </div>
                    </TableCell>
                    <TableCell>
                       <Badge variant="secondary" className={cn(
                         "text-[10px] uppercase font-bold border-none",
                         r.is_same_strand === null ? "bg-slate-100 text-slate-600" : r.is_same_strand ? "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400" : "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
                       )}>
                          {r.is_same_strand === null ? 'Ambiguous' : r.is_same_strand ? 'Same strand' : 'Opposite strand'}
                       </Badge>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </Card>

        {result && (
          <p className="text-xs text-slate-500">
            {result.truncated ? `First ${result.returned.toLocaleString()} of ` : ''}{result.total.toLocaleString()} nested genes without annotated CDS, in coordinate order.
            &quot;No annotated CDS&quot; means no transcript of the gene has a CDS feature in the annotation; it includes lncRNA, tRNA, rRNA, snoRNA and pseudogenes, and the annotation gives no finer class.
          </p>
        )}

      </div>
    </LibraryShell>
  );
}
