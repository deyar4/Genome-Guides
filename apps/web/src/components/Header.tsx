import React from "react";

interface HeaderProps {
  isServerLive: boolean | null;
  selectedGenome: string;
  setSelectedGenome: (val: string) => void;
  selectedContig: string;
  setSelectedContig: (val: string) => void;
  genomeLengths: Record<string, Record<string, number>>;
}

export function Header({ isServerLive, selectedGenome, setSelectedGenome, selectedContig, setSelectedContig, genomeLengths }: HeaderProps) {
  return (
    <header className="border-b border-border bg-background/85 backdrop-blur-md px-6 py-4 flex flex-col md:flex-row md:items-center justify-between gap-4 sticky top-0 z-50">
        <div className="flex items-center gap-3">
          <div className="h-8 w-8 rounded-lg bg-primary flex items-center justify-center font-bold text-primary-foreground tracking-tighter">GG</div>
          <div>
            <h1 className="text-md font-bold tracking-tight text-foreground flex items-center gap-2">
              Genome Guides Track Engine
              {isServerLive === true && <span className="inline-flex items-center rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium text-emerald-400 ring-1 ring-inset ring-emerald-500/20">Live DB</span>}
              {isServerLive === false && <span className="inline-flex items-center rounded-full bg-cyan-500/10 px-2 py-0.5 text-[10px] font-medium text-cyan-400 ring-1 ring-inset ring-cyan-500/20">Local Sim</span>}
            </h1>
            <p className="text-[11px] text-muted-foreground">High-throughput coordinate-based multi-track browser</p>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <div className="flex flex-col gap-1">
            <label className="text-[9px] font-bold text-muted-foreground uppercase tracking-widest">Reference</label>
            <select value={selectedGenome} onChange={(e) => setSelectedGenome(e.target.value)} className="bg-secondary border border-border text-foreground text-xs rounded-lg px-3 py-1.5 focus:outline-none focus:ring-1 focus:ring-cyan-500 cursor-pointer">
              <option value="chrm">Mitochondrial (chrM)</option>
              <option value="tiny">Synthetic Tiny (chr1)</option>
              <option value="t2t">T2T-CHM13v2.0 (Full)</option>
            </select>
          </div>

          <div className="flex flex-col gap-1">
            <label className="text-[9px] font-bold text-muted-foreground uppercase tracking-widest">Contig</label>
            <select value={selectedContig} onChange={(e) => setSelectedContig(e.target.value)} className="bg-secondary border border-border text-foreground text-xs rounded-lg px-3 py-1.5 focus:outline-none focus:ring-1 focus:ring-cyan-500 cursor-pointer">
              {Object.keys(genomeLengths[selectedGenome] || {}).map((contig) => (
                <option key={contig} value={contig}>{contig}</option>
              ))}
            </select>
          </div>
        </div>
      </header>
  );
}
