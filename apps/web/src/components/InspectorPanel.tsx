import React from "react";

interface InspectorPanelProps {
  hoverCoord: number | null;
  hoverData: {
    gc?: number;
    skew?: number | null;
    gap?: string;
    homopolymer?: string;
    entropy?: number;
    geneDensity?: number;
    repeatDensity?: number;
    gene?: string;
    repeat?: string;
    hecate_entropy?: number | null;
  } | null;
}

export function InspectorPanel({ hoverCoord, hoverData }: InspectorPanelProps) {
  return (
    <div className="bg-background p-6 rounded-xl border border-border flex flex-col gap-4">
          <div className="border-b border-border pb-2">
            <h3 className="text-xs font-bold uppercase tracking-widest text-muted-foreground">Position Inspector</h3>
            <p className="text-[11px] text-muted-foreground">Move your pointer over the canvas to inspect DNA, RNA, protein, and repeat metrics in real-time.</p>
          </div>

          {hoverCoord !== null && hoverData ? (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 font-mono text-xs">
              <div className="p-3 bg-card border border-border rounded-lg">
                <div className="text-[9px] font-bold text-muted-foreground uppercase">Coordinate & Complexity</div>
                <div className="text-foreground font-bold mt-1">{hoverCoord.toLocaleString()} bp</div>
                <div className="text-chart-3 font-bold mt-1">Entropy: {hoverData.entropy !== undefined ? hoverData.entropy.toFixed(3) : "N/A"}</div>
              </div>
              <div className="p-3 bg-card border border-border rounded-lg">
                <div className="text-[9px] font-bold text-muted-foreground uppercase">Composition (GC)</div>
                <div className="text-chart-1 font-bold mt-1">GC Content: {hoverData.gc != null ? `${hoverData.gc.toFixed(2)}%` : "N/A"}</div>
                <div className="text-chart-4 font-bold mt-1">GC Skew: {hoverData.skew != null ? hoverData.skew.toFixed(4) : "N/A"}</div>
              </div>
              <div className="p-3 bg-card border border-border rounded-lg">
                <div className="text-[9px] font-bold text-muted-foreground uppercase">Annotation Densities</div>
                <div className="text-chart-2 font-bold mt-1">Gene Density: {hoverData.geneDensity !== undefined ? hoverData.geneDensity.toFixed(2) : "N/A"}</div>
                <div className="text-chart-3 font-bold mt-1">Repeat Density: {hoverData.repeatDensity !== undefined ? hoverData.repeatDensity.toFixed(2) : "N/A"}</div>
                <div className="text-chart-1 font-bold mt-1">Hecate cross-entropy: {hoverData.hecate_entropy != null ? `${hoverData.hecate_entropy.toFixed(3)} bits/base` : "N/A"}</div>
              </div>
              <div className="p-3 bg-card border border-border rounded-lg">
                <div className="text-[9px] font-bold text-muted-foreground uppercase">Feature Inspections</div>
                <div className="text-secondary-foreground mt-1 space-y-1">
                  {hoverData.gene && <div className="text-purple-400">🧬 Gene: {hoverData.gene}</div>}
                  {hoverData.repeat && <div className="text-amber-500">🔁 Repeat: {hoverData.repeat}</div>}
                  {hoverData.gap && <div className="text-red-400">⚠️ Gap: {hoverData.gap}</div>}
                  {hoverData.homopolymer && <div className="text-emerald-400">🧬 Run: {hoverData.homopolymer}</div>}
                  {!hoverData.gene && !hoverData.repeat && !hoverData.gap && !hoverData.homopolymer && <div className="text-muted-foreground">No feature overlaps</div>}
                </div>
              </div>
            </div>
          ) : (
            <div className="py-8 text-center text-xs text-stone-600 italic">Move pointer over the canvas tracks to display local metrics</div>
          )}
        </div>
  );
}
