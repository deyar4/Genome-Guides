import React from "react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Settings2, Eye, EyeOff } from "lucide-react";
import { TrackConfig } from "./BrowserCanvas";

interface TrackSettingsDialogProps {
  tracks: TrackConfig[];
  toggleTrack: (id: string) => void;
  moveTrack: (index: number, direction: -1 | 1) => void;
  setTrackHeight: (id: string, height: number) => void;
}

export function TrackSettingsDialog({ tracks, toggleTrack, moveTrack, setTrackHeight }: TrackSettingsDialogProps) {
  return (
    <Dialog>
      <DialogTrigger asChild>
        <button className="ml-2 px-2.5 py-1 bg-primary text-primary-foreground border border-primary rounded-md text-xs hover:opacity-90 transition cursor-pointer flex items-center gap-1">
          <Settings2 className="h-3 w-3" />
          Tracks
        </button>
      </DialogTrigger>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Configure Tracks</DialogTitle>
        </DialogHeader>
        <div className="space-y-2 mt-4">
          {tracks.map((track, i) => (
            <div key={track.id} className="flex items-center justify-between p-2 rounded-md bg-secondary/50 border border-border">
              <div className="flex items-center gap-3">
                <div className="flex flex-col gap-1">
                  <button onClick={() => moveTrack(i, -1)} disabled={i === 0} className="text-muted-foreground hover:text-foreground disabled:opacity-30">▲</button>
                  <button onClick={() => moveTrack(i, 1)} disabled={i === tracks.length - 1} className="text-muted-foreground hover:text-foreground disabled:opacity-30">▼</button>
                </div>
                <span className="text-xs font-bold font-mono">{track.title}</span>
              </div>
              <div className="flex items-center gap-3">
                <select 
                  className="text-xs bg-background border border-border rounded px-1 py-0.5" 
                  value={track.height} 
                  onChange={(e) => setTrackHeight(track.id, parseInt(e.target.value))}
                >
                  <option value={40}>40px</option>
                  <option value={60}>60px</option>
                  <option value={100}>100px</option>
                  <option value={120}>120px</option>
                  <option value={200}>200px</option>
                </select>
                <button onClick={() => toggleTrack(track.id)} className="p-1 hover:bg-background rounded">
                  {track.visible ? <Eye className="h-4 w-4 text-primary" /> : <EyeOff className="h-4 w-4 text-muted-foreground" />}
                </button>
              </div>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
