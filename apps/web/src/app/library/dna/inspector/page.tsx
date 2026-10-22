"use client"

import React, { useState, useMemo } from 'react';
import { useDNAStats } from '@/hooks/use-dna-stats';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from '@/components/ui/select';
import { dnaApi } from '@/lib/api';
import { useQuery } from '@tanstack/react-query';
import { Loader2, Search, MapPin, Activity, Database, Copy, Check, FileText } from 'lucide-react';
import { formatNumber, alignToBaseWindow } from '@/lib/utils';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

export default function RegionInspectorPage() {
  const { activeGenome, primaryChroms } = useDNAStats();
  const [chrom, setChrom] = useState('chr1');
  const [start, setStart] = useState('1000000');
  const [end, setEnd] = useState('1050000');
  const [copied, setCopied] = useState(false);

  const queryStart = parseInt(start);
  const queryEnd = parseInt(end);
  const isValid = !isNaN(queryStart) && !isNaN(queryEnd) && queryStart >= 0 && queryEnd > queryStart && (queryEnd - queryStart) <= 100000;

  const { data: sequence, isLoading: seqLoading, refetch: fetchSeq } = useQuery({
    queryKey: ['sequence', activeGenome, chrom, queryStart, queryEnd],
    queryFn: () => dnaApi.getSequence(activeGenome, chrom, queryStart, queryEnd),
    enabled: false,
  });

  const { data: windows, refetch: fetchWindows } = useQuery({
    queryKey: ['windows', activeGenome, chrom, queryStart, queryEnd],
    queryFn: () => dnaApi.getSequenceWindows(activeGenome, chrom, { 
      start: queryStart, 
      end: queryEnd, 
      step: alignToBaseWindow((queryEnd - queryStart) / 100) 
    }),
    enabled: false,
  });

  const handleInspect = () => {
    if (isValid) {
      fetchSeq();
      fetchWindows();
    }
  };

  const copySequence = () => {
    if (sequence) {
      navigator.clipboard.writeText(sequence);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const gcContent = useMemo(() => {
    if (!sequence) return 0;
    // GC / (A+C+G+T): N and other symbols are excluded from the denominator (counted separately below)
    const s = sequence.toUpperCase();
    const g = s.split('G').length - 1;
    const c = s.split('C').length - 1;
    const a = s.split('A').length - 1;
    const t = s.split('T').length - 1;
    const acgt = a + c + g + t;
    return acgt > 0 ? ((g + c) / acgt) * 100 : 0;
  }, [sequence]);

  return (
    <LibraryShell 
      title="Region Inspector" 
      subtitle="Sequence, base counts and windowed GC for a range of at most 100 kb (0-based, half-open)."
    >
      <div className="space-y-12 pb-20">
        
        {/* QUERY PANEL */}
        <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm">
           <div className="grid grid-cols-1 md:grid-cols-4 gap-6 items-end">
              <div className="space-y-2">
                 <label className="text-[10px] font-bold text-slate-400 uppercase tracking-widest px-1">Chromosome</label>
                 <Select value={chrom} onValueChange={setChrom}>
                    <SelectTrigger className="h-10">
                       <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                       {primaryChroms.map(c => <SelectItem key={c.name} value={c.name}>{c.name}</SelectItem>)}
                    </SelectContent>
                 </Select>
              </div>
              <div className="space-y-2">
                 <label className="text-[10px] font-bold text-slate-400 uppercase tracking-widest px-1">Start Position</label>
                 <Input 
                   type="number" 
                   value={start} 
                   onChange={(e) => setStart(e.target.value)}
                   className="h-10 font-mono"
                 />
              </div>
              <div className="space-y-2">
                 <label className="text-[10px] font-bold text-slate-400 uppercase tracking-widest px-1">End Position</label>
                 <Input 
                   type="number" 
                   value={end} 
                   onChange={(e) => setEnd(e.target.value)}
                   className="h-10 font-mono"
                 />
              </div>
              <Button 
                onClick={handleInspect} 
                disabled={!isValid || seqLoading}
                className="h-10 gap-2 font-bold"
              >
                 {seqLoading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Search className="h-4 w-4" />}
                 Inspect Region
              </Button>
           </div>
           {!isValid && (
             <p className="mt-3 text-[10px] text-orange-500 font-medium italic">
                Please enter a valid range (max 100kb).
             </p>
           )}
        </Card>

        {/* RESULTS */}
        {sequence && (
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-12 animate-in fade-in slide-in-from-bottom-4 duration-500">
             
             {/* STATS COLUMN */}
             <div className="space-y-6">
                <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
                   <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest mb-6 flex items-center gap-2">
                      <Activity className="h-4 w-4 text-primary" />
                      Local Metrics
                   </h4>
                   <div className="space-y-6">
                      <div className="flex items-center justify-between border-b border-slate-100 dark:border-zinc-900 pb-4">
                         <span className="text-xs text-slate-500">Coordinate Range</span>
                         <span className="text-xs font-mono font-bold text-slate-900 dark:text-zinc-100">
                           {chrom}:{formatNumber(queryStart)}–{formatNumber(queryEnd)}
                         </span>
                      </div>
                      <div className="flex items-center justify-between border-b border-slate-100 dark:border-zinc-900 pb-4">
                         <span className="text-xs text-slate-500">Window Length</span>
                         <span className="text-xs font-mono font-bold text-slate-900 dark:text-zinc-100">
                           {formatNumber(sequence.length)} bp
                         </span>
                      </div>
                      <div className="flex items-center justify-between border-b border-slate-100 dark:border-zinc-900 pb-4">
                         <span className="text-xs text-slate-500">GC Content</span>
                         <span className="text-xs font-mono font-bold text-cyan-500">
                           {gcContent.toFixed(2)}%
                         </span>
                      </div>
                   </div>
                </Card>

                <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
                   <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest mb-6 flex items-center gap-2">
                      <FileText className="h-4 w-4 text-emerald-500" />
                      Base Counts
                   </h4>
                   <div className="grid grid-cols-2 gap-4">
                      {['A', 'C', 'G', 'T', 'N'].map(base => (
                        <div key={base} className="p-3 bg-slate-50 dark:bg-zinc-900 rounded-xl border border-slate-200 dark:border-zinc-800">
                           <div className="text-[10px] font-bold text-slate-400 mb-1">{base}</div>
                           <div className="text-sm font-bold text-slate-900 dark:text-zinc-100 font-mono">
                              {formatNumber(sequence.toUpperCase().split(base).length - 1)}
                           </div>
                        </div>
                      ))}
                   </div>
                </Card>
             </div>

             {/* SEQUENCE COLUMN */}
             <div className="lg:col-span-2 space-y-6">
                <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
                   <div className="flex items-center justify-between mb-6">
                      <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                         <Database className="h-4 w-4 text-blue-500" />
                         Sequence Extraction
                      </h4>
                      <Button 
                        variant="outline" 
                        size="sm" 
                        onClick={copySequence}
                        className="h-8 gap-2 text-[10px] font-bold uppercase"
                      >
                         {copied ? <Check className="h-3 w-3 text-emerald-500" /> : <Copy className="h-3 w-3" />}
                         {copied ? 'Copied' : 'Copy FASTA'}
                      </Button>
                   </div>

                   <div className="bg-slate-50 dark:bg-zinc-950 p-6 rounded-2xl border border-slate-200 dark:border-zinc-800 font-mono text-[11px] break-all leading-relaxed max-h-[400px] overflow-auto custom-scrollbar text-slate-600 dark:text-zinc-400">
                      <div className="text-slate-400 mb-4 font-bold uppercase tracking-widest border-b border-slate-200 dark:border-zinc-800 pb-2">
                         &gt;{activeGenome}_{chrom}_{queryStart}_{queryEnd}
                      </div>
                      {sequence}
                   </div>
                </Card>

                {windows && (
                  <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950">
                     <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest mb-6 flex items-center gap-2">
                        <Activity className="h-4 w-4 text-cyan-500" />
                        Windowed Composition
                     </h4>
                     <div className="overflow-auto custom-scrollbar max-h-[300px]">
                        <Table>
                           <TableHeader>
                              <TableRow>
                                 <TableHead className="text-[10px] uppercase">Window start (bp)</TableHead>
                                 <TableHead className="text-[10px] uppercase">GC %</TableHead>
                                 <TableHead className="text-[10px] uppercase">GC skew (%)</TableHead>
                              </TableRow>
                           </TableHeader>
                           <TableBody>
                              {windows.map((w, i) => (
                                <TableRow key={i}>
                                   <TableCell className="font-mono text-[10px]">{formatNumber(w.start)}</TableCell>
                                   <TableCell className="font-mono text-[10px] font-bold text-cyan-500">{(w.gc_content).toFixed(1)}%</TableCell>
                                   <TableCell className="font-mono text-[10px]">{w.gc_skew != null ? `${w.gc_skew.toFixed(2)}%` : 'n/a'}</TableCell>
                                </TableRow>
                              ))}
                           </TableBody>
                        </Table>
                     </div>
                  </Card>
                )}
             </div>

          </div>
        )}

        {!sequence && !seqLoading && (
          <div className="flex flex-col items-center justify-center py-24 text-slate-300 dark:text-zinc-800">
             <MapPin className="h-20 w-20 mb-4 opacity-20" />
             <p className="text-sm font-medium italic">Enter coordinates above to begin inspection.</p>
          </div>
        )}

      </div>
    </LibraryShell>
  );
}
