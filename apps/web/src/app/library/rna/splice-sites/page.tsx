"use client"

import React, { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { rnaApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { Card } from "@/components/ui/card";
import { cn } from '@/lib/utils';
import { ResponsiveBar } from '@nivo/bar';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { Activity, Scissors, ArrowDown } from 'lucide-react';

export default function SpliceSitesPage() {
  const { activeGenome } = useGenomeStore();
  const chartTheme = useChartTheme();
  const [siteType, setSiteType] = useState<'donor' | 'acceptor'>('donor');

  const { data: consensusList, isLoading } = useQuery({
    queryKey: ['rna-splice-consensus', activeGenome, siteType],
    queryFn: () => rnaApi.getSpliceConsensus(activeGenome, siteType),
  });

  const consensus = consensusList?.[0];

  const chartData = React.useMemo(() => {
    if (!consensus?.pwm) return [];
    const positions = Object.values(consensus.pwm)[0].length;
    return Array.from({ length: positions }).map((_, i) => ({
      position: i + 1,
      A: consensus.pwm.A[i],
      C: consensus.pwm.C[i],
      G: consensus.pwm.G[i],
      T: consensus.pwm.T[i],
    }));
  }, [consensus]);

  return (
    <LibraryShell 
      title="Splice Junction Architecture" 
      subtitle="Visualization of sequence consensus and Position Weight Matrices (PWMs) at intron-exon boundaries."
    >
      <div className="space-y-12 pb-20">
        
        {/* SELECTOR */}
        <div className="flex gap-4">
           {['donor', 'acceptor'].map((type) => (
             <button
               key={type}
               onClick={() => setSiteType(type as 'donor' | 'acceptor')}
               className={cn(
                 "px-8 py-4 rounded-2xl border-2 transition-all font-bold text-sm uppercase tracking-widest",
                 siteType === type 
                   ? "bg-primary border-primary text-white shadow-lg shadow-primary/20 scale-105" 
                   : "bg-white dark:bg-zinc-900 border-slate-200 dark:border-zinc-800 text-slate-400 hover:border-primary/30"
               )}
             >
               {type} Site (5&apos;)
             </button>
           ))}
        </div>

        {/* SUMMARY INFO */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
           <Card className="p-8 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm flex flex-col justify-center text-center">
              <span className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-2">Consensus Sequence</span>
              <div className="flex justify-center gap-1 flex-wrap">
                 {consensus?.consensus_seq.split('').map((base, i) => (
                   <span key={i} className={cn(
                     "text-2xl md:text-3xl font-mono font-black",
                     base === 'G' || base === 'T' ? "text-primary" : "text-slate-300 dark:text-zinc-700"
                   )}>
                      {base}
                   </span>
                 ))}
              </div>
              <p className="text-xs text-slate-400 mt-4">Based on {consensus ? (consensus.total_sites / 1000).toFixed(0) : '...'}k validated junctions</p>
           </Card>

           <div className="lg:col-span-2 p-8 bg-slate-900 rounded-3xl border border-slate-800 flex gap-6 items-start">
              <div className="h-12 w-12 rounded-2xl bg-primary/10 flex items-center justify-center shrink-0">
                 <Scissors className="h-6 w-6 text-primary" />
              </div>
              <div className="space-y-3">
                 <h4 className="font-bold text-foreground uppercase text-xs tracking-widest">Molecular Logic</h4>
                 <p className="text-sm text-slate-400 leading-relaxed">
                   The {siteType} site is the highly conserved boundary where the spliceosome initiates the excision of an intron. 
                   The sequence logo below represents the probability of each nucleotide appearing at specific positions relative to the cleavage point.
                 </p>
                 <div className="flex items-center gap-4 pt-2">
                    <div className="flex items-center gap-2 text-[10px] font-bold text-slate-500">
                       <div className="h-2 w-2 rounded-full bg-emerald-500" />
                       Exonic
                    </div>
                    <div className="flex items-center gap-2 text-[10px] font-bold text-slate-500">
                       <div className="h-2 w-2 rounded-full bg-blue-500" />
                       Intronic
                    </div>
                 </div>
              </div>
           </div>
        </div>

        {/* PWM CHART */}
        <Card className="p-10 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm h-[500px] flex flex-col">
           <div className="flex items-center justify-between mb-8">
              <h3 className="text-xl font-bold text-slate-900 dark:text-zinc-100 flex items-center gap-2">
                 <Activity className="h-5 w-5 text-primary" />
                 Position Weight Matrix (Log-Odds)
              </h3>
           </div>

           <div className="flex-1 min-h-0">
             {isLoading ? (
                <div className="h-full w-full flex items-center justify-center">
                   <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary" />
                </div>
             ) : (
               <ResponsiveBar
                 data={chartData}
                 keys={['A', 'C', 'G', 'T']}
                 indexBy="position"
                 margin={{ top: 20, right: 30, bottom: 60, left: 60 }}
                 padding={0.3}
                 valueScale={{ type: 'linear' }}
                 indexScale={{ type: 'band', round: true }}
                 colors={{ scheme: 'category10' }}
                 borderRadius={4}
                 axisBottom={{
                   tickSize: 5,
                   tickPadding: 10,
                   tickRotation: 0,
                   legend: 'Relative Position (bp)',
                   legendPosition: 'middle',
                   legendOffset: 50
                 }}
                 axisLeft={{
                   tickSize: 5,
                   tickPadding: 10,
                   tickRotation: 0,
                   legend: 'Information Content (Bits)',
                   legendPosition: 'middle',
                   legendOffset: -50,
                   format: '.1f'
                 }}
                 labelSkipWidth={12}
                 labelSkipHeight={12}
                 theme={chartTheme}
                 gridYValues={5}
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
             )}
           </div>
           
           <div className="mt-8 flex justify-center gap-8">
              <div className="text-center">
                 <ArrowDown className="h-4 w-4 text-primary mx-auto mb-2 animate-bounce" />
                 <span className="text-[10px] font-bold text-slate-400 uppercase">Cleavage Point</span>
              </div>
           </div>
        </Card>

      </div>
    </LibraryShell>
  );
}
