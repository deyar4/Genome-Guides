"use client"
import React, { useState } from "react";
import { LibraryShell } from "@/components/layout/library-shell";
import { useGenomeStore } from "@/store/use-genome-store";
import { useGenomeData, genomeLengths } from "@/hooks/useGenomeData";
import { Dashboard } from "@/components/Dashboard";
import { InspectorPanel } from "@/components/InspectorPanel";
import { BrowserCanvas, TrackConfig, HoverData } from "@/components/browser/BrowserCanvas";
import { BrowserControls } from "@/components/browser/BrowserControls";
import { Loader2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { dnaApi } from "@/lib/api";
import { useTheme } from "next-themes";

const DEFAULT_TRACKS: TrackConfig[] = [
  { id: 'gc', title: 'GC CONTENT (%)', visible: true, order: 1, height: 92 },
  { id: 'entropy', title: 'SHANNON ENTROPY OF BASE COMPOSITION (BITS)', visible: true, order: 2, height: 92 },
  { id: 'density', title: 'GENE COUNT PER WINDOW', visible: true, order: 3, height: 92 },
  { id: 'repeat-density', title: 'REPEAT COUNT PER WINDOW', visible: true, order: 4, height: 92 },
  { id: 'gaps', title: 'ASSEMBLY GAPS / HOMOPOLYMERS', visible: true, order: 5, height: 58 },
  { id: 'models', title: 'GENE MODELS / REPEATS', visible: true, order: 6, height: 58 },
  { id: 'hecate', title: 'HECATE CROSS-ENTROPY RATE (BITS/BASE, 1 KB WINDOWS)', visible: true, order: 7, height: 128 }
];

export default function GenomeBrowserPage() {
  const { activeGenome } = useGenomeStore();
  return <GenomeBrowser key={activeGenome} activeGenome={activeGenome} />;
}

function GenomeBrowser({ activeGenome }: { activeGenome: keyof typeof genomeLengths }) {
  const { theme } = useTheme();
  
  // Contig names and lengths come from the API; the static table is only a fallback while loading or on error.
  const { data: chromData } = useQuery({
    queryKey: ['chromosomes', activeGenome],
    queryFn: () => dnaApi.getChromosomes(activeGenome),
  });
  const apiLengths = React.useMemo(() => {
    const out: Record<string, number> = {};
    (chromData?.chromosomes ?? [])
      .slice()
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }))
      .forEach(c => { out[c.name] = c.length; });
    return out;
  }, [chromData]);
  const lengths = Object.keys(apiLengths).length > 0 ? apiLengths : (genomeLengths[activeGenome] || {});
  const availableContigs = Object.keys(lengths);
  const initialContig = Object.keys(genomeLengths[activeGenome] || {})[0] || "chr1";
  const initialEnd = Math.min(100000, genomeLengths[activeGenome]?.[initialContig] || 100000);
  
  const [selectedContig, setSelectedContig] = useState<string>(initialContig);
  const [startPos, setStartPos] = useState<number>(0);
  const [endPos, setEndPos] = useState<number>(initialEnd);
  const [hoverCoord, setHoverCoord] = useState<number | null>(null);
  const [hoverData, setHoverData] = useState<HoverData | null>(null);
  const [tracks, setTracks] = useState<TrackConfig[]>(DEFAULT_TRACKS);

  const { currentMaxLen, loading, data, truncatedTracks } = useGenomeData(
    activeGenome,
    selectedContig,
    startPos,
    endPos,
    lengths[selectedContig]
  );

  const handleContigChange = (contig: string) => {
    // reset the viewport so it never exceeds the new contig
    setSelectedContig(contig);
    setStartPos(0);
    setEndPos(Math.min(100000, lengths[contig] || 100000));
    setHoverCoord(null);
    setHoverData(null);
  };

  const handlePan = (bpDelta: number) => {
    const len = endPos - startPos;
    let newStart = startPos + bpDelta;
    let newEnd = endPos + bpDelta;
    if (newStart < 0) {
      newStart = 0;
      newEnd = len;
    } else if (newEnd > currentMaxLen) {
      newEnd = currentMaxLen;
      newStart = Math.max(0, currentMaxLen - len);
    }
    setStartPos(newStart);
    setEndPos(newEnd);
  };

  const handleZoom = (factor: number) => {
    const currentLen = endPos - startPos;
    const targetLen = Math.max(10, Math.min(currentMaxLen, Math.floor(currentLen * factor)));
    const anchor = startPos + currentLen / 2;
    const ratio = (anchor - startPos) / currentLen;
    const newStart = Math.max(0, Math.floor(anchor - targetLen * ratio));
    const newEnd = Math.min(currentMaxLen, newStart + targetLen);
    setStartPos(newStart);
    setEndPos(newEnd);
  };

  const toggleTrack = (id: string) => {
    setTracks(tracks.map(t => t.id === id ? { ...t, visible: !t.visible } : t));
  };

  const moveTrack = (index: number, direction: -1 | 1) => {
    if (index + direction < 0 || index + direction >= tracks.length) return;
    const newTracks = [...tracks];
    const temp = newTracks[index].order;
    newTracks[index].order = newTracks[index + direction].order;
    newTracks[index + direction].order = temp;
    setTracks(newTracks.sort((a, b) => a.order - b.order));
  };

  const setTrackHeight = (id: string, height: number) => {
    setTracks(tracks.map(t => t.id === id ? { ...t, height } : t));
  };

  return (
    <LibraryShell title="Genome Browser" subtitle="Interactive multi-track genome browser with semantic zoom.">
      <div className="space-y-6 pb-20">
        
        {/* CONTROL PANEL */}
        <BrowserControls
          selectedContig={selectedContig}
          setSelectedContig={handleContigChange}
          availableContigs={availableContigs}
          startPos={startPos}
          endPos={endPos}
          handlePan={handlePan}
          handleZoom={handleZoom}
          tracks={tracks}
          toggleTrack={toggleTrack}
          moveTrack={moveTrack}
          setTrackHeight={setTrackHeight}
        />

        {/* DASHBOARD */}
        <Dashboard startPos={startPos} endPos={endPos} currentMaxLen={currentMaxLen} activeTracks={tracks.filter(t => t.visible).length} />

        {/* BROWSER VIEWPORT */}
        <div className="bg-card rounded-xl border border-border overflow-hidden flex flex-col relative shadow-md">
          {loading && (
            <div className="absolute inset-0 bg-background/60 backdrop-blur-xs flex flex-col items-center justify-center gap-2 z-20">
              <Loader2 className="h-6 w-6 text-primary animate-spin" />
              <span className="text-[10px] text-primary uppercase tracking-widest font-mono">Querying Parquet...</span>
            </div>
          )}
          
          <BrowserCanvas 
            startPos={startPos} 
            endPos={endPos} 
            currentMaxLen={currentMaxLen}
            data={data}
            truncatedTracks={truncatedTracks}
            tracks={tracks}
            theme={theme}
            hoverCoord={hoverCoord}
            setHoverCoord={setHoverCoord}
            setHoverData={setHoverData}
            setStartPos={setStartPos}
            setEndPos={setEndPos}
          />
          
          <div className="px-4 py-2 border-t border-border bg-secondary/20 text-[10px] text-muted-foreground flex items-center justify-between">
            <span>Click and drag to pan. Scroll the mouse wheel with Ctrl/Cmd held to zoom at the pointer.</span>
            <span>Viewport: {(endPos - startPos).toLocaleString()} bp (data are binned to at most 1,200 points, never finer than 1 kb)</span>
          </div>
        </div>

        {/* INSPECTOR PANEL */}
        <InspectorPanel hoverCoord={hoverCoord} hoverData={hoverData} />

      </div>
    </LibraryShell>
  );
}
