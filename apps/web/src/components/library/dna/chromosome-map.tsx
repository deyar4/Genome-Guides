import React from 'react';
import { Chromosome, CentromereRegion, TelomereHit } from '@/types/api';
import { cn } from '@/lib/utils';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { Badge } from '@/components/ui/badge';

interface ChromosomeMapProps {
  chromosomes: Chromosome[];
  centromeres?: CentromereRegion[];
  telomeres?: TelomereHit[];
  onSelect?: (name: string) => void;
  selectedChrom?: string | null;
}

export function ChromosomeMap({ chromosomes, centromeres, telomeres, onSelect, selectedChrom }: ChromosomeMapProps) {
  const maxLen = Math.max(...chromosomes.map(c => c.length));
  
  return (
    <div className="w-full flex flex-col gap-4 py-4">
      {/* SCALE HEADER */}
      <div className="flex items-center mb-4 px-4">
         <div className="w-24 shrink-0" />
         <div className="flex-1 h-6 relative border-b border-slate-200 dark:border-zinc-800">
            {[0, 0.25, 0.5, 0.75, 1].map(tick => (
               <div key={tick} className="absolute bottom-0 flex flex-col items-center" style={{ left: `${tick * 100}%` }}>
                  <div className="h-2 w-[1px] bg-slate-200 dark:border-zinc-800" />
                  <span className="text-[9px] font-mono text-slate-400 mt-1">
                     {((tick * maxLen) / 1e6).toFixed(0)}M
                  </span>
               </div>
            ))}
         </div>
      </div>

      <div className="space-y-2">
        {chromosomes.map((chrom) => {
          const centromere = centromeres?.find(c => c.chrom === chrom.name);
          const chromTelomeres = telomeres?.filter(t => t.chrom === chrom.name);
          const widthPercent = (chrom.length / maxLen) * 100;
          const isSelected = selectedChrom === chrom.name;
          const isCircular = chrom.name.toLowerCase().includes('m');

          return (
            <div 
              key={chrom.name} 
              className={cn(
                "flex items-center gap-4 group cursor-pointer p-1 rounded-lg transition-colors",
                isSelected ? "bg-primary/5" : "hover:bg-slate-50 dark:hover:bg-zinc-900/50"
              )}
              onClick={() => onSelect?.(chrom.name)}
            >
              {/* LABEL */}
              <div className="w-24 shrink-0 flex items-center justify-between pr-4 border-r border-slate-100 dark:border-zinc-800">
                 <span className={cn(
                   "text-[11px] font-bold uppercase tracking-widest",
                   isSelected ? "text-primary" : "text-slate-500 group-hover:text-slate-900 dark:group-hover:text-zinc-100"
                 )}>
                   {chrom.name.replace('chr', '')}
                 </span>
                 {isSelected && <div className="h-1.5 w-1.5 rounded-full bg-primary animate-pulse" />}
              </div>

              {/* TRACK */}
              <div className="flex-1 relative h-8 flex items-center">
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div 
                        className={cn(
                          "relative h-4 rounded-full transition-all duration-300 flex overflow-hidden ring-offset-2 dark:ring-offset-zinc-950 shadow-sm",
                          isSelected 
                            ? "ring-2 ring-primary bg-primary/20 border-primary/30" 
                            : "bg-slate-100 dark:bg-zinc-900 border border-slate-200 dark:border-zinc-800 hover:border-primary/50",
                          isCircular && "border-dashed border-2 border-emerald-500/50"
                        )}
                        style={{ width: `${widthPercent}%` }}
                      >
                        {/* Centromere Visualization */}
                        {centromere && (
                          <div 
                            className="absolute bg-slate-400 dark:bg-zinc-600 z-20 border-x border-white/20 dark:border-black/20"
                            style={{ 
                              left: `${(centromere.start / chrom.length) * 100}%`,
                              width: `${Math.max(0.5, (centromere.length / chrom.length) * 100)}%`,
                              minWidth: '4px',
                              height: '100%'
                            }}
                          />
                        )}

                        {/* Telomere Visualization */}
                        {chromTelomeres?.map((t, i) => {
                          const telomereStart = t.start;
                          return (
                            <div 
                              key={i}
                              className="absolute bg-emerald-500 z-30"
                              style={{ 
                                left: `${(telomereStart / chrom.length) * 100}%`,
                                width: '2px',
                                height: '100%'
                              }}
                            />
                          );
                        })}
                        
                        {/* GC Gradient Overlay (Subtle) */}
                        <div 
                          className="absolute inset-0 opacity-10 bg-gradient-to-r from-transparent via-primary to-transparent"
                          style={{ left: `${(chrom.gc_content / 100) * 100}%` }}
                        />
                      </div>
                    </TooltipTrigger>
                    <TooltipContent side="top" className="p-4 min-w-[200px] space-y-3 bg-white dark:bg-zinc-900 border-slate-200 dark:border-zinc-800 shadow-xl">
                      <div className="flex items-center justify-between border-b border-slate-100 dark:border-zinc-800 pb-2">
                        <span className="font-bold text-lg text-slate-900 dark:text-zinc-100">{chrom.name}</span>
                        <Badge variant="outline" className="text-[10px] uppercase">{isCircular ? 'Circular' : 'Linear'}</Badge>
                      </div>
                      <div className="grid grid-cols-2 gap-4">
                         <div className="space-y-1">
                            <div className="text-[9px] font-bold text-slate-400 uppercase">Length</div>
                            <div className="text-xs font-mono font-bold text-slate-900 dark:text-zinc-100">{(chrom.length / 1e6).toFixed(2)} Mb</div>
                         </div>
                         <div className="space-y-1">
                            <div className="text-[9px] font-bold text-slate-400 uppercase">GC Content</div>
                            <div className="text-xs font-mono font-bold text-cyan-500">{chrom.gc_content.toFixed(2)}%</div>
                         </div>
                         {centromere && (
                           <div className="space-y-1 col-span-2">
                              <div className="text-[9px] font-bold text-slate-400 uppercase">Centromere</div>
                              <div className="text-xs font-mono text-slate-600 dark:text-zinc-400">{(centromere.start / 1e6).toFixed(1)} – {(centromere.end / 1e6).toFixed(1)} Mb</div>
                           </div>
                         )}
                         {chromTelomeres && chromTelomeres.length > 0 && (
                           <div className="space-y-1 col-span-2">
                              <div className="text-[9px] font-bold text-slate-400 uppercase">Telomere Caps</div>
                              <div className="flex gap-1 flex-wrap">
                                 {chromTelomeres.map((t, idx) => (
                                    <Badge key={idx} variant="secondary" className="text-[8px] px-1.5 py-0 bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400 border-none uppercase">
                                       {t.length.toLocaleString()} bp
                                    </Badge>
                                 ))}
                              </div>
                           </div>
                         )}
                      </div>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
