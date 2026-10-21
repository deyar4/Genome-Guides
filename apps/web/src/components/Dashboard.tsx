import React from "react";

interface DashboardProps {
  startPos: number;
  endPos: number;
  currentMaxLen: number;
  activeTracks?: number;
}

export function Dashboard({ startPos, endPos, currentMaxLen, activeTracks }: DashboardProps) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div className="bg-background p-4 rounded-xl border border-border shadow-sm flex flex-col justify-between">
            <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest">Current Viewport</span>
            <span className="text-sm font-mono font-bold text-foreground mt-1">{startPos.toLocaleString()} – {endPos.toLocaleString()} bp</span>
          </div>

          <div className="bg-background p-4 rounded-xl border border-border shadow-sm flex flex-col justify-between">
            <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest">Viewport Width</span>
            <span className="text-sm font-mono font-bold text-cyan-400 mt-1">{(endPos - startPos).toLocaleString()} bp</span>
          </div>

          <div className="bg-background p-4 rounded-xl border border-border shadow-sm flex flex-col justify-between">
            <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest">Chromosome Limit</span>
            <span className="text-sm font-mono font-bold text-secondary-foreground mt-1">{currentMaxLen.toLocaleString()} bp</span>
          </div>

          <div className="bg-background p-4 rounded-xl border border-border shadow-sm flex flex-col justify-between">
            <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest">Data Streams</span>
            <span className="text-sm font-bold text-secondary-foreground mt-1 flex items-center gap-2">
              <span className="h-2 w-2 rounded-full bg-cyan-400 animate-pulse"></span> {activeTracks ?? 0} visible tracks
            </span>
          </div>
        </div>
  );
}
