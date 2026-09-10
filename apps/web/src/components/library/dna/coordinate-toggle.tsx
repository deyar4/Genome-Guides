"use client"

import React from 'react';
import { useCoordinates } from '@/providers/coordinate-provider';
import { Button } from '@/components/ui/button';
import { Binary, GraduationCap } from 'lucide-react';
import { cn } from '@/lib/utils';

export function CoordinateToggle() {
  const { system, setSystem } = useCoordinates();

  return (
    <div className="flex items-center p-1 bg-slate-100 dark:bg-zinc-900 rounded-lg border border-slate-200 dark:border-zinc-800">
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setSystem('0-based')}
        className={cn(
          "h-7 px-3 text-[10px] font-bold uppercase tracking-widest gap-2 transition-all",
          system === '0-based' 
            ? "bg-white dark:bg-zinc-800 shadow-sm text-primary" 
            : "text-slate-500"
        )}
      >
        <Binary className="h-3.5 w-3.5" />
        0-based
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setSystem('1-based')}
        className={cn(
          "h-7 px-3 text-[10px] font-bold uppercase tracking-widest gap-2 transition-all",
          system === '1-based' 
            ? "bg-white dark:bg-zinc-800 shadow-sm text-primary" 
            : "text-slate-500"
        )}
      >
        <GraduationCap className="h-3.5 w-3.5" />
        1-based
      </Button>
    </div>
  );
}
