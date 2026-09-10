export interface TrackDataPoint {
  bin_start: number;
  avg_value: number;
  min_value: number;
  max_value: number;
  avg_skew?: number;
}

export interface GapFeature {
  contig: string;
  start_pos: number;
  end_pos: number;
  gap_length: number;
  gap_type: string;
}

export interface HomopolymerFeature {
  contig: string;
  start_pos: number;
  end_pos: number;
  base: string;
  run_length: number;
}

export interface ShannonEntropyPoint {
  bin_start: number;
  avg_shannon_entropy: number;
  min_shannon_entropy: number;
  max_shannon_entropy: number;
  avg_telomeric_repeats?: number;
}

export interface GeneDensityPoint {
  bin_start: number;
  avg_count: number;
}

export interface RepeatDensityPoint {
  bin_start: number;
  avg_te_count: number;
}

export interface GeneModelFeature {
  contig: string;
  start_pos: number;
  end_pos: number;
  gene_id: string;
  gene_name: string;
  biotype: string;
  strand: string;
}

export interface RepeatFamilyFeature {
  contig: string;
  start_pos: number;
  end_pos: number;
  strand: string;
  te_name: string;
  te_class: string;
  divergence_pct: number;
}

export interface TrackCache {
  genome: string;
  contig: string;
  start: number;
  end: number;
  gc: TrackDataPoint[];
  gaps: GapFeature[];
  homopolymers: HomopolymerFeature[];
  entropy: ShannonEntropyPoint[];
  geneDensity: GeneDensityPoint[];
  repeatDensity: RepeatDensityPoint[];
  geneModels: GeneModelFeature[];
  repeatFamilies: RepeatFamilyFeature[];
  hecate_mprob: HecateMprobPoint[];
}

export interface HecateMprobPoint {
  bin_start: number;
  mean_prob: number;
  min_prob: number;
  max_prob: number;
}
