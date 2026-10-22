import React from 'react';
import { ResponsiveHeatMap } from '@nivo/heatmap';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { GenomicChartContainer } from './genomic-chart-container';

interface DinucleotideHeatmapProps {
  data: {
    id: string;
    data: { x: string; y: number }[];
  }[];
  isLoading?: boolean;
  isError?: boolean;
  onRetry?: () => void;
}

export function DinucleotideHeatmap({ data, isLoading, isError, onRetry }: DinucleotideHeatmapProps) {
  const chartTheme = useChartTheme();

  return (
    <GenomicChartContainer
      title="Dinucleotide Bias (O/E Ratio)"
      subtitle="Observed frequency divided by the frequency expected from the single-base frequencies."
      isLoading={isLoading}
      isError={isError}
      onRetry={onRetry}
    >
      <ResponsiveHeatMap
        data={data}
        margin={{ top: 30, right: 30, bottom: 30, left: 30 }}
        valueFormat=">-.2f"
        axisTop={{
          tickSize: 5,
          tickPadding: 5,
          tickRotation: 0,
          legend: '',
          legendOffset: 46
        }}
        axisLeft={{
          tickSize: 5,
          tickPadding: 5,
          tickRotation: 0,
          legend: '',
          legendOffset: -40
        }}
        colors={{
          type: 'diverging',
          scheme: 'red_yellow_blue',
          divergeAt: 0.5,
          minValue: 0,
          maxValue: 1.5
        }}
        emptyColor="#555555"
        theme={chartTheme}
        labelTextColor={{
          from: 'color',
          modifiers: [['darker', 2]]
        }}
        isInteractive={true}
        hoverTarget="cell"
        animate={true}
        tooltip={({ cell }) => (
          <div className="bg-white dark:bg-zinc-900 p-3 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-xl min-w-[120px]">
             <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-1">{cell.serieId} + {cell.data.x}</div>
             <div className="flex items-center justify-between">
                <span className="text-xs text-slate-500">O/E Ratio</span>
                <span className="text-sm font-bold text-slate-900 dark:text-zinc-100">{cell.data.y.toFixed(3)}</span>
             </div>
             <div className="mt-2 h-1 w-full bg-slate-100 dark:bg-zinc-800 rounded-full overflow-hidden">
                <div 
                  className="h-full bg-primary transition-all" 
                  style={{ 
                    width: `${Math.min(100, (cell.data.y / 1.5) * 100)}%`,
                    backgroundColor: cell.data.y > 1 ? '#ef4444' : cell.data.y < 0.5 ? '#3b82f6' : '#f59e0b'
                  }} 
                />
             </div>
          </div>
        )}
      />
    </GenomicChartContainer>
  );
}
