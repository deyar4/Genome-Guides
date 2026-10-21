import { useState, useRef, useEffect, useMemo } from "react";
import { TrackDataPoint, GapFeature, HomopolymerFeature, ShannonEntropyPoint, GeneDensityPoint, RepeatDensityPoint, GeneModelFeature, RepeatFamilyFeature, TrackCache, HecateMprobPoint } from "../types";

export const genomeLengths: Record<string, Record<string, number>> = {
  chrm: { chrM: 16569 },
  tiny: { chr1: 40 },
  t2t: {
    chr1: 248387328, chr2: 242696752, chr3: 201105948, chr4: 193574630, chr5: 182045439,
    chr6: 172126658, chr7: 160567428, chr8: 146259331, chr9: 150617247, chr10: 134753305,
    chr11: 135127769, chr12: 133324548, chr13: 113566686, chr14: 101161492, chr15: 99753195,
    chr16: 96330374, chr17: 84276897, chr18: 80542538, chr19: 61707364, chr20: 66210255,
    chr21: 45090682, chr22: 41324358, chrX: 154259566, chrY: 62460029,
  },
};

// maxLenOverride: contig length from the API (preferred); the static table above is only a fallback.
export function useGenomeData(selectedGenome: string, selectedContig: string, startPos: number, endPos: number, maxLenOverride?: number) {
  const currentMaxLen = useMemo(() => maxLenOverride || genomeLengths[selectedGenome]?.[selectedContig] || 100000, [selectedGenome, selectedContig, maxLenOverride]);
  
  const [isServerLive, setIsServerLive] = useState<boolean | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  
  const cacheRef = useRef<TrackCache | null>(null);
  const clearCache = () => { cacheRef.current = null; };

  const [gcData, setGcData] = useState<TrackDataPoint[]>([]);
  const [gapData, setGapData] = useState<GapFeature[]>([]);
  const [homopolymerData, setHomopolymerData] = useState<HomopolymerFeature[]>([]);
  const [entropyData, setEntropyData] = useState<ShannonEntropyPoint[]>([]);
  const [geneDensityData, setGeneDensityData] = useState<GeneDensityPoint[]>([]);
  const [repeatDensityData, setRepeatDensityData] = useState<RepeatDensityPoint[]>([]);
  const [geneModelData, setGeneModelData] = useState<GeneModelFeature[]>([]);
  const [repeatFamilyData, setRepeatFamilyData] = useState<RepeatFamilyFeature[]>([]);
  const [hecateMprobData, setHecateMprobData] = useState<HecateMprobPoint[]>([]);
  // Feature tracks are capped server-side (`truncated: true`): surfaced so missing features are not read as absent.
  const [truncatedTracks, setTruncatedTracks] = useState<Record<string, boolean>>({});

  useEffect(() => {
    clearCache();
  }, [selectedGenome, selectedContig]);

  useEffect(() => {
    const fetchTrackData = async () => {
      const viewportLen = endPos - startPos;
      const cache = cacheRef.current;
      const isCached = cache && cache.genome === selectedGenome && cache.contig === selectedContig && cache.start <= startPos && cache.end >= endPos
        // zooming in must refetch: cached bins are (cache span / max_points) wide and would stay coarse
        && (cache.end - cache.start) <= 3 * viewportLen;

      if (isCached) {
        setGcData(cache.gc.filter((d) => (d.bin_end ?? d.bin_start + 1) > startPos && d.bin_start <= endPos));
        setGapData(cache.gaps.filter((g) => g.start_pos < endPos && g.end_pos > startPos));
        setHomopolymerData(cache.homopolymers.filter((h) => h.start_pos < endPos && h.end_pos > startPos));
        setEntropyData(cache.entropy.filter((e) => (e.bin_end ?? e.bin_start + 1) > startPos && e.bin_start <= endPos));
        setGeneDensityData(cache.geneDensity.filter((d) => (d.bin_end ?? d.bin_start + 1) > startPos && d.bin_start <= endPos));
        setRepeatDensityData(cache.repeatDensity.filter((d) => (d.bin_end ?? d.bin_start + 1) > startPos && d.bin_start <= endPos));
        setGeneModelData(cache.geneModels.filter((g) => g.start_pos < endPos && g.end_pos > startPos));
        setRepeatFamilyData(cache.repeatFamilies.filter((r) => r.start_pos < endPos && r.end_pos > startPos));
        setHecateMprobData(cache.hecate_mprob.filter((h) => (h.bin_end ?? h.bin_start + 1) > startPos && h.bin_start <= endPos));
        setTruncatedTracks(cache.truncated);
        return;
      }

      const pad = Math.floor(viewportLen * 0.5);
      const prefetchStart = Math.max(0, startPos - pad);
      const prefetchEnd = Math.min(currentMaxLen, endPos + pad);

      setLoading(true);
      const apiBaseUrl = (process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000/api/v1").replace(/\/$/, "");
      const baseUrl = `${apiBaseUrl}/tracks`;
      
      const truncated: Record<string, boolean> = {};
      try {
        const fetchTrack = async (trackName: string) => {
          const res = await fetch(`${baseUrl}/${selectedGenome}/${selectedContig}/${trackName}?start=${prefetchStart}&end=${prefetchEnd}&max_points=1200`);
          if (!res.ok) {
            console.warn(`[Genome Browser] ${trackName} returned HTTP ${res.status}`);
            return null;
          }
          const json = await res.json();
          if (json.truncated) truncated[trackName] = true;
          return json.data;
        };

        const [
          gcDataRes, hecateRes, gapRes,
          homoRes, entropyRes, geneDensityRes,
          repeatDensityRes, geneModelRes, repeatFamilyRes
        ] = await Promise.all([
          fetchTrack("gc_content"),
          fetchTrack("hecate_mprob"),
          fetchTrack("assembly_gaps"),
          fetchTrack("homopolymer_genomic_tracts"),
          fetchTrack("shannon_entropy"),
          fetchTrack("gene_density"),
          fetchTrack("repeatmasker_density"),
          fetchTrack("gene_models"),
          fetchTrack("repeatmasker_families")
        ]);

        // Track column is the cross-entropy rate `mean_entropy` (bits/base); NULL stays null (no `?? 0`, audit 05 P1/P2)
        const mappedHecateRes = hecateRes ? hecateRes.map((d: {
          bin_start: number;
          bin_end?: number;
          avg_mean_entropy: number | null;
          min_mean_entropy: number | null;
          max_mean_entropy: number | null;
        }) => ({
          bin_start: d.bin_start,
          bin_end: d.bin_end,
          mean_entropy: d.avg_mean_entropy ?? null,
          min_entropy: d.min_mean_entropy ?? null,
          max_entropy: d.max_mean_entropy ?? null
        })) : null;

        setIsServerLive(true);
        cacheRef.current = {
          genome: selectedGenome, contig: selectedContig, start: prefetchStart, end: prefetchEnd,
          gc: gcDataRes ?? [],
          gaps: gapRes ?? [],
          homopolymers: homoRes ?? [],
          entropy: entropyRes ?? [],
          geneDensity: geneDensityRes ?? [],
          repeatDensity: repeatDensityRes ?? [],
          geneModels: geneModelRes ?? [],
          repeatFamilies: repeatFamilyRes ?? [],
          hecate_mprob: mappedHecateRes ?? [],
          truncated
        };

      } catch {
        setIsServerLive(false);
        cacheRef.current = {
          genome: selectedGenome, contig: selectedContig, start: prefetchStart, end: prefetchEnd,
          gc: [], gaps: [], homopolymers: [], entropy: [], geneDensity: [],
          repeatDensity: [], geneModels: [], repeatFamilies: [], hecate_mprob: [], truncated: {}
        };
      } finally {
        const c = cacheRef.current;
        if (c) {
          setGcData(c.gc.filter((d) => (d.bin_end ?? d.bin_start + 1) > startPos && d.bin_start <= endPos));
          setGapData(c.gaps.filter((g) => g.start_pos < endPos && g.end_pos > startPos));
          setHomopolymerData(c.homopolymers.filter((h) => h.start_pos < endPos && h.end_pos > startPos));
          setEntropyData(c.entropy.filter((e) => (e.bin_end ?? e.bin_start + 1) > startPos && e.bin_start <= endPos));
          setGeneDensityData(c.geneDensity.filter((d) => (d.bin_end ?? d.bin_start + 1) > startPos && d.bin_start <= endPos));
          setRepeatDensityData(c.repeatDensity.filter((d) => (d.bin_end ?? d.bin_start + 1) > startPos && d.bin_start <= endPos));
          setGeneModelData(c.geneModels.filter((g) => g.start_pos < endPos && g.end_pos > startPos));
          setRepeatFamilyData(c.repeatFamilies.filter((r) => r.start_pos < endPos && r.end_pos > startPos));
          setHecateMprobData(c.hecate_mprob.filter((h) => (h.bin_end ?? h.bin_start + 1) > startPos && h.bin_start <= endPos));
          setTruncatedTracks(c.truncated);
        }
        setLoading(false);
      }
    };
    
    const delayDebounce = setTimeout(fetchTrackData, 50);
    return () => clearTimeout(delayDebounce);
  }, [selectedGenome, selectedContig, startPos, endPos, currentMaxLen]);

  return {
    currentMaxLen, isServerLive, loading, truncatedTracks,
    data: { gcData, gapData, homopolymerData, entropyData, geneDensityData, repeatDensityData, geneModelData, repeatFamilyData, hecateMprobData }
  };
}
