import React from 'react';
import { Card } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import { ResponsiveLine } from '@nivo/line';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';

interface GenomicMetricCardProps {
  label: string;
  fullName?: string;
  value: string | number;
  unit?: string;
  color?: string;
  sparklineData?: { x: string | number; y: number }[];
  description?: string;
  className?: string;
}

export function GenomicMetricCard({
  label,
  fullName,
  value,
  unit,
  color = '#3b82f6',
  sparklineData,
  description,
  className,
}: GenomicMetricCardProps) {
  const chartTheme = useChartTheme();

  return (
    <Card className={cn("p-6 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm relative group overflow-hidden transition-all duration-300 hover:shadow-md", className)}>
      <TooltipProvider>
        <Tooltip>
          <TooltipTrigger asChild>
            <div className="flex items-center justify-between mb-3 relative z-10 cursor-help">
              <div className="flex flex-col">
                <span className="text-[10px] font-bold uppercase tracking-widest text-slate-400 group-hover:text-slate-500 transition-colors">{fullName || label}</span>
                <span className="text-2xl font-mono font-bold text-slate-900 dark:text-zinc-100 mt-1">
                  {value}{unit && <span className="text-xs ml-1 opacity-50 font-sans">{unit}</span>}
                </span>
              </div>
              <div className="w-3 h-3 rounded-full shadow-sm ring-2 ring-white dark:ring-zinc-900" style={{ backgroundColor: color }} />
            </div>
          </TooltipTrigger>
          {description && (
            <TooltipContent side="top" className="max-w-[280px] p-3 text-xs leading-relaxed bg-white dark:bg-zinc-900 border-slate-200 dark:border-zinc-800 shadow-xl">
              <p className="text-slate-600 dark:text-zinc-400">{description}</p>
            </TooltipContent>
          )}
        </Tooltip>
      </TooltipProvider>

      {description && (
        <p className="text-[10px] text-slate-400 dark:text-zinc-500 font-medium leading-relaxed relative z-10 max-w-[80%]">
          {description}
        </p>
      )}

      {sparklineData && sparklineData.length > 0 && (
        <div className="h-16 w-full absolute bottom-0 left-0 opacity-20 group-hover:opacity-40 transition-all duration-500 pointer-events-none">
          <ResponsiveLine
            data={[{ id: label, data: sparklineData }]}
            margin={{ top: 10, right: 0, bottom: 0, left: 0 }}
            xScale={{ type: 'point' }}
            yScale={{ type: 'linear', min: 'auto', max: 'auto' }}
            enablePoints={false}
            enableGridX={false}
            enableGridY={false}
            enableArea={true}
            colors={[color]}
            theme={chartTheme}
            animate={true}
          />
        </div>
      )}
    </Card>
  );
}
