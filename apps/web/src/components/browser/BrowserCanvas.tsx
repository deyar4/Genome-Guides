"use client"
import React, { useRef, useEffect } from "react";
import { TrackDataPoint, GapFeature, HomopolymerFeature, ShannonEntropyPoint, GeneDensityPoint, RepeatDensityPoint, GeneModelFeature, RepeatFamilyFeature, HecateMprobPoint } from "@/types";

export interface TrackConfig {
  id: string;
  title: string;
  visible: boolean;
  order: number;
  height: number;
}

// null = no data at this position (shown as N/A); undefined only before a value is looked up
export interface HoverData {
  gc?: number | null;
  skew?: number | null;
  entropy?: number | null;
  geneDensity?: number | null;
  repeatDensity?: number | null;
  hecate_entropy?: number | null;
  gene?: string;
  repeat?: string;
  gap?: string;
  homopolymer?: string;
}

interface BrowserCanvasProps {
  startPos: number;
  endPos: number;
  currentMaxLen: number;
  data: {
    gcData: TrackDataPoint[];
    gapData: GapFeature[];
    homopolymerData: HomopolymerFeature[];
    entropyData: ShannonEntropyPoint[];
    geneDensityData: GeneDensityPoint[];
    repeatDensityData: RepeatDensityPoint[];
    geneModelData: GeneModelFeature[];
    repeatFamilyData: RepeatFamilyFeature[];
    hecateMprobData: HecateMprobPoint[];
  };
  /** track names the API capped (truncated: true) for the current window */
  truncatedTracks?: Record<string, boolean>;
  tracks: TrackConfig[];
  theme: string | undefined;
  hoverCoord: number | null;
  setHoverCoord: (coord: number | null) => void;
  setHoverData: (data: HoverData | null) => void;
  setStartPos: (pos: number) => void;
  setEndPos: (pos: number) => void;
}

export function BrowserCanvas({ startPos, endPos, currentMaxLen, data, truncatedTracks = {}, tracks, theme, hoverCoord, setHoverCoord, setHoverData, setStartPos, setEndPos }: BrowserCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const dragStartRef = useRef<{ x: number; start: number; end: number } | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  // Canvas pixel width follows the container so text and features are not stretched/squashed by CSS scaling.
  const [canvasWidth, setCanvasWidth] = React.useState(1800);
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const update = () => setCanvasWidth(Math.max(320, Math.floor(el.clientWidth)));
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  
  const { gcData, gapData, homopolymerData, entropyData, geneDensityData, repeatDensityData, geneModelData, repeatFamilyData, hecateMprobData } = data;

  const visibleTracks = React.useMemo(() => {
    return [...tracks].filter(t => t.visible).sort((a, b) => a.order - b.order);
  }, [tracks]);
  const trackPadding = 12;
  const rulerHeight = 40;
  const totalHeight = rulerHeight + visibleTracks.reduce((sum, t) => sum + t.height + trackPadding, 0) + trackPadding;

  const isDark = theme === "dark";
  const colors = React.useMemo(() => ({
    bg: isDark ? "#0c0a09" : "#f8fafc",
    rulerBg: isDark ? "#1c1917" : "#e2e8f0",
    rulerLine: isDark ? "#2e2a24" : "#cbd5e1",
    tickLine: isDark ? "#44403c" : "#94a3b8",
    tickText: isDark ? "#a8a29e" : "#475569",
    gridLine: isDark ? "rgba(148, 163, 184, 0.12)" : "rgba(100, 116, 139, 0.16)",
    trackBg: isDark ? "#101827" : "#ffffff",
    trackBorder: isDark ? "#26354d" : "#dbe4f0",
    trackTitle: isDark ? "#94a3b8" : "#475569",
    hoverLine: isDark ? "rgba(255, 255, 255, 0.18)" : "rgba(0, 0, 0, 0.2)",
    hoverTextBg: isDark ? "#22d3ee" : "#0284c7",
    hoverText: isDark ? "#000" : "#fff",
  }), [isDark]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    
    const width = canvas.width;
    const height = canvas.height;
    
    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = colors.bg;
    ctx.fillRect(0, 0, width, height);

    const viewWidth = endPos - startPos;
    const bpToX = (bp: number) => ((bp - startPos) / viewWidth) * width;

    const drawRuler = () => {
      ctx.fillStyle = colors.rulerBg;
      ctx.fillRect(0, 0, width, rulerHeight);
      ctx.strokeStyle = colors.rulerLine;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, rulerHeight);
      ctx.lineTo(width, rulerHeight);
      ctx.stroke();

      const tickIntervals = [1, 5, 10, 50, 100, 500, 1000, 5000, 10000, 50000, 100000, 500000, 1000000, 5000000, 10000000];
      let tickSpacing = tickIntervals.find((interval) => viewWidth / interval < 10) || 5000000;
      if (viewWidth / tickSpacing > 15) {
        const idx = tickIntervals.indexOf(tickSpacing);
        if (idx < tickIntervals.length - 1) tickSpacing = tickIntervals[idx + 1];
      }

      const firstTick = Math.ceil(startPos / tickSpacing) * tickSpacing;
      ctx.fillStyle = colors.tickText;
      ctx.font = "10px monospace";
      ctx.textAlign = "center";

      for (let tick = firstTick; tick <= endPos; tick += tickSpacing) {
        const x = bpToX(tick);
        ctx.strokeStyle = colors.tickLine;
        ctx.beginPath();
        ctx.moveTo(x, rulerHeight - 12);
        ctx.lineTo(x, rulerHeight);
        ctx.stroke();

        ctx.strokeStyle = colors.gridLine;
        ctx.beginPath();
        ctx.moveTo(x, rulerHeight);
        ctx.lineTo(x, height);
        ctx.stroke();

        let label = tick.toLocaleString();
        if (tickSpacing >= 1000000) label = `${(tick / 1000000).toFixed(2)} Mb`;
        else if (tickSpacing >= 1000) label = `${(tick / 1000).toFixed(1)} kb`;
        else label = `${tick} bp`;
        ctx.fillText(label, x, rulerHeight - 16);
      }
    };

    const drawTracks = () => {
      let currentTop = rulerHeight + trackPadding;

      const drawBox = (top: number, th: number, title: string) => {
        ctx.fillStyle = colors.trackBg;
        ctx.fillRect(0, top, width, th);
        ctx.strokeStyle = colors.trackBorder;
        ctx.strokeRect(0, top, width, th);
        ctx.fillStyle = colors.trackTitle;
        ctx.font = "600 10px sans-serif";
        ctx.textAlign = "left";
        ctx.fillText(title, 12, top + 16);
        ctx.strokeStyle = colors.gridLine;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(0, top + 22);
        ctx.lineTo(width, top + 22);
        ctx.stroke();
      };

      visibleTracks.forEach(track => {
        drawBox(currentTop, track.height, track.title);
        
        if (track.id === 'gc' && gcData.length > 0) {
          ctx.beginPath();
          let first = true;
          gcData.forEach((pt) => {
            // a missing bin breaks the line instead of drawing a value
            if (pt.avg_value == null) { first = true; return; }
            const x = bpToX(pt.bin_start);
            const y = currentTop + track.height - 6 - Math.max(0, Math.min(1, (pt.avg_value - 20) / 60)) * (track.height - 32);
            if (first) { ctx.moveTo(x, y); first = false; } else { ctx.lineTo(x, y); }
          });
          ctx.strokeStyle = isDark ? "#22d3ee" : "#0284c7";
          ctx.lineWidth = 1.5;
          ctx.stroke();
          ctx.fillStyle = colors.trackTitle;
          ctx.font = "9px monospace";
          ctx.textAlign = "right";
          ctx.fillText("GC %, display range 20 to 80 (clamped)", width - 8, currentTop + 14);
        } 
        else if (track.id === 'entropy' && entropyData.length > 0) {
          ctx.beginPath();
          let first = true;
          entropyData.forEach((pt) => {
            // NULL entropy (window with too few ACGT bases) breaks the line; never substitute a value
            if (pt.avg_shannon_entropy == null) { first = true; return; }
            const x = bpToX(pt.bin_start);
            const y = currentTop + track.height - 6 - Math.max(0, Math.min(1, (pt.avg_shannon_entropy - 0.8) / 1.2)) * (track.height - 32);
            if (first) { ctx.moveTo(x, y); first = false; } else { ctx.lineTo(x, y); }
          });
          ctx.strokeStyle = isDark ? "#fbbf24" : "#d97706";
          ctx.lineWidth = 1.5;
          ctx.stroke();
          ctx.fillStyle = colors.trackTitle;
          ctx.font = "9px monospace";
          ctx.textAlign = "right";
          ctx.fillText("bits (base composition), display range 0.8 to 2.0 (clamped)", width - 8, currentTop + 14);
        }
        else if ((track.id === 'density' && geneDensityData.length > 0) || (track.id === 'repeat-density' && repeatDensityData.length > 0)) {
          const plotTop = currentTop + 26;
          const plotHeight = track.height - 32;
          const drawDensity = (points: { bin_start: number; value: number }[], color: string, scale: number) => {
            if (points.length < 1) return;
            points.forEach((pt, i) => {
              const x = bpToX(pt.bin_start);
              const nextX = i + 1 < points.length ? bpToX(points[i + 1].bin_start) : width;
              const barWidth = Math.max(3, nextX - x - 1);
              const barHeight = Math.min(1, pt.value / scale) * plotHeight;
              ctx.fillStyle = color;
              ctx.fillRect(x, plotTop + plotHeight - barHeight, barWidth, barHeight);
            });
          };
          const densityScale = track.id === 'density' ? 15 : 500;
          if (track.id === 'density') {
            drawDensity(geneDensityData.map(pt => ({ bin_start: pt.bin_start, value: pt.avg_count })), isDark ? "rgba(52, 211, 153, 0.8)" : "rgba(5, 150, 105, 0.72)", densityScale);
          } else {
            drawDensity(repeatDensityData.map(pt => ({ bin_start: pt.bin_start, value: pt.avg_te_count })), isDark ? "rgba(251, 146, 60, 0.8)" : "rgba(234, 88, 12, 0.72)", densityScale);
          }
          ctx.fillStyle = colors.trackTitle;
          ctx.font = "9px monospace";
          ctx.textAlign = "right";
          ctx.fillText(`bar height: count per window, display max ${densityScale} (clamped)`, width - 8, currentTop + 14);
        }
        else if (track.id === 'gaps') {
          gapData.forEach((gap) => {
            const xStart = Math.max(0, bpToX(gap.start_pos));
            const xEnd = Math.min(width, bpToX(gap.end_pos));
            ctx.fillStyle = isDark ? "#fb7185" : "#e11d48";
            ctx.fillRect(xStart, currentTop + 30, Math.max(3, xEnd - xStart), track.height - 42);
          });
          homopolymerData.forEach((tract) => {
            const xStart = Math.max(0, bpToX(tract.start_pos));
            const xEnd = Math.min(width, bpToX(tract.end_pos));
            const baseColor: Record<string, string> = { A: "#38bdf8", C: "#a78bfa", G: "#fbbf24", T: "#4ade80" };
            ctx.fillStyle = baseColor[tract.base] || "#94a3b8";
            ctx.fillRect(xStart, currentTop + 30, Math.max(2, xEnd - xStart), track.height - 42);
          });
        }
        else if (track.id === 'models') {
          geneModelData.forEach((gm) => {
            const xStart = Math.max(0, bpToX(gm.start_pos));
            const xEnd = Math.min(width, bpToX(gm.end_pos));
            ctx.fillStyle = gm.biotype === "protein_coding" ? (isDark ? "#a78bfa" : "#7c3aed") : (isDark ? "#f472b6" : "#db2777");
            ctx.fillRect(xStart, currentTop + 30, Math.max(2, xEnd - xStart), 10);
          });
          repeatFamilyData.forEach((repeat) => {
            const xStart = Math.max(0, bpToX(repeat.start_pos));
            const xEnd = Math.min(width, bpToX(repeat.end_pos));
            ctx.fillStyle = isDark ? "rgba(251, 146, 60, 0.75)" : "rgba(234, 88, 12, 0.7)";
            ctx.fillRect(xStart, currentTop + 45, Math.max(2, xEnd - xStart), 8);
          });
        }
        else if (track.id === 'hecate' && hecateMprobData.length > 0) {
          const plotTop = currentTop + 26;
          const plotHeight = track.height - 32;
          const maxBits = 2.5; // display scale only; values above are clamped, typical range 0 to about 2 bits/base
          const yForBits = (bits: number) => plotTop + plotHeight - Math.max(0, Math.min(maxBits, bits)) / maxBits * plotHeight;

          // min/max band of the window values inside each bin (only bins with data)
          const valid = hecateMprobData.filter((pt) => pt.mean_entropy !== null && pt.min_entropy !== null && pt.max_entropy !== null);
          if (valid.length > 0) {
            ctx.beginPath();
            valid.forEach((pt, i) => {
              const x = bpToX(pt.bin_start);
              const y = yForBits(pt.max_entropy as number);
              if (i === 0) { ctx.moveTo(x, y); } else { ctx.lineTo(x, y); }
            });
            for (let i = valid.length - 1; i >= 0; i--) {
              ctx.lineTo(bpToX(valid[i].bin_start), yForBits(valid[i].min_entropy as number));
            }
            ctx.closePath();
            ctx.fillStyle = isDark ? "rgba(56, 189, 248, 0.14)" : "rgba(3, 105, 161, 0.12)";
            ctx.fill();
          }

          // mean line: a missing bin (null) breaks the line instead of drawing a zero
          ctx.beginPath();
          let drawing = false;
          hecateMprobData.forEach((pt) => {
            if (pt.mean_entropy === null) { drawing = false; return; }
            const x = bpToX(pt.bin_start);
            const y = yForBits(pt.mean_entropy);
            if (!drawing) { ctx.moveTo(x, y); drawing = true; } else { ctx.lineTo(x, y); }
          });
          ctx.strokeStyle = isDark ? "#38bdf8" : "#0369a1";
          ctx.lineWidth = 2;
          ctx.stroke();

          ctx.fillStyle = colors.trackTitle;
          ctx.font = "9px monospace";
          ctx.textAlign = "right";
          ctx.fillText(`${maxBits.toFixed(1)} bits/base`, width - 8, plotTop + 9);
          ctx.fillText("0", width - 8, plotTop + plotHeight);
        }

        // explicit empty / capped notices (an empty lane must not be read as a measured zero)
        const emptyMessages: Record<string, [boolean, string]> = {
          'gc': [gcData.length === 0, "No GC data in this window"],
          'entropy': [entropyData.length === 0, "No Shannon entropy data in this window"],
          'density': [geneDensityData.length === 0, "No gene density data in this window"],
          'repeat-density': [repeatDensityData.length === 0, "No repeat density data in this window"],
          'gaps': [gapData.length === 0 && homopolymerData.length === 0, "No assembly gaps or homopolymer runs in this window (T2T has no gaps)"],
          'models': [geneModelData.length === 0 && repeatFamilyData.length === 0, "No gene models or repeats in this window"],
          'hecate': [hecateMprobData.length === 0, "No Hecate cross-entropy data in this window"],
        };
        const [isEmpty, emptyText] = emptyMessages[track.id] ?? [false, ""];
        if (isEmpty) {
          ctx.fillStyle = colors.trackTitle;
          ctx.font = "10px sans-serif";
          ctx.textAlign = "center";
          ctx.fillText(emptyText, width / 2, currentTop + track.height / 2 + 8);
        }
        const capped = track.id === 'gaps' ? (truncatedTracks['homopolymer_genomic_tracts'] || truncatedTracks['assembly_gaps'])
          : track.id === 'models' ? (truncatedTracks['gene_models'] || truncatedTracks['repeatmasker_families']) : false;
        if (capped) {
          ctx.fillStyle = isDark ? "#fbbf24" : "#b45309";
          ctx.font = "9px sans-serif";
          ctx.textAlign = "right";
          ctx.fillText("API returned only the first 5,000 features here; zoom in to see all", width - 8, currentTop + 14);
        }
        
        currentTop += track.height + trackPadding;
      });
    };

    const drawHover = () => {
      if (hoverCoord !== null) {
        const hx = bpToX(hoverCoord);
        ctx.strokeStyle = colors.hoverLine;
        ctx.lineWidth = 1;
        ctx.setLineDash([3, 3]);
        ctx.beginPath();
        ctx.moveTo(hx, 0);
        ctx.lineTo(hx, height);
        ctx.stroke();
        ctx.setLineDash([]);
        
        const label = `${hoverCoord.toLocaleString()} bp`;
        ctx.font = "bold 10px monospace";
        const metrics = ctx.measureText(label);
        ctx.fillStyle = colors.hoverTextBg;
        ctx.fillRect(hx - metrics.width/2 - 4, 2, metrics.width + 8, 16);
        ctx.fillStyle = colors.hoverText;
        ctx.textAlign = "center";
        ctx.fillText(label, hx, 14);
      }
    };

    drawRuler();
    drawTracks();
    drawHover();
  }, [startPos, endPos, data, hoverCoord, visibleTracks, theme, colors, trackPadding, rulerHeight, isDark, canvasWidth, truncatedTracks, gcData, gapData, homopolymerData, entropyData, geneDensityData, repeatDensityData, geneModelData, repeatFamilyData, hecateMprobData]);

  const handleZoom = (factor: number, anchorBp?: number) => {
    const currentLen = endPos - startPos;
    const targetLen = Math.max(10, Math.min(currentMaxLen, Math.floor(currentLen * factor)));
    const anchor = anchorBp !== undefined ? anchorBp : startPos + currentLen / 2;
    
    const ratio = (anchor - startPos) / currentLen;
    const newStart = Math.max(0, Math.floor(anchor - targetLen * ratio));
    const newEnd = Math.min(currentMaxLen, newStart + targetLen);
    
    if (newEnd === currentMaxLen) {
      setStartPos(Math.max(0, currentMaxLen - targetLen));
      setEndPos(currentMaxLen);
    } else {
      setStartPos(newStart);
      setEndPos(newEnd);
    }
  };

  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    dragStartRef.current = { x, start: startPos, end: endPos };
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const viewWidth = endPos - startPos;
    const hoveredBp = Math.floor(startPos + (x / rect.width) * viewWidth);

    if (hoveredBp >= 0 && hoveredBp <= currentMaxLen) {
      setHoverCoord(hoveredBp);
      // value of the bin that contains the pointer; null when no bin covers it (never the nearest bin)
      const inBin = <T extends { bin_start: number; bin_end?: number }>(arr: T[]): T | null =>
        arr.find((d) => hoveredBp >= d.bin_start && hoveredBp < (d.bin_end ?? d.bin_start + 1)) ?? null;
      const point = inBin(gcData);
      const entropyPoint = inBin(entropyData);
      const geneDensityPoint = inBin(geneDensityData);
      const repeatDensityPoint = inBin(repeatDensityData);
      const hecatePoint = inBin(hecateMprobData ?? []);
      const covering = <T extends { start_pos: number; end_pos: number }>(arr: T[]) =>
        arr.filter((f) => hoveredBp >= f.start_pos && hoveredBp < f.end_pos);
      const genes = covering(geneModelData);
      const repeats = covering(repeatFamilyData);
      const gaps = covering(gapData);
      const runs = covering(homopolymerData);
      const list = (items: string[]) => items.slice(0, 3).join("; ") + (items.length > 3 ? ` (+${items.length - 3} more)` : "");
      setHoverData({
        gc: point?.avg_value ?? null,
        skew: point?.avg_skew ?? null,
        entropy: entropyPoint?.avg_shannon_entropy ?? null,
        geneDensity: geneDensityPoint?.avg_count ?? null,
        repeatDensity: repeatDensityPoint?.avg_te_count ?? null,
        hecate_entropy: hecatePoint?.mean_entropy ?? null,
        gene: genes.length ? list(genes.map((g) => `${g.gene_name || g.gene_id} (${g.biotype}, ${g.strand}) ${g.start_pos.toLocaleString()}-${g.end_pos.toLocaleString()}`)) : undefined,
        repeat: repeats.length ? list(repeats.map((r) => `${r.te_name} (${r.te_class}) ${r.start_pos.toLocaleString()}-${r.end_pos.toLocaleString()}`)) : undefined,
        gap: gaps.length ? list(gaps.map((g) => `${g.start_pos.toLocaleString()}-${g.end_pos.toLocaleString()}${g.gap_type ? ` ${g.gap_type}` : ""}`)) : undefined,
        homopolymer: runs.length ? list(runs.map((h) => `${h.base} x ${h.run_length} (${h.start_pos.toLocaleString()}-${h.end_pos.toLocaleString()})`)) : undefined,
      });
    }

    if (dragStartRef.current) {
      const dx = x - dragStartRef.current.x;
      const bpDelta = Math.floor((dx / rect.width) * (dragStartRef.current.end - dragStartRef.current.start));
      const prevLen = dragStartRef.current.end - dragStartRef.current.start;
      let newStart = dragStartRef.current.start - bpDelta;
      let newEnd = dragStartRef.current.end - bpDelta;

      if (newStart < 0) { newStart = 0; newEnd = prevLen; }
      else if (newEnd > currentMaxLen) { newEnd = currentMaxLen; newStart = Math.max(0, currentMaxLen - prevLen); }

      setStartPos(newStart);
      setEndPos(newEnd);
    }
  };

  const handleMouseUp = () => { dragStartRef.current = null; };
  const handleMouseLeave = () => { dragStartRef.current = null; setHoverCoord(null); setHoverData(null); };

  const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const viewWidth = endPos - startPos;
      const anchorBp = startPos + (x / rect.width) * viewWidth;
      const zoomFactor = e.deltaY > 0 ? 1.15 : 0.85;
      handleZoom(zoomFactor, anchorBp);
    }
  };

  return (
    <div ref={containerRef} className="relative w-full h-[760px] overflow-y-auto overflow-x-hidden custom-scrollbar bg-card" onWheel={handleWheel}>
      <canvas
        ref={canvasRef}
        width={canvasWidth}
        height={totalHeight}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseLeave}
        className="w-full block cursor-grab active:cursor-grabbing"
        style={{ width: "100%", height: totalHeight }}
      />
    </div>
  );
}
