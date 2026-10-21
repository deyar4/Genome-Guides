import { useState, useRef, useEffect, useMemo, useCallback } from "react";
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

export function useGenomeData(selectedGenome: string, selectedContig: string, startPos: number, endPos: number) {
  const currentMaxLen = useMemo(() => genomeLengths[selectedGenome]?.[selectedContig] || 100000, [selectedGenome, selectedContig]);
  
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

  useEffect(() => {
    clearCache();
  }, [selectedGenome, selectedContig]);

  // Simulation Data Generator for Offline Mode
  const generateSimulatedData = useCallback((genome: string, contig: string, start: number, end: number) => {
    const len = end - start;
    const gc: TrackDataPoint[] = [];
    const numPoints = 600;
    const step = len / numPoints;

    for (let i = 0; i < numPoints; i++) {
      const coord = Math.floor(start + i * step);
      const wave1 = Math.sin(coord / 1500) * 8;
      const wave2 = Math.cos(coord / 300) * 4;
      const wave3 = Math.sin(coord / 50) * 2;
      const avg_value = Math.max(0, Math.min(100, 42 + wave1 + wave2 + wave3));
      const skewWave = Math.sin(coord / 2000) * 0.15 + Math.cos(coord / 400) * 0.05;

      gc.push({
        bin_start: coord,
        avg_value: avg_value,
        min_value: Math.max(0, avg_value - 3),
        max_value: Math.min(100, avg_value + 3),
        avg_skew: skewWave,
      });
    }

    const gaps: GapFeature[] = [];
    if (genome === "chrm") {
      gaps.push({ contig, start_pos: 4000, end_pos: 4120, gap_length: 120, gap_type: "assembly_gap" });
      gaps.push({ contig, start_pos: 11200, end_pos: 11250, gap_length: 50, gap_type: "assembly_gap" });
    }

    const homopolymers: HomopolymerFeature[] = [];
    const bases = ["A", "C", "G", "T"];
    const seed = genome === "chrm" ? 42 : 101;
    const spacing = genome === "tiny" ? 8 : 1200;
    const size = genome === "tiny" ? 5 : 18;
    for (let pos = Math.floor(start / spacing) * spacing; pos < end; pos += spacing) {
      if (pos < 0) continue;
      const base = bases[Math.floor(Math.abs(Math.sin(pos + seed)) * 4)];
      const hLen = Math.floor(Math.abs(Math.cos(pos + seed)) * 12) + size;
      homopolymers.push({ contig, start_pos: pos, end_pos: pos + hLen, base, run_length: hLen });
    }

    const entropy: ShannonEntropyPoint[] = [];
    for (let i = 0; i < numPoints; i++) {
      const coord = Math.floor(start + i * step);
      const entVal = 1.35 + Math.sin(coord / 800) * 0.25 + Math.cos(coord / 200) * 0.1;
      entropy.push({
        bin_start: coord,
        avg_shannon_entropy: entVal,
        min_shannon_entropy: entVal - 0.05,
        max_shannon_entropy: entVal + 0.05,
        avg_telomeric_repeats: coord > currentMaxLen - 5000 || coord < 5000 ? 5 : 0
      });
    }

    const geneDensity: GeneDensityPoint[] = [];
    const repeatDensity: RepeatDensityPoint[] = [];
    for (let i = 0; i < numPoints; i++) {
      const coord = Math.floor(start + i * step);
      const gdVal = Math.max(0, 2 + Math.sin(coord / 25000) * 5 + Math.cos(coord / 5000) * 2);
      const rdVal = Math.max(0, 15 + Math.sin(coord / 15000) * 20 + Math.cos(coord / 3000) * 8);
      geneDensity.push({ bin_start: coord, avg_count: gdVal });
      repeatDensity.push({ bin_start: coord, avg_te_count: rdVal });
    }

    const geneModels: GeneModelFeature[] = [];
    const repeatFamilies: RepeatFamilyFeature[] = [];
    if (genome === "chrm") {
      geneModels.push({ contig, start_pos: 200, end_pos: 1200, gene_id: "MT-TF", gene_name: "MT-TF", biotype: "tRNA", strand: "+" });
      geneModels.push({ contig, start_pos: 1600, end_pos: 3200, gene_id: "MT-RNR1", gene_name: "MT-RNR1", biotype: "rRNA", strand: "+" });
    }

    return { gc, gaps, homopolymers, entropy, geneDensity, repeatDensity, geneModels, repeatFamilies, hecate_mprob: [] };
  }, [currentMaxLen]);

  useEffect(() => {
    const fetchTrackData = async () => {
      const viewportLen = endPos - startPos;
      const cache = cacheRef.current;
      const isCached = cache && cache.genome === selectedGenome && cache.contig === selectedContig && cache.start <= startPos && cache.end >= endPos;

      if (isCached) {
        setGcData(cache.gc.filter((d) => d.bin_start >= startPos && d.bin_start <= endPos));
        setGapData(cache.gaps.filter((g) => g.start_pos < endPos && g.end_pos > startPos));
        setHomopolymerData(cache.homopolymers.filter((h) => h.start_pos < endPos && h.end_pos > startPos));
        setEntropyData(cache.entropy.filter((e) => e.bin_start >= startPos && e.bin_start <= endPos));
        setGeneDensityData(cache.geneDensity.filter((d) => d.bin_start >= startPos && d.bin_start <= endPos));
        setRepeatDensityData(cache.repeatDensity.filter((d) => d.bin_start >= startPos && d.bin_start <= endPos));
        setGeneModelData(cache.geneModels.filter((g) => g.start_pos < endPos && g.end_pos > startPos));
        setRepeatFamilyData(cache.repeatFamilies.filter((r) => r.start_pos < endPos && r.end_pos > startPos));
        setHecateMprobData(cache.hecate_mprob.filter((h) => h.bin_start >= startPos && h.bin_start <= endPos));
        return;
      }

      const pad = Math.floor(viewportLen * 0.5);
      const prefetchStart = Math.max(0, startPos - pad);
      const prefetchEnd = Math.min(currentMaxLen, endPos + pad);

      setLoading(true);
      const apiBaseUrl = (process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000/api/v1").replace(/\/$/, "");
      const baseUrl = `${apiBaseUrl}/tracks`;
      
      try {
        const fetchTrack = async (trackName: string) => {
          const res = await fetch(`${baseUrl}/${selectedGenome}/${selectedContig}/${trackName}?start=${prefetchStart}&end=${prefetchEnd}&max_points=1200`);
          if (!res.ok) {
            console.warn(`[Genome Browser] ${trackName} returned HTTP ${res.status}`);
            return null;
          }
          const json = await res.json();
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
          avg_mean_entropy: number | null;
          min_mean_entropy: number | null;
          max_mean_entropy: number | null;
        }) => ({
          bin_start: d.bin_start,
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
          hecate_mprob: mappedHecateRes ?? []
        };

      } catch {
        setIsServerLive(false);
        cacheRef.current = {
          genome: selectedGenome, contig: selectedContig, start: prefetchStart, end: prefetchEnd,
          gc: [], gaps: [], homopolymers: [], entropy: [], geneDensity: [],
          repeatDensity: [], geneModels: [], repeatFamilies: [], hecate_mprob: []
        };
      } finally {
        const c = cacheRef.current;
        if (c) {
          setGcData(c.gc.filter((d) => d.bin_start >= startPos && d.bin_start <= endPos));
          setGapData(c.gaps.filter((g) => g.start_pos < endPos && g.end_pos > startPos));
          setHomopolymerData(c.homopolymers.filter((h) => h.start_pos < endPos && h.end_pos > startPos));
          setEntropyData(c.entropy.filter((e) => e.bin_start >= startPos && e.bin_start <= endPos));
          setGeneDensityData(c.geneDensity.filter((d) => d.bin_start >= startPos && d.bin_start <= endPos));
          setRepeatDensityData(c.repeatDensity.filter((d) => d.bin_start >= startPos && d.bin_start <= endPos));
          setGeneModelData(c.geneModels.filter((g) => g.start_pos < endPos && g.end_pos > startPos));
          setRepeatFamilyData(c.repeatFamilies.filter((r) => r.start_pos < endPos && r.end_pos > startPos));
          setHecateMprobData(c.hecate_mprob.filter((h) => h.bin_start >= startPos && h.bin_start <= endPos));
        }
        setLoading(false);
      }
    };
    
    const delayDebounce = setTimeout(fetchTrackData, 50);
    return () => clearTimeout(delayDebounce);
  }, [selectedGenome, selectedContig, startPos, endPos, currentMaxLen, generateSimulatedData]);

  return {
    currentMaxLen, isServerLive, loading,
    data: { gcData, gapData, homopolymerData, entropyData, geneDensityData, repeatDensityData, geneModelData, repeatFamilyData, hecateMprobData }
  };
}
