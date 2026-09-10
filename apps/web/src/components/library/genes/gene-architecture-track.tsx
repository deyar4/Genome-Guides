"use client"

import React from 'react';
import { Transcript } from '@/types/api';
import { cn } from '@/lib/utils';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';

interface GeneArchitectureTrackProps {
  geneStart: number;
  geneEnd: number;
  transcripts: Transcript[];
  strand: string;
}

export function GeneArchitectureTrack({ geneStart, geneEnd, transcripts, strand }: GeneArchitectureTrackProps) {
  const geneLength = geneEnd - geneStart;

  return (
    <div className="w-full space-y-8 py-4">
      {transcripts.map((t) => (
        <div key={t.transcript_id} className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-mono font-bold text-slate-400 uppercase tracking-widest">{t.transcript_id}</span>
            <span className="text-[9px] font-bold text-slate-500 uppercase bg-slate-100 dark:bg-zinc-800 px-1.5 py-0.5 rounded">
              {t.biotype?.replace('_', ' ')}
            </span>
          </div>
          
          <div className="relative h-6 w-full flex items-center group">
            {/* Intron Line */}
            <div className="absolute h-[1px] w-full bg-slate-200 dark:bg-zinc-800 top-1/2 -translate-y-1/2" />
            
            {/* Strand Arrows (Subtle) */}
            <div className="absolute inset-0 flex items-center justify-around pointer-events-none opacity-20">
               {[...Array(10)].map((_, i) => (
                 <div key={i} className={cn(
                   "text-[10px] font-bold",
                   strand === '+' ? "animate-in fade-in" : ""
                 )}>
                   {strand === '+' ? '→' : '←'}
                 </div>
               ))}
            </div>

            {/* Exons */}
            <TooltipProvider>
              {t.exons.map((exon) => {
                const left = ((exon.start - geneStart) / geneLength) * 100;
                const width = ((exon.end - exon.start) / geneLength) * 100;
                
                return (
                  <Tooltip key={exon.exon_number}>
                    <TooltipTrigger asChild>
                      <div 
                        className="absolute h-4 bg-primary rounded-sm border border-primary/20 hover:ring-2 ring-primary/30 transition-all cursor-crosshair z-10"
                        style={{ 
                          left: `${Math.max(0, left)}%`, 
                          width: `${Math.max(0.5, width)}%`,
                          top: '50%',
                          transform: 'translateY(-50%)'
                        }}
                      />
                    </TooltipTrigger>
                    <TooltipContent side="top" className="text-[10px]">
                      <div className="font-bold">Exon {exon.exon_number}</div>
                      <div className="text-slate-500">Range: {exon.start.toLocaleString()} - {exon.end.toLocaleString()}</div>
                      <div className="text-slate-500">Length: {(exon.end - exon.start).toLocaleString()} bp</div>
                    </TooltipContent>
                  </Tooltip>
                );
              })}
            </TooltipProvider>
          </div>
        </div>
      ))}
    </div>
  );
}
