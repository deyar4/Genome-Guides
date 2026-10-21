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

  const { data: nuclearComp, isLoading: compositionLoading } = useQuery({
    queryKey: ['protein-composition-global', activeGenome],
    queryFn: () => proteinsApi.getComposition(activeGenome),
  });

  // One response carries both termini (n_terminal_pct / c_terminal_pct); the API takes no terminal or k parameter.
  const { data: terminalComp, isLoading: terminalLoading } = useQuery({
    queryKey: ['protein-composition-terminal', activeGenome],
    queryFn: () => proteinsApi.getTerminalComposition(activeGenome),
  });

  const compositionData = useMemo(() => {
    if (!nuclearComp) return [];
    return nuclearComp
      .map(c => ({ aa: c.amino_acid, frequency: c.percentage }))
      .sort((a, b) => b.frequency - a.frequency);
  }, [nuclearComp]);

  return (
    <LibraryShell 
      title="Amino Acid Composition" 
      subtitle="Residue frequencies over all protein records, and amino acid usage at the protein termini."
    >
      <div className="space-y-12 pb-20">
        
        <div className="p-6 bg-amber-50 dark:bg-amber-900/20 rounded-2xl border border-amber-100 dark:border-amber-800 flex gap-6 items-start">
           <div className="h-10 w-10 rounded-xl bg-amber-500/10 flex items-center justify-center shrink-0">
              <Info className="h-5 w-5 text-amber-600 dark:text-amber-400" />
           </div>
           <div className="space-y-2">
              <h4 className="font-bold text-amber-900 dark:text-amber-100">Mitochondrial comparison unavailable</h4>
              <p className="text-sm text-amber-700/80 dark:text-amber-300/80 leading-relaxed">
                The composition endpoint has no nuclear/mitochondrial split: asking for either returns the same all-protein table, so a nuclear versus mitochondrial comparison is not shown.
              </p>
           </div>
        </div>

        {/* COMPARISON CHART */}
        <Card className="p-10 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm h-[600px] flex flex-col">
           <div className="flex items-center justify-between mb-8">
              <h3 className="text-xl font-bold text-slate-900 dark:text-zinc-100 flex items-center gap-2">
                 <Activity className="h-5 w-5 text-primary" />
                 Amino Acid Frequency (all protein records)
              </h3>
           </div>

           <div className="flex-1 min-h-0">
             {compositionLoading ? (
                <div className="h-full w-full flex items-center justify-center">
                   <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                </div>
             ) : (
               <ResponsiveBar
                 data={compositionData}
                 keys={['frequency']}
                 indexBy="aa"
                 margin={{ top: 20, right: 20, bottom: 50, left: 60 }}
                 padding={0.3}
                 groupMode="grouped"
                 colors={['#3b82f6']}
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
               />
             )}
           </div>
        </Card>

        <p className="text-xs text-slate-500">Terminal usage is computed over proteins of at least 40 residues (pipeline script extract_protein_terminal_bias.py).</p>
        {/* TERMINAL COMPOSITION CHARTS */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-8">
           <Card className="p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm h-[500px] flex flex-col">
              <div className="flex items-center justify-between mb-8">
                 <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                    <AlignLeft className="h-4 w-4 text-emerald-500" />
                    N-Terminal Usage (first 20 residues)
                 </h3>
              </div>
              <div className="flex-1 min-h-0">
                 {terminalLoading ? (
                    <div className="h-full w-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-emerald-500" />
                    </div>
                 ) : (
                    <ResponsiveBar
                      data={(terminalComp || []).map(c => ({ amino_acid: c.amino_acid, percentage: c.n_terminal_pct }))}
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
                    C-Terminal Usage (last 20 residues)
                 </h3>
              </div>
              <div className="flex-1 min-h-0">
                 {terminalLoading ? (
                    <div className="h-full w-full flex items-center justify-center">
                       <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-amber-500" />
                    </div>
                 ) : (
                    <ResponsiveBar
                      data={(terminalComp || []).map(c => ({ amino_acid: c.amino_acid, percentage: c.c_terminal_pct }))}
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
