import { useQuery } from '@tanstack/react-query';
import { dnaApi } from '@/lib/api';
import { useGenomeStore } from '@/store/use-genome-store';
import React from 'react';

export function useDNAStats() {
  const { activeGenome } = useGenomeStore();
  const [selectedChrom, setSelectedChrom] = React.useState<string | null>(null);

  const summary = useQuery({
    queryKey: ['dna-summary', activeGenome],
    queryFn: () => dnaApi.getSummary(activeGenome),
  });

  const chromosomes = useQuery({
    queryKey: ['chromosomes', activeGenome],
    queryFn: () => dnaApi.getChromosomes(activeGenome),
  });

  const dinucleotides = useQuery({
    queryKey: ['dinucleotides', activeGenome],
    queryFn: () => dnaApi.getDinucleotides(activeGenome),
  });

  const telomeres = useQuery({
    queryKey: ['telomeres', activeGenome],
    queryFn: () => dnaApi.getTelomeres(activeGenome),
  });

  const centromeres = useQuery({
    queryKey: ['centromeres', activeGenome],
    queryFn: () => dnaApi.getCentromeres(activeGenome),
  });

  // Derived & Formatted Data
  const primaryChroms = React.useMemo(() => {
    if (!chromosomes.data?.chromosomes) return [];
    
    // Natural sorting function for chromosome names (handles 'chr1', 'chr10' correctly)
    const naturalSort = (a: string, b: string) => {
      return a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' });
    };

    return chromosomes.data.chromosomes
      .filter(c => !c.name.includes('_') && !c.name.includes('.'))
      .sort((a, b) => naturalSort(a.name, b.name));
  }, [chromosomes.data]);

  const totalPrimaryLength = React.useMemo(() => 
    primaryChroms.reduce((acc, c) => acc + c.length, 0), 
  [primaryChroms]);

  const focusedChromData = React.useMemo(() => 
    selectedChrom ? primaryChroms.find(c => c.name === selectedChrom) : null,
  [selectedChrom, primaryChroms]);

  const isLoading = summary.isLoading || chromosomes.isLoading || dinucleotides.isLoading;
  const isError = summary.isError || chromosomes.isError || dinucleotides.isError;

  return {
    // Raw Queries
    summary,
    chromosomes,
    dinucleotides,
    telomeres,
    centromeres,
    
    // Computed
    primaryChroms,
    totalPrimaryLength,
    focusedChromData,
    
    // State
    selectedChrom,
    setSelectedChrom,
    activeGenome,
    
    // Status
    isLoading,
    isError
  };
}
