"use client"

import React, { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { proteinsApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from '@/components/ui/card';
import { ResponsiveBar } from '@nivo/bar';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { Activity, Info, AlignLeft, AlignRight } from 'lucide-react';

export default function ProteinCompositionPage() {
  const { activeGenome } = useGenomeStore();
  const chartTheme = useChartTheme();

  const { data: nuclearComp, isLoading: nuclearLoading } = useQuery({
    queryKey: ['protein-composition-nuclear', activeGenome],
    queryFn: () => proteinsApi.getComposition(activeGenome, false),
  });

  const { data: mitochondrialComp, isLoading: mtLoading } = useQuery({
    queryKey: ['protein-composition-mt', activeGenome],
    queryFn: () => proteinsApi.getComposition(activeGenome, true),
  });

  const { data: nTerminalComp, isLoading: nTerminalLoading } = useQuery({
    queryKey: ['protein-composition-n-term', activeGenome],
    queryFn: () => proteinsApi.getTerminalComposition(activeGenome, { terminal: 'N', k: 10 }),
  });

  const { data: cTerminalComp, isLoading: cTerminalLoading } = useQuery({
    queryKey: ['protein-composition-c-term', activeGenome],
    queryFn: () => proteinsApi.getTerminalComposition(activeGenome, { terminal: 'C', k: 10 }),
  });

  const comparisonData = useMemo(() => {
    if (!nuclearComp || !mitochondrialComp) return [];
    
    // Create a map of AA to their percentages
    const mtMap = new Map(mitochondrialComp.map(c => [c.amino_acid, c.percentage]));
    
    return nuclearComp
      .map(c => ({
        aa: c.amino_acid,
        nuclear: c.percentage,
        mitochondrial: mtMap.get(c.amino_acid) || 0,
      }))
      .sort((a, b) => b.nuclear - a.nuclear);
  }, [nuclearComp, mitochondrialComp]);

  return (
    <LibraryShell 
      title="Amino Acid Composition" 
      subtitle="Comparative analysis of residue frequencies between nuclear and mitochondrial proteomes."
    >
      <div className="space-y-12 pb-20">
        
        <div className="p-6 bg-amber-50 dark:bg-amber-900/20 rounded-2xl border border-amber-100 dark:border-amber-800 flex gap-6 items-start">
           <div className="h-10 w-10 rounded-xl bg-amber-500/10 flex items-center justify-center shrink-0">
              <Info className="h-5 w-5 text-amber-600 dark:text-amber-400" />
           </div>
           <div className="space-y-2">
              <h4 className="font-bold text-amber-900 dark:text-amber-100">Evolutionary Divergence</h4>
              <p className="text-sm text-amber-700/80 dark:text-amber-300/80 leading-relaxed">
                Mitochondrial proteins often exhibit significant compositional differences compared to nuclear-encoded proteins, 
                reflecting their unique evolutionary origin and the specific constraints of the mitochondrial environment.
              </p>
           </div>
        </div>

        {/* COMPARISON CHART */}
        <Card className="p-10 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm h-[600px] flex flex-col">
           <div className="flex items-center justify-between mb-8">
              <h3 className="text-xl font-bold text-slate-900 dark:text-zinc-100 flex items-center gap-2">
                 <Activity className="h-5 w-5 text-primary" />
                 Nuclear vs. Mitochondrial Frequency
              </h3>
           </div>

           <div className="flex-1 min-h-0">
             {(nuclearLoading || mtLoading) ? (
                <div className="h-full w-full flex items-center justify-center">
                   <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                </div>
             ) : (
               <ResponsiveBar
                 data={comparisonData}
                 keys={['nuclear', 'mitochondrial']}
                 indexBy="aa"
                 margin={{ top: 20, right: 130, bottom: 50, left: 60 }}
                 padding={0.3}
                 groupMode="grouped"
                 colors={['#3b82f6', '#ec4899']}
                 borderRadius={4}
                 theme={chartTheme}
                 axisBottom={{
                   tickSize: 5,
                   tickPadding: 5,
                   tickRotation: 0,
                   legend: 'Amino Acid',
                   legendPosition: 'middle',
                   legendOffset: 40
                 }}
                 axisLeft={{
                   tickSize: 5,
                   tickPadding: 5,
                   tickRotation: 0,
                   legend: 'Frequency (%)',
                   legendPosition: 'middle',
                   legendOffset: -50,
                   format: '.1f'
                 }}
                 enableLabel={false}
                 legends={[
                    {
                      dataFrom: 'keys',
                      anchor: 'bottom-right',
                      direction: 'column',
                      justify: false,
                      translateX: 120,
                      translateY: 0,
                      itemsSpacing: 2,
                      itemWidth: 100,
                      itemHeight: 20,
                      itemDirection: 'left-to-right',
                      itemOpacity: 0.85,
                      symbolSize: 20,
                    }
                 ]}
               />
             )}
           </div>
        </Card>

        {/* TERMINAL COMPOSITION CHARTS */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
           <Card className="p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm h-[500px] flex flex-col">
              <div className="flex items-center justify-between mb-8">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                    <AlignLeft className="h-4 w-4 text-emerald-500" />
                    N-Terminal Bias (First 10 AA)
                 </h3>
              </div>
              <div className="flex-1 min-h-0">
                 {nTerminalLoading ? (
                    <div className="h-full w-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-emerald-500" />
                    </div>
                 ) : (
                    <ResponsiveBar
                      data={(nTerminalComp || []).map(c => ({ amino_acid: c.amino_acid, percentage: c.percentage }))}
                      keys={['percentage']}
                      indexBy="amino_acid"
                      margin={{ top: 20, right: 20, bottom: 50, left: 60 }}
                      padding={0.3}
                      colors={['#10b981']}
                      borderRadius={4}
                      theme={chartTheme}
                      axisBottom={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                      }}
                      axisLeft={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: 'Frequency (%)',
                        legendPosition: 'middle',
                        legendOffset: -50,
                        format: '.1f'
                      }}
                      enableLabel={false}
                    />
                 )}
              </div>
           </Card>

           <Card className="p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm h-[500px] flex flex-col">
              <div className="flex items-center justify-between mb-8">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                    <AlignRight className="h-4 w-4 text-amber-500" />
                    C-Terminal Bias (Last 10 AA)
                 </h3>
              </div>
              <div className="flex-1 min-h-0">
                 {cTerminalLoading ? (
                    <div className="h-full w-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-amber-500" />
                    </div>
                 ) : (
                    <ResponsiveBar
                      data={(cTerminalComp || []).map(c => ({ amino_acid: c.amino_acid, percentage: c.percentage }))}
                      keys={['percentage']}
                      indexBy="amino_acid"
                      margin={{ top: 20, right: 20, bottom: 50, left: 60 }}
                      padding={0.3}
                      colors={['#f59e0b']}
                      borderRadius={4}
                      theme={chartTheme}
                      axisBottom={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                      }}
                      axisLeft={{
                        tickSize: 5,
                        tickPadding: 5,
                        tickRotation: 0,
                        legend: 'Frequency (%)',
                        legendPosition: 'middle',
                        legendOffset: -50,
                        format: '.1f'
                      }}
                      enableLabel={false}
                    />
                 )}
              </div>
           </Card>
        </div>

      </div>
    </LibraryShell>
  );
}
