"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { dnaApi, genesApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { formatNumber, formatGb } from '@/lib/utils';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';

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
      subtitle={`${assemblyName}: assembly size, GC content and gene counts.`}
    >
      <div className="space-y-12 pb-20">
        
        {/* HERO METRICS */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          <GenomicMetricCard 
            label="Nuclear Length"
            value={nuclearLength > 0 ? formatGb(nuclearLength) : '...'}
            unit="Gb"
            description="Sum of lengths of the non-mitochondrial sequences (bp)."
            color="#3b82f6"
          />
          <GenomicMetricCard 
            label="Total Genes"
            value={totalGenes > 0 ? formatNumber(totalGenes) : '...'}
            description="Gene models in the annotation, all biotypes."
            color="#10b981"
          />
          <GenomicMetricCard 
            label="GC Content"
            value={dnaSummary?.nuclear_composition ? dnaSummary.nuclear_composition.gc_content.toFixed(2) : '...'}
            unit="%"
            description="100*(G+C)/(A+C+G+T) over the assembly."
            color="#06b6d4"
          />
          <GenomicMetricCard 
            label="Protein Coding"
            value={proteinCodingCount > 0 ? formatNumber(proteinCodingCount) : '...'}
            description="Genes with biotype protein_coding."
            color="#f59e0b"
          />
        </div>

        <p className="text-xs text-slate-500 max-w-3xl">
          Reference: T2T-CHM13v2.0 (source URL and checksum in the workflow config). Statistics come from the precomputed tracks served by the API. Coordinates are 0-based; intervals are half-open [start, end).
        </p>
      </div>
    </LibraryShell>
  );
}
