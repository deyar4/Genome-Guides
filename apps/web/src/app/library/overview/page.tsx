"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { dnaApi, genesApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { formatNumber, formatGb } from '@/lib/utils';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { Dna, Target, ShieldCheck, BookOpen } from 'lucide-react';
import { Card } from '@/components/ui/card';

export default function LibraryOverview() {
  const { activeGenome } = useGenomeStore();

  const assemblyName = activeGenome === 't2t' || activeGenome === 'hs1' ? 'T2T-CHM13v2.0' : activeGenome;

  const { data: dnaSummary } = useQuery({
    queryKey: ['dna-summary', activeGenome],
    queryFn: () => dnaApi.getSummary(activeGenome),
  });

  const { data: geneSummary } = useQuery({
    queryKey: ['gene-summary', activeGenome],
    queryFn: () => genesApi.getSummary(activeGenome),
  });

  const { data: chromosomes } = useQuery({
    queryKey: ['chromosomes', activeGenome],
    queryFn: () => dnaApi.getChromosomes(activeGenome),
  });

  const proteinCodingCount = useMemo(() => {
    if (dnaSummary?.protein_coding_count) return dnaSummary.protein_coding_count;
    if (!geneSummary?.biotype_counts) return 0;
    return geneSummary.biotype_counts.find(b => b.biotype === 'protein_coding')?.count || 0;
  }, [geneSummary, dnaSummary]);

  const totalGenes = useMemo(() => {
    if (dnaSummary?.gene_count) return dnaSummary.gene_count;
    return geneSummary?.total_genes || 0;
  }, [geneSummary, dnaSummary]);

  const nuclearLength = useMemo(() => {
    // sum over non-mitochondrial sequences: the API's nuclear_composition equals the whole-assembly total (includes chrM)
    if (!chromosomes?.chromosomes) return 0;
    return chromosomes.chromosomes.filter(c => !c.is_mitochondrial).reduce((acc, c) => acc + c.length, 0);
  }, [chromosomes]);

  return (
    <LibraryShell 
      title="Genome Overview" 
      subtitle={`Summary of the ${assemblyName} assembly and global genomic statistics.`}
    >
      <div className="space-y-12 pb-20">
        
        {/* HERO METRICS */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <GenomicMetricCard 
            label="Nuclear Length"
            value={nuclearLength > 0 ? formatGb(nuclearLength) : '...'}
            unit="Gb"
            description="Total combined length of all nuclear chromosomes."
            color="#3b82f6"
          />
          <GenomicMetricCard 
            label="Total Genes"
            value={totalGenes > 0 ? formatNumber(totalGenes) : '...'}
            description="Total annotated features (Coding & Non-coding)."
            color="#10b981"
          />
          <GenomicMetricCard 
            label="GC Content"
            value={dnaSummary?.nuclear_composition ? dnaSummary.nuclear_composition.gc_content.toFixed(2) : '...'}
            unit="%"
            description="Average chemical composition across the genome."
            color="#06b6d4"
          />
          <GenomicMetricCard 
            label="Protein Coding"
            value={proteinCodingCount > 0 ? formatNumber(proteinCodingCount) : '...'}
            description="Genes with biotype protein_coding."
            color="#f59e0b"
          />
        </div>

        {/* NARRATIVE SECTION */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-12">
          <div className="lg:col-span-2 space-y-8">
            <section className="prose prose-slate dark:prose-invert max-w-none">
              <h3 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-zinc-100">About the T2T-CHM13 Assembly</h3>
              <p className="text-lg text-slate-600 dark:text-zinc-400 leading-relaxed">
                T2T-CHM13v2.0 represents the first complete, gapless human genome assembly. It resolves nearly 200 million base pairs of previously unknown sequence—including centromeres and ribosomal DNA arrays—effectively completing the map of the human blueprint.
              </p>
            </section>

            <div className="p-8 bg-slate-50 dark:bg-zinc-900/50 rounded-3xl border border-slate-200 dark:border-zinc-800 flex gap-6">
               <div className="h-12 w-12 rounded-2xl bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center shrink-0">
                  <ShieldCheck className="h-6 w-6 text-blue-600 dark:text-blue-400" />
               </div>
               <div className="space-y-2">
                  <h4 className="font-bold text-slate-900 dark:text-zinc-100 uppercase text-xs tracking-widest">Reference Methodology</h4>
                  <p className="text-sm text-slate-500 dark:text-zinc-400 leading-relaxed">
                    Statistics are generated from precomputed reference datasets verified by the Genome Guides analysis pipeline. 
                    The use of T2T-CHM13 enables comprehensive analysis of the most complex structural elements of our DNA.
                  </p>
               </div>
            </div>
          </div>

          <aside className="space-y-6">
             <Card className="p-6 bg-primary text-primary-foreground border-none shadow-xl relative overflow-hidden group">
                <div className="relative z-10">
                   <h4 className="font-bold text-sm mb-4 flex items-center gap-2">
                      <BookOpen className="h-4 w-4 text-primary" />
                      Explorer&apos;s Guide
                   </h4>
                   <p className="text-[11px] text-slate-400 leading-relaxed mb-6">
                      Welcome to the Genome Guides. Use the sidebar to explore the physical blueprint (DNA) or functional units (Genes) of the human assembly.
                   </p>
                   <ul className="space-y-3">
                      <li className="flex items-center gap-3 text-[10px] font-medium text-slate-300">
                         <div className="h-1.5 w-1.5 rounded-full bg-primary" />
                         Use ⌘K for instant search
                      </li>
                      <li className="flex items-center gap-3 text-[10px] font-medium text-slate-300">
                         <div className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
                         Click a chromosome on the Chromosome Metrics map to select it
                      </li>
                      <li className="flex items-center gap-3 text-[10px] font-medium text-slate-300">
                         <div className="h-1.5 w-1.5 rounded-full bg-blue-400" />
                         Download a gene&apos;s sequence as FASTA from its gene page
                      </li>
                   </ul>
                </div>
                <Dna className="absolute -right-12 -bottom-12 w-40 h-40 text-primary-foreground/10 group-hover:text-primary-foreground/20 transition-colors pointer-events-none" />
             </Card>

             <Card className="p-6 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm">
                <h4 className="font-bold text-slate-900 dark:text-zinc-100 text-sm mb-4 flex items-center gap-2">
                   <Target className="h-4 w-4 text-emerald-500" />
                   Analytical Modules
                </h4>
                <ul className="space-y-4">
                   {[
                     { label: 'DNA Analysis', status: 'Ready', desc: 'Assembly metrics & composition' },
                     { label: 'Gene Census', status: 'Ready', desc: 'Biotype distribution & search' },
                     { label: 'RNA Clusters', status: 'Beta', desc: 'ncRNA clusters (nested-gene analysis not yet available)' },
                     { label: 'Protein Census', status: 'Beta', desc: 'Proteomic composition' },
                   ].map((m, i) => (
                     <li key={i} className="group cursor-default">
                        <div className="flex items-center justify-between mb-1">
                           <span className="text-xs font-bold text-slate-700 dark:text-zinc-300">{m.label}</span>
                           <span className={`text-[9px] font-bold px-1.5 py-0.5 rounded ${
                             m.status === 'Ready' ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400' :
                             m.status === 'Beta' ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400' :
                             'bg-slate-100 text-slate-500 dark:bg-zinc-800 dark:text-zinc-500'
                           }`}>{m.status}</span>
                        </div>
                        <p className="text-[10px] text-slate-400 group-hover:text-slate-500 transition-colors">{m.desc}</p>
                     </li>
                   ))}
                </ul>
             </Card>
          </aside>
        </div>

      </div>
    </LibraryShell>
  );
}
