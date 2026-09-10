"use client"

import React, { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { genesApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Search, Loader2, ExternalLink, Info, Database, Fingerprint } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import Link from 'next/link';
import { formatNumber } from '@/lib/utils';
import { useDNAStats } from '@/hooks/use-dna-stats';
import { Button } from '@/components/ui/button';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from '@/components/ui/select';

export default function GeneSearchPage() {
  const { activeGenome } = useGenomeStore();
  const { primaryChroms } = useDNAStats();
  const [query, setQuery] = useState('');
  const [selectedChrom, setSelectedChrom] = useState<string>('all');
  const [selectedBiotype, setSelectedBiotype] = useState<string>('all');

  const { data: biotypes } = useQuery({
    queryKey: ['biotypes', activeGenome],
    queryFn: () => genesApi.getBiotypes(activeGenome),
  });

  const { data: results, isLoading, isError } = useQuery({
    queryKey: ['search-genes', activeGenome, query],
    queryFn: () => genesApi.search(activeGenome, query),
    enabled: query.length >= 2,
  });

  const filteredResults = useMemo(() => {
    if (!results) return [];
    return results.filter(gene => {
      const chromMatch = selectedChrom === 'all' || gene.chrom === selectedChrom;
      const biotypeMatch = selectedBiotype === 'all' || gene.biotype === selectedBiotype;
      return chromMatch && biotypeMatch;
    });
  }, [results, selectedChrom, selectedBiotype]);

  return (
    <LibraryShell 
      title="Gene Registry Search" 
      subtitle="Search the genomic catalog by gene name or Ensembl identifier."
    >
      <div className="flex flex-col lg:flex-row gap-12 pb-20">
        
        {/* FILTERS SIDEBAR */}
        <aside className="w-full lg:w-64 space-y-8 shrink-0">
           <div className="space-y-6 sticky top-8">
              <div className="flex items-center gap-2 px-1 text-[10px] font-bold text-slate-400 uppercase tracking-widest border-b border-slate-100 dark:border-zinc-900 pb-2">
                 <Database className="h-3 w-3" />
                 Search Filters
              </div>
              
              <div className="space-y-3">
                 <label className="text-[10px] font-bold text-slate-500 uppercase tracking-tight px-1">Chromosome</label>
                 <Select value={selectedChrom} onValueChange={setSelectedChrom}>
                    <SelectTrigger className="w-full bg-white dark:bg-zinc-950">
                       <SelectValue placeholder="All Chromosomes" />
                    </SelectTrigger>
                    <SelectContent>
                       <SelectItem value="all">All Chromosomes</SelectItem>
                       {primaryChroms.map(c => <SelectItem key={c.name} value={c.name}>{c.name}</SelectItem>)}
                    </SelectContent>
                 </Select>
              </div>

              <div className="space-y-3">
                 <label className="text-[10px] font-bold text-slate-500 uppercase tracking-tight px-1">Biotype</label>
                 <Select value={selectedBiotype} onValueChange={setSelectedBiotype}>
                    <SelectTrigger className="w-full bg-white dark:bg-zinc-950">
                       <SelectValue placeholder="All Biotypes" />
                    </SelectTrigger>
                    <SelectContent>
                       <SelectItem value="all">All Biotypes</SelectItem>
                       {biotypes?.map(b => (
                         <SelectItem key={b.biotype} value={b.biotype}>
                            {b.biotype.replace(/_/g, ' ')}
                         </SelectItem>
                       ))}
                    </SelectContent>
                 </Select>
              </div>

              {filteredResults.length > 0 && (
                <div className="p-4 bg-slate-50 dark:bg-zinc-900/50 rounded-2xl border border-slate-200 dark:border-zinc-800">
                   <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-1">Results Found</div>
                   <div className="text-2xl font-bold text-primary">{formatNumber(filteredResults.length)}</div>
                </div>
              )}
           </div>
        </aside>

        {/* SEARCH AREA */}
        <div className="flex-1 space-y-12">
          {/* SEARCH INPUT */}
          <div className="relative group">
            <div className="absolute inset-y-0 left-5 flex items-center pointer-events-none text-slate-400 group-focus-within:text-primary transition-colors">
              <Search className="h-6 w-6" />
            </div>
            <input 
              type="text" 
              placeholder="e.g. BRCA1, TP53, ENSG00000139618..."
              className="w-full h-16 bg-white dark:bg-zinc-900 border-2 border-slate-100 dark:border-zinc-800 rounded-[1.25rem] pl-14 pr-16 text-xl font-medium focus:border-primary/30 focus:ring-4 focus:ring-primary/5 transition-all outline-none shadow-sm placeholder:text-slate-300 dark:placeholder:text-zinc-700"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <div className="absolute inset-y-0 right-5 flex items-center">
              {isLoading ? (
                <Loader2 className="h-6 w-6 animate-spin text-primary" />
              ) : (
                <div className="px-2 py-1 bg-slate-50 dark:bg-zinc-800 rounded-md border border-slate-100 dark:border-zinc-700 text-[10px] font-bold text-slate-400 uppercase tracking-widest">
                  {activeGenome.toUpperCase()}
                </div>
              )}
            </div>
          </div>

          {/* RESULTS AREA */}
          <div className="space-y-4">
            {filteredResults && filteredResults.length > 0 ? (
              <div className="grid grid-cols-1 gap-4">
                {filteredResults.map((gene) => (
                  <Link key={gene.gene_id} href={`/library/genes/${gene.gene_id}`} className="block group">
                    <Card className="hover:border-primary/40 transition-all duration-300 group-hover:shadow-md group-hover:-translate-y-0.5 overflow-hidden border-slate-200 dark:border-zinc-800">
                      <CardContent className="p-0">
                        <div className="p-6 flex items-center justify-between">
                          <div className="flex items-center gap-5">
                             <div className="h-12 w-12 rounded-2xl bg-slate-50 dark:bg-zinc-900 flex items-center justify-center border border-slate-100 dark:border-zinc-800 group-hover:bg-primary/5 group-hover:border-primary/10 transition-colors">
                                <Fingerprint className="h-6 w-6 text-slate-400 group-hover:text-primary transition-colors" />
                             </div>
                             <div>
                                <div className="flex items-center gap-3 mb-1">
                                  <h3 className="text-xl font-bold text-slate-900 dark:text-zinc-100">
                                    {gene.gene_name || 'Unnamed Gene'}
                                  </h3>
                                  <Badge variant="secondary" className="bg-slate-100 dark:bg-zinc-800 text-slate-500 dark:text-zinc-400 text-[10px] font-bold uppercase tracking-widest border-none">
                                    {gene.biotype?.replace('_', ' ')}
                                  </Badge>
                                </div>
                                <p className="text-xs font-mono text-slate-400 font-bold uppercase tracking-tight">{gene.gene_id}</p>
                             </div>
                          </div>
                          <div className="flex items-center gap-8">
                            <div className="text-right hidden sm:block">
                              <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-0.5">Scaffold Location</div>
                              <div className="text-sm font-mono font-bold text-slate-700 dark:text-zinc-300">
                                {gene.chrom}<span className="text-slate-300 dark:text-zinc-700 mx-1">:</span>{gene.start != null ? formatNumber(gene.start) : '---'}
                              </div>
                            </div>
                            <div className="h-10 w-10 rounded-full flex items-center justify-center bg-slate-50 dark:bg-zinc-900 group-hover:bg-primary group-hover:text-white transition-all">
                               <ExternalLink className="h-4 w-4" />
                            </div>
                          </div>
                        </div>
                      </CardContent>
                    </Card>
                  </Link>
                ))}
              </div>
            ) : isError ? (
              <div className="p-12 text-center bg-red-50/50 dark:bg-red-900/10 rounded-[2rem] border-2 border-dashed border-red-100 dark:border-red-900/20">
                 <Info className="h-10 w-10 text-red-400 mx-auto mb-4" />
                 <h3 className="text-lg font-bold text-red-900 dark:text-red-400">Search Failed</h3>
                 <p className="text-sm text-red-600/70 dark:text-red-400/60 mt-1">We encountered an error connecting to the reference database.</p>
              </div>
            ) : query.length >= 2 && !isLoading ? (
              <div className="p-20 text-center bg-slate-50 dark:bg-zinc-900/50 rounded-[2.5rem] border-2 border-dashed border-slate-100 dark:border-zinc-800">
                 <Database className="h-12 w-12 text-slate-200 dark:text-zinc-800 mx-auto mb-6" />
                 <h3 className="text-xl font-bold text-slate-900 dark:text-zinc-100">No Matches Found</h3>
                 <p className="text-slate-500 max-w-xs mx-auto mt-2">
                   No genomic features matching &quot;{query}&quot; were found for the selected filters in the {activeGenome} assembly.
                 </p>
                 {(selectedChrom !== 'all' || selectedBiotype !== 'all') && (
                   <Button 
                     variant="outline" 
                     size="sm" 
                     className="mt-6"
                     onClick={() => { setSelectedChrom('all'); setSelectedBiotype('all'); }}
                   >
                     Clear Filters
                   </Button>
                 )}
              </div>
            ) : (
              <div className="p-20 text-center bg-slate-50/50 dark:bg-zinc-900/30 rounded-[2.5rem] border border-slate-100 dark:border-zinc-800/50">
                 <div className="flex flex-col items-center gap-4">
                    <div className="h-16 w-16 rounded-3xl bg-white dark:bg-zinc-900 flex items-center justify-center shadow-sm border border-slate-100 dark:border-zinc-800">
                       <Search className="h-8 w-8 text-slate-200 dark:text-zinc-800" />
                    </div>
                    <div className="space-y-1">
                       <h3 className="text-sm font-bold text-slate-400 dark:text-zinc-600 uppercase tracking-widest">Awaiting Input</h3>
                       <p className="text-xs text-slate-400 italic">Enter at least two characters to search the assembly.</p>
                    </div>
                 </div>
              </div>
            )}
          </div>

          {/* ACADEMIC TIP */}
          {filteredResults && filteredResults.length > 0 && (
             <div className="p-6 bg-blue-50 dark:bg-blue-900/20 rounded-2xl border border-blue-100 dark:border-blue-800 flex gap-4 items-start">
                <Info className="h-5 w-5 text-blue-500 shrink-0 mt-0.5" />
                <p className="text-[11px] text-blue-700/70 dark:text-blue-400/70 leading-relaxed italic">
                  The registry includes both canonical genes and predicted features. For highly precise results, use Ensembl Gene IDs (starting with ENSG) to avoid ambiguity with common gene symbols.
                </p>
             </div>
          )}
        </div>
      </div>
    </LibraryShell>
  );
}
