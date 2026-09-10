"use client"

import React, { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { genesApi } from '@/lib/api';
import { useGenomeStore } from '@/store/use-genome-store';
import { useDNAStats } from '@/hooks/use-dna-stats';
import { Card } from '@/components/ui/card';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from '@/components/ui/select';
import { 
  ResponsiveContainer, 
  BarChart, 
  Bar, 
  XAxis, 
  YAxis, 
  Tooltip, 
  Cell
} from 'recharts';
import { formatNumber } from '@/lib/utils';
import { Loader2, Activity } from 'lucide-react';

export function GeneDensityMap() {
  const { activeGenome } = useGenomeStore();
  const { primaryChroms } = useDNAStats();
  const [selectedChrom, setSelectedChrom] = useState('chr1');

  const { data: density, isLoading } = useQuery({
    queryKey: ['gene-density', activeGenome, selectedChrom],
    queryFn: () => genesApi.getDensity(activeGenome, selectedChrom, 1000000), // 1Mb bins
    enabled: !!selectedChrom
  });

  const currentChrom = primaryChroms.find(c => c.name === selectedChrom);

  const chartData = useMemo(() => {
    if (!density || !currentChrom) return [];
    
    // Create a complete set of bins from 0 to chromosome length
    const binSize = 1000000;
    const maxBin = Math.ceil(currentChrom.length / binSize) * binSize;
    const dataMap = new Map(density.map(d => [d.start, (d as { count: number, value?: number }).value || d.count || 0]));
    
    const fullData = [];
    for (let pos = 0; pos < maxBin; pos += binSize) {
      fullData.push({
        bin: pos,
        count: dataMap.get(pos) || 0,
      });
    }
    return fullData;
  }, [density, currentChrom]);

  const maxCount = useMemo(() => {
    if (!chartData.length) return 0;
    return Math.max(...chartData.map(d => d.count));
  }, [chartData]);

  const getColor = (count: number) => {
    const ratio = count / (maxCount || 1);
    if (ratio > 0.8) return '#ef4444'; // High density - Red
    if (ratio > 0.5) return '#f97316'; // Medium-high - Orange
    if (ratio > 0.2) return '#3b82f6'; // Medium - Blue
    return '#94a3b8'; // Low - Slate
  };

  return (
    <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm">
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 mb-8">
        <div className="space-y-1">
          <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
            <Activity className="h-4 w-4 text-primary" />
            Chromosomal Gene Density
          </h3>
          <p className="text-[10px] text-slate-400 font-medium">Binned distribution of functional elements (1Mb resolution).</p>
        </div>

        <Select value={selectedChrom} onValueChange={setSelectedChrom}>
          <SelectTrigger className="w-32 h-8 text-xs bg-slate-50 dark:bg-zinc-900">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {primaryChroms.map(c => (
              <SelectItem key={c.name} value={c.name}>{c.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="h-[250px] w-full relative">
        {isLoading && (
          <div className="absolute inset-0 flex items-center justify-center bg-white/50 dark:bg-zinc-950/50 backdrop-blur-sm z-10 rounded-xl">
            <Loader2 className="h-6 w-6 animate-spin text-primary" />
          </div>
        )}

        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chartData} margin={{ top: 0, right: 0, left: 0, bottom: 0 }}>
            <XAxis 
              dataKey="bin" 
              hide 
            />
            <YAxis hide domain={[0, 'auto']} />
            <Tooltip 
              content={({ active, payload, label }) => {
                if (active && payload && payload.length) {
                  const binStart = typeof label === 'number' ? label : Number(label ?? 0);
                  return (
                    <div className="bg-white dark:bg-zinc-900 p-2 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-xl">
                      <div className="text-[10px] font-bold text-slate-400 uppercase mb-1">
                        Region: {formatNumber(binStart)} – {formatNumber(binStart + 1000000)} bp
                      </div>
                      <div className="text-sm font-bold text-slate-900 dark:text-zinc-100">
                        {payload[0].value} Genes
                      </div>
                    </div>
                  );
                }
                return null;
              }}
            />
            <Bar dataKey="count" isAnimationActive={true}>
              {chartData.map((entry, index) => (
                <Cell key={`cell-${index}`} fill={getColor(entry.count)} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>

      <div className="mt-6 flex items-center justify-between">
         <div className="flex items-center gap-4">
            <div className="flex items-center gap-1.5">
               <div className="w-2 h-2 rounded-full bg-slate-400" />
               <span className="text-[10px] text-slate-500 font-bold uppercase tracking-tight">Low</span>
            </div>
            <div className="flex items-center gap-1.5">
               <div className="w-2 h-2 rounded-full bg-blue-500" />
               <span className="text-[10px] text-slate-500 font-bold uppercase tracking-tight">Med</span>
            </div>
            <div className="flex items-center gap-1.5">
               <div className="w-2 h-2 rounded-full bg-orange-500" />
               <span className="text-[10px] text-slate-500 font-bold uppercase tracking-tight">High</span>
            </div>
            <div className="flex items-center gap-1.5">
               <div className="w-2 h-2 rounded-full bg-red-500" />
               <span className="text-[10px] text-slate-500 font-bold uppercase tracking-tight">Extreme</span>
            </div>
         </div>
         <div className="text-[10px] text-slate-400 font-mono italic">
            Max Density: {maxCount} genes/Mb
         </div>
      </div>
    </Card>
  );
}
