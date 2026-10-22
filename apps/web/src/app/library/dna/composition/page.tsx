"use client"

import React, { useMemo } from 'react';
import { useDNAStats } from '@/hooks/use-dna-stats';
import { LibraryShell } from '@/components/layout/library-shell';
import { SectionHeader } from '@/components/library/dna/shared';
import { NUCLEOTIDE_COLORS } from '@/lib/constants';

// Modular Components
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { NucleotideDistribution } from '@/components/library/dna/nucleotide-distribution';
import { DinucleotideHeatmap } from '@/components/library/dna/dinucleotide-heatmap';
import { ChromosomeBrowser } from '@/components/library/dna/chromosome-browser';
import { GenomicChartContainer } from '@/components/library/dna/genomic-chart-container';
import { ResponsiveBar } from '@nivo/bar';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { Activity } from 'lucide-react';

export default function DNACompositionPage() {
  const { summary, dinucleotides, primaryChroms, isLoading, isError } = useDNAStats();
  const chartTheme = useChartTheme();

  // 1. Data Processing
  const nuclearComp = summary.data?.nuclear_composition;
  
  const totalNuclearBases = useMemo(() => {
    if (!nuclearComp) return 0;
    return (nuclearComp.a || 0) + (nuclearComp.c || 0) + (nuclearComp.g || 0) + (nuclearComp.t || 0);
  }, [nuclearComp]);

  const baseStats = useMemo(() => {
    if (!nuclearComp || totalNuclearBases === 0) return [];
    
    return [
      { id: 'A', name: 'Adenine', value: nuclearComp.a, color: NUCLEOTIDE_COLORS.A, desc: 'Adenine count over total ACGT.' },
      { id: 'C', name: 'Cytosine', value: nuclearComp.c, color: NUCLEOTIDE_COLORS.C, desc: 'Cytosine count over total ACGT.' },
      { id: 'G', name: 'Guanine', value: nuclearComp.g, color: NUCLEOTIDE_COLORS.G, desc: 'Guanine count over total ACGT.' },
      { id: 'T', name: 'Thymine', value: nuclearComp.t, color: NUCLEOTIDE_COLORS.T, desc: 'Thymine count over total ACGT.' },
    ].map(b => ({
      ...b,
      percent: (b.value / totalNuclearBases) * 100
    }));
  }, [nuclearComp, totalNuclearBases]);

  const heatmapData = useMemo(() => {
    if (!dinucleotides.data?.dinucleotides) return [];
    const bases = ['A', 'C', 'G', 'T'];
    return bases.map(b1 => ({
      id: b1,
      data: bases.map(b2 => {
        const entry = dinucleotides.data.dinucleotides.find(d => d.pair === b1 + b2);
        return { x: b2, y: entry?.oe_ratio || 0 };
      })
    }));
  }, [dinucleotides.data]);

  // Per-chromosome A/C/G/T counts are not served by the API (the per-chromosome
  // dinucleotide field is an empty placeholder), so only GC vs AT is shown.
  const chromProfiles = useMemo(() => {
    return primaryChroms.map(c => ({
      id: c.name.replace('chr', ''),
      GC: c.gc_content,
      AT: 100 - c.gc_content,
    })).reverse();
  }, [primaryChroms]);

  const globalGC = nuclearComp?.gc_content;
  const gcVariationData = useMemo(() => {
    if (globalGC == null) return [];
    return primaryChroms.map(c => ({
      id: c.name.replace('chr', ''),
      gc: c.gc_content,
      deviation: c.gc_content - globalGC,
    }));
  }, [primaryChroms, globalGC]);

  // 2. Render
  return (
    <LibraryShell 
      title="Base Composition" 
      subtitle="Base counts, dinucleotide observed/expected ratios and GC content of T2T-CHM13v2.0."
    >
      <div className="space-y-20 pb-20">
        
        {/* GLOBAL PERSPECTIVE */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Global Nucleotide Landscape" 
              subtitle="Share of A, C, G, T over the whole assembly (percent of ACGT bases)."
            />
            <div className="flex items-center gap-2 px-3 py-1 bg-slate-100 dark:bg-zinc-900 rounded-full border border-slate-200 dark:border-zinc-800">
               <Activity className="h-3.5 w-3.5 text-slate-400" />
               <span className="text-[10px] font-bold text-slate-500 uppercase tracking-tight">Precomputed Track</span>
            </div>
          </div>

          <div className="space-y-8">
            {nuclearComp && (
              <NucleotideDistribution 
                data={{ 
                  A: (nuclearComp.a/totalNuclearBases)*100, 
                  C: (nuclearComp.c/totalNuclearBases)*100, 
                  G: (nuclearComp.g/totalNuclearBases)*100, 
                  T: (nuclearComp.t/totalNuclearBases)*100,
                  label: 'Global Assembly'
                }} 
              />
            )}

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
               {isLoading ? (
                  [...Array(4)].map((_, i) => <div key={i} className="h-32 bg-slate-100 dark:bg-zinc-900 rounded-xl animate-pulse" />)
               ) : (
                 baseStats.map(base => (
                   <GenomicMetricCard 
                     key={base.id}
                     label={base.id}
                     fullName={base.name}
                     value={base.percent.toFixed(2)}
                     unit="%"
                     color={base.color}
                     description={base.desc}
                   />
                 ))
               )}
            </div>
          </div>
        </section>

        {/* INTERACTIVE BROWSER */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Chromosomal Dynamics" 
              subtitle="GC content (%) in windows along one chromosome."
            />
          </div>
          <ChromosomeBrowser 
            chromosomes={primaryChroms} 
            initialChrom={primaryChroms[0]?.name}
          />
        </section>

        {/* VARIATION & BIAS */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-12 items-stretch">
          <DinucleotideHeatmap 
            data={heatmapData} 
            isLoading={dinucleotides.isLoading} 
            isError={dinucleotides.isError}
            onRetry={() => dinucleotides.refetch()}
          />

          <GenomicChartContainer
            title="GC Deviation by Scaffold"
            subtitle="Chromosome GC content minus whole-assembly GC content, in percentage points."
            isLoading={isLoading}
            isError={isError}
            onRetry={() => summary.refetch()}
          >
            <ResponsiveBar
              data={gcVariationData}
              keys={['deviation']}
              indexBy="id"
              margin={{ top: 20, right: 10, bottom: 40, left: 45 }}
              padding={0.3}
              valueScale={{ type: 'linear', min: 'auto', max: 'auto' }}
              indexScale={{ type: 'band', round: true }}
              colors={d => (d.value ?? 0) >= 0 ? '#06b6d4' : '#f97316'}
              axisBottom={{
                tickSize: 5,
                tickPadding: 5,
                tickRotation: 0,
                legend: 'Chromosome',
                legendPosition: 'middle',
                legendOffset: 32
              }}
              axisLeft={{
                tickSize: 5,
                tickPadding: 5,
                tickRotation: 0,
                legend: 'Deviation (%)',
                legendPosition: 'middle',
                legendOffset: -40,
                format: '.1f'
              }}
              enableLabel={false}
              theme={chartTheme}
              animate={true}
              tooltip={({ value, indexValue }) => (
                <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-sm">
                   <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-1">Chr{indexValue}</div>
                   <div className="text-sm font-bold text-slate-900 dark:text-zinc-100">
                     {value > 0 ? '+' : ''}{value.toFixed(2)}% GC Deviation
                   </div>
                </div>
              )}
            />
          </GenomicChartContainer>
        </div>

        {/* SCAFFOLD-LEVEL PROFILES */}
        <section>
          <GenomicChartContainer
            title="Chromosome Composition Profiles"
            subtitle="GC versus AT content (percent of A, C, G, T bases) for each primary scaffold."
            isLoading={isLoading}
            isError={isError}
            height="h-[600px]"
          >
            <ResponsiveBar
              data={chromProfiles}
              keys={['GC', 'AT']}
              indexBy="id"
              layout="horizontal"
              margin={{ top: 20, right: 30, bottom: 50, left: 40 }}
              padding={0.2}
              valueScale={{ type: 'linear' }}
              colors={['#06b6d4', '#94a3b8']}
              axisBottom={{
                tickSize: 5,
                tickPadding: 5,
                tickRotation: 0,
                legend: 'Percentage (%)',
                legendPosition: 'middle',
                legendOffset: 40
              }}
              axisLeft={{
                tickSize: 5,
                tickPadding: 5,
                tickRotation: 0,
                legend: 'Chr',
                legendPosition: 'middle',
                legendOffset: -35
              }}
              enableLabel={false}
              theme={chartTheme}
              animate={true}
              legends={[
                {
                  dataFrom: 'keys',
                  anchor: 'bottom-right',
                  direction: 'row',
                  justify: false,
                  translateX: 0,
                  translateY: 45,
                  itemsSpacing: 2,
                  itemWidth: 60,
                  itemHeight: 20,
                  itemDirection: 'left-to-right',
                  itemOpacity: 0.85,
                  symbolSize: 10,
                  effects: [
                    {
                      on: 'hover',
                      style: {
                        itemOpacity: 1
                      }
                    }
                  ]
                }
              ]}
            />
          </GenomicChartContainer>
        </section>


      </div>
    </LibraryShell>
  );
}
