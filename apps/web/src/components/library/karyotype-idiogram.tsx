"use client"

import React from 'react';
import { Chromosome, CentromereRegion } from '@/types/api';
import { cn, formatMb } from '@/lib/utils';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';

interface KaryotypeIdiogramProps {
  chromosomes: Chromosome[];
  centromeres?: CentromereRegion[];
  onSelect?: (chrom: string) => void;
  className?: string;
}

export function KaryotypeIdiogram({ chromosomes, centromeres, onSelect, className }: KaryotypeIdiogramProps) {
  const maxLen = Math.max(...chromosomes.map(c => c.length));

  return (
    <TooltipProvider>
      <div className={cn("flex items-start justify-between w-full h-full min-h-[300px] px-2", className)}>
        {chromosomes.map((chrom) => {
          const heightPercent = (chrom.length / maxLen) * 100;
          const centromere = centromeres?.find(c => c.chrom === chrom.name);
          
          const centromereTop = centromere 
            ? (centromere.start / chrom.length) * 100 
            : 50;

          return (
            <div 
              key={chrom.name} 
              className="flex flex-col items-center h-full group"
              style={{ width: `${100 / chromosomes.length}%` }}
            >
              <Tooltip>
                <TooltipTrigger asChild>
                  <div 
                    onClick={() => onSelect?.(chrom.name)}
                    className="relative w-2.5 md:w-3 bg-slate-200 dark:bg-zinc-800 rounded-full cursor-pointer hover:ring-2 hover:ring-primary/50 transition-all overflow-hidden border border-slate-300 dark:border-zinc-700 shadow-sm"
                    style={{ height: `${heightPercent}%` }}
                  >
                    {/* Simulated G-Bands */}
                    <div className="absolute inset-0 opacity-[0.03] dark:opacity-[0.07] pointer-events-none bg-[repeating-linear-gradient(0deg,transparent,transparent_2px,black_2px,black_4px)]" />
                    
                    {/* Centromere - Visualized as a pinch/line */}
                    <div 
                      className="absolute left-0 right-0 h-[2px] bg-slate-400 dark:bg-slate-500 z-10 opacity-50"
                      style={{ top: `${centromereTop}%` }}
                    />
                    
                    {/* Morphology coloring */}
                    <div 
                      className="absolute inset-0 bg-gradient-to-b from-transparent via-primary/5 to-transparent"
                    />
                  </div>
                </TooltipTrigger>
                <TooltipContent side="top" className="bg-slate-950 text-white border-white/10 shadow-2xl">
                  <div className="text-[10px] space-y-1.5 p-1">
                    <div className="flex justify-between gap-4 border-b border-white/10 pb-1 mb-1">
                      <span className="font-bold uppercase tracking-widest">Scaffold</span>
                      <span className="font-mono">{chrom.name}</span>
                    </div>
                    <div className="flex justify-between gap-4">
                      <span className="text-slate-400">Length</span>
                      <span className="font-mono">{formatMb(chrom.length)} Mb</span>
                    </div>
                    <div className="flex justify-between gap-4">
                      <span className="text-slate-400">GC Content</span>
                      <span className="font-mono">{chrom.gc_content.toFixed(2)}%</span>
                    </div>
                    {centromere && (
                      <div className="flex justify-between gap-4 text-emerald-400 pt-1">
                        <span className="uppercase text-[8px] font-bold">Centromere</span>
                        <span className="font-mono">Detected</span>
                      </div>
                    )}
                  </div>
                </TooltipContent>
              </Tooltip>
              <span className="mt-3 text-[9px] font-bold text-slate-400 dark:text-zinc-500 uppercase tracking-tighter truncate w-full text-center group-hover:text-primary transition-colors">
                {chrom.name.replace('chr', '')}
              </span>
            </div>
          );
        })}
      </div>
    </TooltipProvider>
  );
}
