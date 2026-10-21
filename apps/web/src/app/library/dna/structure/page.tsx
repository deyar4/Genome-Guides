"use client"

import React, { useMemo } from 'react';
import { useDNAStats } from '@/hooks/use-dna-stats';
import { useQuery } from '@tanstack/react-query';
import { dnaApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { SectionHeader } from '@/components/library/dna/shared';
import { GenomicChartContainer } from '@/components/library/dna/genomic-chart-container';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { 
  Shield, 
  Target, 
  Fingerprint, 
  CircleCheck, 
  CircleX, 
  AlertTriangle,
  Map,
} from 'lucide-react';
import { cn, formatNumber } from '@/lib/utils';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

export default function DNAStructurePage() {
  const { 
    telomeres, 
    centromeres, 
    primaryChroms, 
    activeGenome,
    isLoading,
  } = useDNAStats();

  // 1. Structural Integrity Analysis
  const integrityData = useMemo(() => {
    if (!primaryChroms.length) return [];
    
    return primaryChroms.map(c => {
      const isCircular = c.name.toLowerCase().includes('m');
      const ts = telomeres.data?.filter(t => t.chrom === c.name) || [];
      // The API labels every record 't2t_telomeres' (no 5'/3' field), so the end is derived from
      // coordinates: a record starting at 0 is the 5' end, one ending at the chromosome length the 3' end.
      const hasFive = ts.some(t => t.position === 0);
      const hasThree = ts.some(t => t.end === c.length);
      const hasCen = centromeres.data?.some(cen => {
        const cName = c.name.replace('chr', '');
        const cenName = (cen.chrom || '').replace('chr', '');
        return cName === cenName;
      });
      
      let telomereStatus: 'Bi-terminal' | 'Uni-terminal' | 'Missing' | 'N/A' = 'Missing';
      if (isCircular) telomereStatus = 'N/A';
      else if (hasFive && hasThree) telomereStatus = 'Bi-terminal';
      else if (hasFive || hasThree) telomereStatus = 'Uni-terminal';

      return {
        name: c.name,
        telomereStatus,
        hasCentromere: isCircular ? true : hasCen, // Circular don't have classical centromeres
        isComplete: isCircular ? true : (telomereStatus === 'Bi-terminal' && hasCen),
        missing: isCircular ? null : !hasFive ? "5' Cap" : !hasThree ? "3' Cap" : !hasCen ? "Centromere" : null
      };
    });
  }, [primaryChroms, telomeres.data, centromeres.data]);

  const t2tCount = useMemo(() => integrityData.filter(d => d.isComplete).length, [integrityData]);

  // 2. Fetch Additional Structural Data
  // The unrestricted /dna/isochores response is ~170 MB (1.67 M rows), so always request one chromosome.
  const [isoChrom, setIsoChrom] = React.useState<string | undefined>(undefined);
  const isoChromEff = isoChrom ?? primaryChroms[0]?.name;
  const isochores = useQuery({
    queryKey: ['isochores', activeGenome, isoChromEff],
    queryFn: () => dnaApi.getIsochores(activeGenome, isoChromEff),
    enabled: !!isoChromEff,
  });

  const cpgIslands = useQuery({
    queryKey: ['cpg-islands', activeGenome],
    queryFn: () => dnaApi.getCPGIslands(activeGenome),
  });

  // 3. Render
  return (
    <LibraryShell 
      title="Structural Landscape" 
      subtitle="Evaluation of assembly integrity, telomeric caps, and centromeric regions."
    >
      <div className="space-y-12 pb-20">
        
        {/* ASSEMBLY OVERVIEW */}
        <section className="grid grid-cols-1 md:grid-cols-3 gap-6">
           <Card className="p-6 bg-slate-50 dark:bg-zinc-900 border-none shadow-none flex items-center gap-5">
              <div className="h-12 w-12 rounded-2xl bg-emerald-100 dark:bg-emerald-900/30 flex items-center justify-center">
                 <Shield className="h-6 w-6 text-emerald-600 dark:text-emerald-400" />
              </div>
              <div>
                 <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-0.5">T2T Completeness</div>
                 <div className="text-2xl font-mono font-bold text-slate-900 dark:text-zinc-100">{t2tCount} <span className="text-sm font-sans text-slate-400">/ {primaryChroms.length}</span></div>
              </div>
           </Card>

           <Card className="p-6 bg-slate-50 dark:bg-zinc-900 border-none shadow-none flex items-center gap-5">
              <div className="h-12 w-12 rounded-2xl bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center">
                 <Target className="h-6 w-6 text-blue-600 dark:text-blue-400" />
              </div>
              <div>
                 <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-0.5">Centromere Regions</div>
                 <div className="text-2xl font-mono font-bold text-slate-900 dark:text-zinc-100">{centromeres.data?.length || 0}</div>
              </div>
           </Card>

           <Card className="p-6 bg-slate-50 dark:bg-zinc-900 border-none shadow-none flex items-center gap-5">
              <div className="h-12 w-12 rounded-2xl bg-amber-100 dark:bg-amber-900/30 flex items-center justify-center">
                 <Fingerprint className="h-6 w-6 text-amber-600 dark:text-amber-400" />
              </div>
              <div>
                 <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-0.5">Telomeric Hits</div>
                 <div className="text-2xl font-mono font-bold text-slate-900 dark:text-zinc-100">{telomeres.data?.length || 0}</div>
              </div>
           </Card>
        </section>

        {/* INTEGRITY MATRIX */}
        <section>
          <div className="flex items-center justify-between mb-6">
            <SectionHeader 
              title="Structural Integrity Matrix" 
              subtitle="Verification of landmark elements for each primary chromosome scaffold."
            />
            <div className="px-3 py-1 bg-slate-100 dark:bg-zinc-900 rounded-full border border-slate-200 dark:border-zinc-800 text-[10px] font-bold text-slate-500 uppercase tracking-tight flex items-center gap-2">
               <Map className="h-3 w-3" />
               Scaffold Map
            </div>
          </div>

          <Card className="overflow-hidden border-slate-200 dark:border-zinc-800">
            <Table>
              <TableHeader className="bg-slate-50 dark:bg-zinc-900/50">
                <TableRow>
                  <TableHead className="w-48">Chromosome</TableHead>
                  <TableHead>Telomere Status</TableHead>
                  <TableHead>Centromere</TableHead>
                  <TableHead>Integrity</TableHead>
                  <TableHead className="text-right">Missing Landmarks</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {isLoading ? (
                  [...Array(10)].map((_, i) => (
                    <TableRow key={i}>
                      <TableCell><div className="h-4 w-24 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                      <TableCell><div className="h-4 w-32 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                      <TableCell><div className="h-4 w-12 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                      <TableCell><div className="h-4 w-12 bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                      <TableCell className="text-right"><div className="h-4 w-32 ml-auto bg-slate-100 dark:bg-zinc-800 animate-pulse rounded" /></TableCell>
                    </TableRow>
                  ))
                ) : (
                  integrityData.map((row) => (
                    <TableRow key={row.name} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                      <TableCell className="font-bold text-slate-900 dark:text-zinc-100">{row.name}</TableCell>
                      <TableCell>
                         <Badge variant="outline" className={cn(
                           "font-bold uppercase text-[9px] tracking-widest px-2 py-0.5",
                           row.telomereStatus === 'Bi-terminal' ? "bg-emerald-50 text-emerald-700 border-emerald-100 dark:bg-emerald-900/20 dark:text-emerald-400 dark:border-emerald-800" :
                           row.telomereStatus === 'Uni-terminal' ? "bg-amber-50 text-amber-700 border-amber-100 dark:bg-amber-900/20 dark:text-amber-400 dark:border-amber-800" :
                           "bg-red-50 text-red-700 border-red-100 dark:bg-red-900/20 dark:text-red-400 dark:border-red-800"
                         )}>
                           {row.telomereStatus}
                         </Badge>
                      </TableCell>
                      <TableCell>
                         {row.hasCentromere ? (
                           <CircleCheck className="h-4 w-4 text-emerald-500" />
                         ) : (
                           <div className="flex items-center gap-1.5 text-slate-300 dark:text-zinc-700">
                             <CircleX className="h-4 w-4" />
                             <span className="text-[9px] font-bold opacity-50 italic">NOT ANNOTATED</span>
                           </div>
                         )}
                      </TableCell>
                      <TableCell>
                         {row.isComplete ? (
                           <Badge className="bg-emerald-500 hover:bg-emerald-600 text-white border-none text-[9px] font-bold px-2">COMPLETE</Badge>
                         ) : (
                           <Badge variant="secondary" className="text-[9px] font-bold px-2 opacity-50">PARTIAL</Badge>
                         )}
                      </TableCell>
                      <TableCell className="text-right">
                         {row.missing ? (
                           <div className="flex items-center justify-end gap-1.5 text-[10px] font-bold text-amber-600 dark:text-amber-400 uppercase tracking-tight">
                             <AlertTriangle className="h-3 w-3" />
                             {row.missing} Missing
                           </div>
                         ) : (
                           <span className="text-[10px] font-bold text-slate-400 uppercase tracking-tight">Verified Landmarked</span>
                         )}
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </Card>
        </section>

        {/* ADDITIONAL FEATURES: ISOCHORES & CPG */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-12">
           <GenomicChartContainer
             title="GC-class runs (1 kb windows)"
             subtitle="Bernardi's classification of chromosomal segments by GC content."
             isLoading={isochores.isLoading}
             isError={isochores.isError}
             onRetry={() => isochores.refetch()}
             height="h-[300px]"
           >
              <div className="space-y-4">
                 <div className="text-[11px] text-slate-500 leading-relaxed italic mb-4">
                   Runs of consecutive 1 kb windows in the same GC class: L1 (&lt;37%), L2 (37-41%), H1 (41-46%), H2 (46-53%), H3 (&ge;53% GC). These are not literature isochores, which are defined on much larger windows.
                 </div>
                 <Select value={isoChromEff} onValueChange={setIsoChrom}>
                   <SelectTrigger className="w-32 h-8 text-xs"><SelectValue placeholder="Chromosome" /></SelectTrigger>
                   <SelectContent>
                     {primaryChroms.map(c => <SelectItem key={c.name} value={c.name}>{c.name}</SelectItem>)}
                   </SelectContent>
                 </Select>
                 <div className="text-[10px] text-slate-500">First 10 runs by start position on {isoChromEff} ({isochores.data ? formatNumber(isochores.data.length) : '...'} runs in total), not ranked.</div>
                 <div className="flex items-center justify-between text-[10px] font-bold text-slate-400 uppercase tracking-widest px-2">
                    <span>Region [start, end) bp</span>
                    <span>Class</span>
                    <span className="text-right">GC %</span>
                 </div>
                 <div className="space-y-2 max-h-[180px] overflow-y-auto custom-scrollbar pr-2">
                    {isochores.data?.slice(0, 10).map((i, idx) => (
                      <div key={idx} className="flex items-center justify-between p-2 bg-slate-50 dark:bg-zinc-900 rounded-lg border border-slate-100 dark:border-zinc-800">
                         <span className="font-mono text-xs">{i.chrom}:{formatNumber(i.start)}-{formatNumber(i.end)}</span>
                         <Badge variant="secondary" className="text-[9px] font-bold">{i.isochore_class}</Badge>
                         <span className="font-mono text-xs text-blue-500">{(i.gc_content).toFixed(1)}%</span>
                      </div>
                    ))}
                 </div>
              </div>
           </GenomicChartContainer>

           <GenomicChartContainer
             title="CpG Island Records"
             subtitle="Detected regulatory clusters using Gardiner-Garden & Frommer criteria."
             isLoading={cpgIslands.isLoading}
             isError={cpgIslands.isError}
             onRetry={() => cpgIslands.refetch()}
             height="h-[300px]"
           >
              <div className="flex flex-col h-full">
                 <div className="flex-1 flex flex-col items-center justify-center text-center p-6 bg-slate-50/50 dark:bg-zinc-900/50 rounded-xl border border-dashed border-slate-200 dark:border-zinc-800">
                    <div className="text-4xl font-mono font-bold text-slate-900 dark:text-zinc-100 mb-2">
                       {cpgIslands.data ? `${cpgIslands.data.length.toLocaleString()}${cpgIslands.data.length >= 1000 ? '+' : ''}` : '...'}
                    </div>
                    <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest">CpG records returned</div>
                    {cpgIslands.data && cpgIslands.data.length >= 1000 && (
                      <p className="text-[10px] text-amber-600 mt-2 max-w-[260px]">The API returns at most 1,000 records (200 bp each, adjacent ones not merged), so the genome-wide total is not shown.</p>
                    )}
                    <p className="text-[10px] text-slate-500 mt-4 max-w-[200px]">
                      Highly conserved regions often associated with gene promoters and transcription start sites.
                    </p>
                 </div>
              </div>
           </GenomicChartContainer>
        </div>

        {/* STRUCTURAL LANDMARKS EXPLAINED */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
           <Card className="p-8 bg-slate-50 dark:bg-zinc-900 border-none shadow-none">
              <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest mb-4 flex items-center gap-2">
                 <Shield className="h-4 w-4 text-emerald-500" />
                 Telomeres (Assembly Caps)
              </h4>
              <p className="text-xs text-slate-500 dark:text-zinc-400 leading-relaxed">
                TTAGGG hexameric repeats that protect chromosome ends from degradation and fusion. 
                Bi-terminal status confirms the assembly reaches the physical ends of the linear DNA molecule.
              </p>
           </Card>

           <Card className="p-8 bg-slate-50 dark:bg-zinc-900 border-none shadow-none">
              <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest mb-4 flex items-center gap-2">
                 <Target className="h-4 w-4 text-blue-500" />
                 Centromeres (Segregation Hubs)
              </h4>
              <p className="text-xs text-slate-500 dark:text-zinc-400 leading-relaxed">
                Landmarks for spindle attachment during mitosis. Typically composed of alpha-satellite DNA.
                Annotation confirms the assembly spans the highly repetitive pericentromeric regions.
              </p>
           </Card>
        </div>

      </div>
    </LibraryShell>
  );
}
