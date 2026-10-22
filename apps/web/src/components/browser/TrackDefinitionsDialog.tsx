import React from "react";
import { useQuery } from "@tanstack/react-query";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { BookOpen } from "lucide-react";
import { tracksApi } from "@/lib/api";
import { TrackMetadata } from "@/types/api";

const TRACK_TITLES: Record<string, string> = {
  gc_content: "GC content and GC skew",
  shannon_entropy: "Shannon entropy of base composition",
  hecate_mprob: "Hecate cross-entropy rate",
  cpg_islands: "CpG islands",
  gene_density: "Gene count per 100 kb",
  repeatmasker_density: "RepeatMasker records per 100 kb",
  gene_models: "Gene models",
  repeatmasker_families: "RepeatMasker records",
  assembly_gaps: "Assembly gaps",
  homopolymer_genomic_tracts: "Homopolymer tracts (>= 10 bases)",
};

/** Exact definition, units, log base and window of each track, as served by /tracks/metadata (D17). */
export function TrackDefinitionsDialog() {
  const [open, setOpen] = React.useState(false);
  const { data, isLoading, isError } = useQuery({
    queryKey: ["track-metadata"],
    queryFn: tracksApi.getMetadata,
    enabled: open,
    staleTime: Infinity,
  });
  const { binning, ...tracks } = (data ?? {}) as TrackMetadata;

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <button className="ml-2 px-2.5 py-1 bg-secondary text-secondary-foreground border border-border rounded-md text-xs hover:bg-accent hover:text-accent-foreground transition cursor-pointer flex items-center gap-1">
          <BookOpen className="h-3 w-3" />
          Definitions
        </button>
      </DialogTrigger>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Track definitions</DialogTitle>
        </DialogHeader>
        {isLoading && <p className="text-xs text-muted-foreground">Loading definitions...</p>}
        {isError && <p className="text-xs text-red-500">Could not load the track definitions.</p>}
        <div className="space-y-6 mt-2">
          {Object.entries(tracks).map(([name, meta]) => (
            <section key={name}>
              <h4 className="text-sm font-bold">{TRACK_TITLES[name] ?? name}</h4>
              <p className="text-[10px] font-mono text-muted-foreground uppercase tracking-widest mb-2">
                {name} · {meta.window_bp === null ? "interval track (not binned)" : `base window ${meta.window_bp.toLocaleString()} bp`}
              </p>
              <dl className="space-y-2">
                {Object.entries(meta.columns).map(([col, text]) => (
                  <div key={col}>
                    <dt className="text-xs font-mono font-bold">{col}</dt>
                    <dd className="text-xs text-muted-foreground leading-relaxed">{text}</dd>
                  </div>
                ))}
              </dl>
              {meta.source && <p className="text-[10px] text-muted-foreground mt-2">Produced by {meta.source}</p>}
            </section>
          ))}
          {Array.isArray(binning) && binning.length > 0 && (
            <section>
              <h4 className="text-sm font-bold">Binning (all window tracks)</h4>
              <ul className="list-disc pl-4 mt-2 space-y-1">
                {binning.map((note, i) => (
                  <li key={i} className="text-xs text-muted-foreground leading-relaxed">{note}</li>
                ))}
              </ul>
            </section>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
