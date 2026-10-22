"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { genesApi } from '@/lib/api';
import { useGenomeStore } from '@/store/use-genome-store';
import { Card } from '@/components/ui/card';
import Link from 'next/link';
import { cn, geneHref } from '@/lib/utils';
import { Loader2, ArrowLeft, ArrowRight } from 'lucide-react';

interface NeighborhoodBrowserProps {
  chrom: string;
  start: number;
  end: number;
  centerGeneId: string;
}

export function NeighborhoodBrowser({ chrom, start, end, centerGeneId }: NeighborhoodBrowserProps) {
  const { activeGenome } = useGenomeStore();
  
  // Expand range to find neighbors (e.g., +/- 100kb)
  const expandedStart = Math.max(0, start - 100000);
  const expandedEnd = end + 100000;

  const { data: neighbors, isLoading } = useQuery({
    queryKey: ['neighborhood', activeGenome, chrom, expandedStart, expandedEnd],
    queryFn: () => genesApi.getRegion(activeGenome, chrom, expandedStart, expandedEnd),
  });

  const sortedNeighbors = useMemo(() => {
    if (!neighbors) return [];
    return [...neighbors].sort((a, b) => a.start - b.start);
  }, [neighbors]);

  return (
    <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm relative min-h-[150px]">
       <div className="flex items-center justify-between mb-8">
          <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
             <ArrowLeft className="h-3 w-3" />
             Neighboring Genomic Loci
             <ArrowRight className="h-3 w-3" />
          </h4>
          <span className="text-[10px] font-bold text-slate-400 uppercase tracking-tight">+/- 100kb Window</span>
       </div>

       {isLoading ? (
         <div className="absolute inset-0 flex items-center justify-center bg-white/50 dark:bg-zinc-950/50 backdrop-blur-sm z-10 rounded-xl">
            <Loader2 className="h-5 w-5 animate-spin text-primary" />
         </div>
       ) : null}

       <div className="relative h-12 w-full flex items-center gap-1">
          {/* Timeline Line */}
          <div className="absolute h-[2px] w-full bg-slate-100 dark:bg-zinc-900 top-1/2 -translate-y-1/2 z-0" />
          
          <div className="relative z-10 w-full flex items-center justify-between gap-4">
             {sortedNeighbors.map((gene) => {
                const isCenter = gene.gene_id === centerGeneId;
                
                return (
                  <Link 
                    key={gene.gene_id} 
                    href={geneHref(gene.gene_id, gene.chrom)}
                    className={cn(
                      "flex flex-col items-center gap-2 transition-all duration-300",
                      isCenter ? "scale-110" : "opacity-40 hover:opacity-100 hover:scale-105"
                    )}
                  >
                     <div className={cn(
                       "h-3 rounded-full border shadow-sm",
                       isCenter ? "w-12 bg-primary border-primary" : "w-8 bg-slate-300 dark:bg-zinc-700 border-slate-400/20"
                     )} />
                     <div className="flex flex-col items-center">
                        <span className={cn(
                          "text-[10px] font-bold truncate max-w-[80px]",
                          isCenter ? "text-slate-900 dark:text-zinc-100" : "text-slate-500"
                        )}>
                          {gene.gene_name || gene.gene_id.slice(0, 8)}
                        </span>
                        {isCenter && (
                          <div className="h-1 w-1 rounded-full bg-primary mt-1" />
                        )}
                     </div>
                  </Link>
                );
             })}
          </div>
       </div>
    </Card>
  );
}
