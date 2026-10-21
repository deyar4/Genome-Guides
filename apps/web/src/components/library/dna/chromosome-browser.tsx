"use client"

import React, { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { dnaApi } from '@/lib/api';
import { useGenomeStore } from '@/store/use-genome-store';
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
  XAxis, 
  YAxis, 
  CartesianGrid, 
  Tooltip, 
  AreaChart, 
  Area, 
  Brush
} from 'recharts';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { formatNumber, alignToBaseWindow } from '@/lib/utils';
import { Loader2, MoveHorizontal, Info } from 'lucide-react';

interface ChromosomeBrowserProps {
  chromosomes: { name: string; length: number }[];
  initialChrom?: string;
}

export function ChromosomeBrowser({ chromosomes, initialChrom }: ChromosomeBrowserProps) {
  const { activeGenome } = useGenomeStore();
  const [selectedChrom, setSelectedChrom] = useState(initialChrom || chromosomes[0]?.name);
  const chartTheme = useChartTheme();

  const currentChrom = chromosomes.find(c => c.name === selectedChrom);

  // 1. Fetch GC Windows
  const { data: windows, isLoading: windowsLoading } = useQuery({
    queryKey: ['gc-windows', activeGenome, selectedChrom],
    queryFn: () => dnaApi.getSequenceWindows(activeGenome, selectedChrom, { 
      step: alignToBaseWindow(Math.max(10000, (currentChrom?.length || 0) / 1000)),
      limit: 1000 
    }),
    enabled: !!selectedChrom && !!currentChrom,
  });

  // 2. Fetch Gaps
  const { data: gaps } = useQuery({
    queryKey: ['gaps', activeGenome, selectedChrom],
    queryFn: () => dnaApi.getGaps(activeGenome, selectedChrom),
    enabled: !!selectedChrom,
  });

  const chartData = useMemo(() => {
    if (!windows) return [];
    return windows.map(w => ({
      pos: w.start,
      gc: w.gc_content,
      at: 100 - w.gc_content,
    }));
  }, [windows]);

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
        <div className="space-y-1">
          <h3 className="text-lg font-bold text-slate-900 dark:text-zinc-100 flex items-center gap-2">
            <MoveHorizontal className="h-5 w-5 text-primary" />
            Genomic Landscape Browser
          </h3>
          <p className="text-xs text-slate-500">Interactive exploration of sequence composition and assembly quality.</p>
        </div>

        <Select value={selectedChrom} onValueChange={setSelectedChrom}>
          <SelectTrigger className="w-40 bg-white dark:bg-zinc-900">
            <SelectValue placeholder="Select Chromosome" />
          </SelectTrigger>
          <SelectContent>
            {chromosomes.map(c => (
              <SelectItem key={c.name} value={c.name}>{c.name}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm relative min-h-[450px]">
        {windowsLoading ? (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-white/50 dark:bg-zinc-950/50 backdrop-blur-sm z-10 rounded-xl">
            <Loader2 className="h-8 w-8 animate-spin text-primary" />
            <span className="text-xs font-medium text-slate-500 uppercase tracking-widest">Scanning Sequence...</span>
          </div>
        ) : null}

        <div className="h-[400px] w-full">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={chartData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
              <defs>
                <linearGradient id="colorGc" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#06b6d4" stopOpacity={0.3}/>
                  <stop offset="95%" stopColor="#06b6d4" stopOpacity={0}/>
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" vertical={false} stroke={chartTheme.grid?.line?.stroke} />
              <XAxis 
                dataKey="pos" 
                tickFormatter={(val) => `${(val / 1e6).toFixed(1)}M`}
                fontSize={10}
                tick={{ fill: chartTheme.axis?.ticks?.text?.fill }}
                axisLine={{ stroke: chartTheme.axis?.domain?.line?.stroke }}
              />
              <YAxis 
                fontSize={10}
                tick={{ fill: chartTheme.axis?.ticks?.text?.fill }}
                axisLine={{ stroke: chartTheme.axis?.domain?.line?.stroke }}
                domain={[0, 100]}
                unit="%"
              />
              <Tooltip 
                content={({ active, payload, label }) => {
                  if (active && payload && payload.length) {
                    const parsedLabel = typeof label === 'number' ? label : Number(label);
                    const safeLabel = Number.isFinite(parsedLabel) ? parsedLabel : 0;
                    const rawValue = payload[0]?.value;
                    const gcValue = typeof rawValue === 'number' ? rawValue : Number(rawValue ?? 0);

                    return (
                      <div className="bg-white dark:bg-zinc-900 p-3 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-xl">
                        <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-2">
                          Pos: {formatNumber(safeLabel)} bp
                        </div>
                        <div className="space-y-1">
                          <div className="flex items-center justify-between gap-4">
                            <span className="text-xs text-slate-500">GC Content</span>
                            <span className="text-xs font-bold text-cyan-500">{gcValue.toFixed(2)}%</span>
                          </div>
                          <div className="w-24 h-1 bg-slate-100 dark:bg-zinc-800 rounded-full overflow-hidden">
                            <div className="h-full bg-cyan-500" style={{ width: `${gcValue}%` }} />
                          </div>
                        </div>
                      </div>
                    );
                  }
                  return null;
                }}
              />
              <Area 
                type="monotone" 
                dataKey="gc" 
                stroke="#06b6d4" 
                strokeWidth={2}
                fillOpacity={1} 
                fill="url(#colorGc)" 
                isAnimationActive={true}
              />
              <Brush 
                dataKey="pos" 
                height={30} 
                stroke="#3b82f6" 
                fill={chartTheme.background}
                tickFormatter={(val) => `${(val / 1e6).toFixed(0)}M`}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Legend / Stats overlay */}
        <div className="mt-8 grid grid-cols-2 md:grid-cols-4 gap-4">
           <div className="p-3 bg-slate-50 dark:bg-zinc-900 rounded-xl border border-slate-200 dark:border-zinc-800">
              <div className="text-[10px] font-bold text-slate-400 uppercase tracking-tight mb-1">Assembly Gaps</div>
              <div className="text-lg font-bold text-slate-900 dark:text-zinc-100">{gaps?.length || 0}</div>
           </div>
           <div className="p-3 bg-slate-50 dark:bg-zinc-900 rounded-xl border border-slate-200 dark:border-zinc-800">
              <div className="text-[10px] font-bold text-slate-400 uppercase tracking-tight mb-1">Resolution</div>
              <div className="text-lg font-bold text-slate-900 dark:text-zinc-100">
                {formatNumber(Math.max(10000, Math.floor((currentChrom?.length || 0) / 1000)))} bp
              </div>
           </div>
           <div className="p-3 bg-slate-50 dark:bg-zinc-900 rounded-xl border border-slate-200 dark:border-zinc-800">
              <div className="text-[10px] font-bold text-slate-400 uppercase tracking-tight mb-1">Current Chrom</div>
              <div className="text-lg font-bold text-slate-900 dark:text-zinc-100">{selectedChrom}</div>
           </div>
           <div className="p-3 bg-slate-50 dark:bg-zinc-900 rounded-xl border border-slate-200 dark:border-zinc-800">
              <div className="text-[10px] font-bold text-slate-400 uppercase tracking-tight mb-1">Total Length</div>
              <div className="text-lg font-bold text-slate-900 dark:text-zinc-100">{( (currentChrom?.length || 0) / 1e6 ).toFixed(1)} Mb</div>
           </div>
        </div>
      </Card>

      <div className="flex gap-4 p-4 bg-blue-50/50 dark:bg-blue-900/10 rounded-2xl border border-blue-100 dark:border-blue-900/30">
        <Info className="h-5 w-5 text-blue-500 shrink-0 mt-0.5" />
        <p className="text-xs text-blue-700 dark:text-blue-300 leading-relaxed">
          The <strong>Genomic Landscape Browser</strong> visualizes local GC content variation using a sliding window analysis. 
          Use the <strong>Brush tool</strong> at the bottom to zoom into specific regions and observe fine-grained structural variation.
        </p>
      </div>
    </div>
  );
}
