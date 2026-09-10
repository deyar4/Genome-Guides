import React from "react";
import { Card } from "@/components/ui/card";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { TrackSettingsDialog } from "./TrackSettingsDialog";
import { TrackConfig } from "./BrowserCanvas";

interface BrowserControlsProps {
  selectedContig: string;
  setSelectedContig: (contig: string) => void;
  availableContigs: string[];
  startPos: number;
  endPos: number;
  handlePan: (bpDelta: number) => void;
  handleZoom: (factor: number) => void;
  tracks: TrackConfig[];
  toggleTrack: (id: string) => void;
  moveTrack: (index: number, direction: -1 | 1) => void;
  setTrackHeight: (id: string, height: number) => void;
}

export function BrowserControls({
  selectedContig,
  setSelectedContig,
  availableContigs,
  startPos,
  endPos,
  handlePan,
  handleZoom,
  tracks,
  toggleTrack,
  moveTrack,
  setTrackHeight
}: BrowserControlsProps) {
  return (
    <Card className="p-4 border-border bg-card shadow-sm flex flex-col md:flex-row items-center gap-4 justify-between">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest">Contig</span>
          <Select value={selectedContig} onValueChange={setSelectedContig}>
            <SelectTrigger className="w-32 h-8">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {availableContigs.map((c) => (
                <SelectItem key={c} value={c}>{c}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      
      <div className="flex items-center gap-2">
        <button onClick={() => handlePan(-Math.floor((endPos - startPos) * 0.25))} className="px-2.5 py-1 bg-secondary text-secondary-foreground border border-border rounded-md text-xs hover:bg-accent hover:text-accent-foreground transition cursor-pointer">← Pan</button>
        <button onClick={() => handleZoom(0.7)} className="px-2.5 py-1 bg-secondary text-secondary-foreground border border-border rounded-md text-xs hover:bg-accent hover:text-accent-foreground transition cursor-pointer">Zoom in</button>
        <button onClick={() => handleZoom(1.3)} className="px-2.5 py-1 bg-secondary text-secondary-foreground border border-border rounded-md text-xs hover:bg-accent hover:text-accent-foreground transition cursor-pointer">Zoom out</button>
        <button onClick={() => handlePan(Math.floor((endPos - startPos) * 0.25))} className="px-2.5 py-1 bg-secondary text-secondary-foreground border border-border rounded-md text-xs hover:bg-accent hover:text-accent-foreground transition cursor-pointer">Pan →</button>
        
        <TrackSettingsDialog 
          tracks={tracks}
          toggleTrack={toggleTrack}
          moveTrack={moveTrack}
          setTrackHeight={setTrackHeight}
        />
      </div>
    </Card>
  );
}
