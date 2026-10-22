"use client"

import React, { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { rnaApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { 
  Table, 
  TableBody, 
  TableCell, 
  TableHead, 
  TableHeader, 
  TableRow 
} from "@/components/ui/table";
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { formatNumber } from '@/lib/utils';
import { MapPin, Hash, ExternalLink } from 'lucide-react';

export default function RnaClustersPage() {
  const { activeGenome } = useGenomeStore();
  const [rnaType, setRnaType] = useState<string | undefined>(undefined);
  const [chrom, setChrom] = useState<string | undefined>(undefined);

  const { data: summary } = useQuery({
    queryKey: ['rna-summary', activeGenome],
    queryFn: () => rnaApi.getSummary(activeGenome),
  });

  const { data: clusters, isLoading } = useQuery({
    queryKey: ['rna-clusters', activeGenome, rnaType, chrom],
    queryFn: () => rnaApi.getClusters(activeGenome, { rna_type: rnaType === 'all' ? undefined : rnaType, chrom: chrom === 'all' ? undefined : chrom }),
  });

  const rnaTypes = summary?.cluster_summaries.map(s => s.rna_type) || [];
  const chromosomes = Array.from(new Set(clusters?.map(c => c.chrom) || [])).sort();

  return (
    <LibraryShell 
      title="RNA Clusters" 
      subtitle="Gene clusters per RNA type from the pipeline track."
    >
      <div className="space-y-8 pb-20">
        
        {/* FILTERS */}
        <div className="flex flex-wrap gap-4 items-end bg-white dark:bg-zinc-900 p-6 rounded-2xl border border-slate-200 dark:border-zinc-800 shadow-sm">
          <div className="space-y-2">
            <label className="text-[10px] font-bold text-slate-400 uppercase tracking-widest px-1">RNA Type</label>
            <Select value={rnaType || 'all'} onValueChange={setRnaType}>
              <SelectTrigger className="w-[180px]">
                <SelectValue placeholder="All Types" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Types</SelectItem>
                {rnaTypes.map(t => (
                  <SelectItem key={t} value={t}>{t}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <label className="text-[10px] font-bold text-slate-400 uppercase tracking-widest px-1">Chromosome</label>
            <Select value={chrom || 'all'} onValueChange={setChrom}>
              <SelectTrigger className="w-[180px]">
                <SelectValue placeholder="All Chromosomes" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All Chromosomes</SelectItem>
                {chromosomes.map(c => (
                  <SelectItem key={c} value={c}>{c}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          
          <div className="ml-auto">
             <Badge variant="outline" className="px-3 py-1 text-xs font-mono">
                {clusters && clusters.length >= 5000 ? 'First 5,000 clusters (API cap; narrow with the filters)' : `${clusters?.length || 0} Clusters Found`}
             </Badge>
          </div>
        </div>

        {/* CLUSTERS TABLE */}
        <Card className="border-slate-200 dark:border-zinc-800 overflow-hidden shadow-sm">
          <Table>
            <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
              <TableRow>
                <TableHead className="w-[150px]">Cluster ID</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Location</TableHead>
                <TableHead className="text-right">Gene Count</TableHead>
                <TableHead className="text-right">Span (bp)</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {isLoading ? (
                Array.from({ length: 5 }).map((_, i) => (
                  <TableRow key={i}>
                    <TableCell><div className="h-4 w-24 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell><div className="h-4 w-16 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell><div className="h-4 w-32 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell className="text-right"><div className="h-4 w-12 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell className="text-right"><div className="h-4 w-16 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    <TableCell className="text-right"><div className="h-4 w-8 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                  </TableRow>
                ))
              ) : clusters?.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="h-32 text-center text-slate-500">
                    No clusters found matching the filters.
                  </TableCell>
                </TableRow>
              ) : (
                clusters?.map((c) => (
                  <TableRow key={c.cluster_id} className="group hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                    <TableCell className="font-mono text-[10px] font-bold text-slate-900 dark:text-zinc-100">
                      {c.cluster_id}
                    </TableCell>
                    <TableCell>
                      <Badge variant="secondary" className="bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400 border-none">
                        {c.rna_type}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-xs text-slate-600 dark:text-zinc-400">
                      <div className="flex items-center gap-1.5">
                        <MapPin className="h-3 w-3 text-slate-400" />
                        {c.chrom}:{formatNumber(c.start)}-{formatNumber(c.end)}
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-bold text-slate-900 dark:text-zinc-100">
                      <div className="flex items-center justify-end gap-1.5">
                         <Hash className="h-3 w-3 text-slate-400" />
                         {c.size}
                      </div>
                    </TableCell>
                    <TableCell className="text-right text-xs font-mono text-slate-500">
                      {formatNumber(c.end - c.start)} bp
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

      </div>
    </LibraryShell>
  );
}
