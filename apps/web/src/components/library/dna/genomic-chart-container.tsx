import React from 'react';
import { Card } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { AlertCircle, RefreshCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface GenomicChartContainerProps {
  title: string;
  subtitle?: string;
  isLoading?: boolean;
  isError?: boolean;
  onRetry?: () => void;
  children: React.ReactNode;
  className?: string;
  height?: string;
}

export function GenomicChartContainer({
  title,
  subtitle,
  isLoading,
  isError,
  onRetry,
  children,
  className,
  height = 'h-[400px]'
}: GenomicChartContainerProps) {
  return (
    <Card className={className}>
      <div className="p-6 border-b border-slate-100 dark:border-zinc-800 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-tight">{title}</h3>
          {subtitle && <p className="text-xs text-slate-500 dark:text-zinc-400 mt-1">{subtitle}</p>}
        </div>
        {onRetry && (
           <Button variant="ghost" size="icon" onClick={onRetry} className="h-8 w-8 text-slate-400 hover:text-slate-900 dark:hover:text-zinc-100">
             <RefreshCcw className="h-3.5 w-3.5" />
           </Button>
        )}
      </div>
      <div className={`p-6 relative ${height}`}>
        {isLoading ? (
          <div className="absolute inset-0 flex flex-col items-center justify-center p-6 space-y-4">
            <Skeleton className="w-full h-full rounded-lg" />
            <div className="absolute inset-0 flex items-center justify-center">
               <div className="flex flex-col items-center gap-2">
                 <RefreshCcw className="h-5 w-5 text-slate-300 animate-spin" />
                 <span className="text-[10px] font-bold text-slate-400 uppercase tracking-widest">Crunching Data...</span>
               </div>
            </div>
          </div>
        ) : isError ? (
          <div className="absolute inset-0 flex flex-col items-center justify-center p-6 text-center">
            <AlertCircle className="h-8 w-8 text-red-500 mb-2 opacity-50" />
            <h4 className="text-sm font-bold text-slate-900 dark:text-zinc-100">Analysis Failed</h4>
            <p className="text-xs text-slate-500 dark:text-zinc-400 mt-1 max-w-[200px]">
              We encountered an error while processing this genomic region.
            </p>
            {onRetry && (
              <Button variant="outline" size="sm" onClick={onRetry} className="mt-4 h-8 text-[10px] font-bold uppercase tracking-wider">
                Retry Analysis
              </Button>
            )}
          </div>
        ) : (
          children
        )}
      </div>
    </Card>
  );
}
