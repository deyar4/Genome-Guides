export interface AssemblyStats {
  total_length: number;
  contig_count: number;
  n50: number;
  l50: number;
  n90?: number;
  l90?: number;
  min_len?: number;
  max_len?: number;
  mean_length?: number;
  median_length?: number;
  gc_content: number | null;
}

export interface GenomeSection {
  a: number;
  c: number;
  g: number;
  t: number;
  n: number;
  gc_content: number;
}

export interface DNAStats {
  assembly: AssemblyStats;
  nuclear_composition: GenomeSection | null;
  mitochondrial_composition: GenomeSection | null;
  gene_count?: number;
  protein_coding_count?: number;
}

export interface Chromosome {
  name: string;
  length: number;
  gc_content: number; // backend may send null only for an all-N contig (none in T2T)
  gc_skew: number | null;
  is_mitochondrial: boolean;
  ssr_count: number | null; // null = no SSR track for this build
}

export interface ChromosomeListResponse {
  count: number;
  chromosomes: Chromosome[];
}

export interface DinucleotideStats {
  pair: string;
  observed_count: number;
  observed_freq: number;
  expected_freq: number | null;
  oe_ratio: number;
}

export interface GenomeDinucleotides {
  total_dinucleotides: number;
  base_frequencies: Record<string, number>;
  dinucleotides: DinucleotideStats[];
}

export interface SSRSummary {
  period: number;
  name: string;
  count: number;
  total_length: number;
  avg_repeat_count: number;
}

export interface SSRDensityWindow {
  chrom: string;
  start: number;
  end: number;
  count: number;
}

export interface SSRHit {
  chrom: string;
  start: number;
  end: number;
  motif: string;
  period: number;
  repeat_count: number;
  sequence: string;
  length: number;
  period_name: string;
  complexity: number;
}

export interface TelomereHit {
  chrom: string;
  start: number; // 0-based, half-open [start, end)
  end: number;
  length: number;
  source: string;
}

export interface CentromereRegion {
  chrom: string;
  start: number;
  end: number;
  length: number;
  gc_content: number | null;
  sequence_sample: string | null;
}

export interface IsochoreStats {
  chrom: string;
  start: number;
  end: number;
  gc_content: number;
  isochore_class: string;
}

export interface CpGIslandHit {
  chrom: string;
  start: number;
  end: number;
  length: number;
  gc_content: number;
  obs_exp_ratio: number;
  cpg_count: number;
  n_windows: number;
  strict: boolean;
}

export interface CpGIslandResponse {
  total: number; // all matching islands, before limit/offset
  returned: number;
  truncated: boolean;
  islands: CpGIslandHit[];
}

export interface AssemblyGapHit {
  chrom: string;
  start: number;
  end: number;
  length: number;
  gap_type: string;
}

export interface Exon {
  exon_number: string;
  start: number;
  end: number;
  strand: string;
  transcript_id: string;
}

export interface Transcript {
  transcript_id: string;
  gene_id: string;
  biotype: string | null;
  start: number;
  end: number;
  strand: string;
  exons: Exon[];
}

export interface Gene {
  gene_id: string;
  gene_name: string | null;
  biotype: string | null;
  chrom: string;
  start: number;
  end: number;
  strand: string;
  /** Same gene id on other contigs (PAR genes); empty for single-copy genes. */
  other_copies?: { gene_id: string; chrom: string; start: number; end: number }[];
  transcripts: Transcript[];
}

export interface BiotypeStat {
  biotype: string;
  count: number;
}

export interface GeneSummary {
  total_genes: number;
  biotype_counts: BiotypeStat[];
  avg_exons_per_transcript: number;
}

export interface TopGene {
  gene_id: string;
  gene_name: string | null;
  biotype: string | null;
  chrom: string;
  value: number;
  secondary_value: number | null;
  rank: number;
  start: number | null;
  end: number | null;
  strand: string | null;
}

export interface GeneSearchResponse {
  gene_id: string;
  gene_name: string | null;
  biotype: string | null;
  chrom: string;
  start?: number;
  end?: number;
}

export interface GeneChromosomeStat {
  chrom: string;
  gene_count: number;
  density: number;
}

export interface IntergenicSpacing {
  chrom: string;
  gene_id_1: string;
  gene_id_2: string;
  start: number;
  end: number;
  distance: number;
}

export interface GeneOverlap {
  chrom: string;
  gene_id_1: string;
  gene_id_2: string;
  overlap_type: 'same_strand' | 'opposite_strand';
  overlap_len: number;
  contained: boolean;
}

export interface NestedGenePair {
  chrom: string;
  host_gene_id: string;
  host_locus: number;
  host_gene_name: string | null;
  host_coding_class: string; // protein_coding = some transcript has annotated CDS, else non_coding
  host_start: number;
  host_end: number;
  guest_gene_id: string;
  guest_locus: number;
  guest_gene_name: string | null;
  guest_coding_class: string;
  guest_start: number;
  guest_end: number;
  relationship_type: string; // intronic | exonic_overlap | mixed | span_only
  strand_relation: string; // same | opposite | ambiguous
  is_direct: boolean;
}

export interface NestedGeneSummary {
  total_pairs: number;
  direct_pairs: number;
  same_strand: number;
  opposite_strand: number;
  ambiguous_strand: number;
  by_type: Record<string, number>;
  distinct_hosts: number;
  distinct_guests: number;
  first_pairs: NestedGenePair[]; // first pairs in coordinate order, not a random sample
}

export interface CpGAssociation {
  with_cpg: number;
  without_cpg: number;
  total: number;
  percentage: number;
  by_biotype: Record<string, { with_cpg: number, without_cpg: number, percentage: number }>;
  note: string | null;
}

// --- RNA Module ---

export interface RNACluster {
  cluster_id: string;
  rna_type: string;
  chrom: string;
  start: number;
  end: number;
  size: number;
  genes: string[];
}

export interface RNAClusterResponse {
  total: number;
  returned: number;
  truncated: boolean;
  clusters: RNACluster[];
}

export interface CappedList<T> {
  total: number;
  returned: number;
  truncated: boolean;
  items: T[];
}

export interface RNAClusterSummary {
  rna_type: string;
  num_clusters: number;
  avg_cluster_size: number;
  max_cluster_size: number;
  largest_cluster_location: string;
}

export interface RNASummary {
  overall_stats: {
    biotype_counts: Record<string, number>;
    cluster_counts: Record<string, number>;
    total_nc_rna: number;
    total_clusters: number;
    census_unit: string;
  };
  cluster_summaries: RNAClusterSummary[];
}

export interface HostGuestRelationship {
  contig: string;
  host_gene_id: string;
  host_locus: number;
  host_gene_name: string | null;
  guest_gene_id: string;
  guest_locus: number;
  guest_gene_name: string | null;
  guest_start: number;
  guest_end: number;
  guest_coding_class: string; // non_coding = no annotated CDS in any transcript
  relationship_type: string;
  strand_relation: string;
  is_same_strand: boolean | null; // null when ambiguous
}

export interface HostGuestResponse {
  total: number;
  returned: number;
  truncated: boolean;
  relationships: HostGuestRelationship[];
}

export interface SpliceSiteConsensus {
  site_type: string;
  consensus_seq: string;
  pwm: Record<string, number[]>;
  total_sites: number;
}

export interface RNAEditingSite {
  gene_id: string;
  chrom: string;
  position: number;
  ref: string;
  alt: string;
  frequency: number | null;
  site_type: string;
}

export interface SpliceSiteRecord {
  chrom: string;
  start: number;
  end: number;
  transcript_id: string;
  gene_id: string;
  strand: string;
  junction_number: number;
  donor_seq: string | null;
  acceptor_seq: string | null;
  is_canonical: boolean;
}

// --- Protein Module ---

export interface ProteinStats {
  total_proteins: number;
  avg_length: number;
  median_length: number;
  max_length: number;
  min_length: number;
}

export interface AminoAcidComposition {
  amino_acid: string;
  count: number;
  percentage: number;
}

export interface TerminalComposition {
  amino_acid: string;
  n_terminal_count: number;
  n_terminal_pct: number;
  c_terminal_count: number;
  c_terminal_pct: number;
  n_to_c_bias_ratio: number | null;
}

export interface ProteinBase {
  protein_id: string;
  gene_id: string | null; // null: protein -> gene mapping not available (OPEN)
  gene_name: string | null;
  transcript_id: string | null;
  sequence: string | null;
  length: number;
  is_mitochondrial: boolean | null;
  // only on /proteins/rich
  distinct_amino_acids?: number;
  dominant_amino_acid?: string | null;
  dominant_frequency_pct?: number;
  shannon_entropy?: number;
}

export interface HomopolymerTract {
  amino_acid: string;
  start: number;
  end: number;
  length: number;
  protein_id?: string; // Added for context in some views
}

export interface ProteinRichness {
  protein_id: string;
  amino_acid: string;
  percentage: number;
  z_score: number;
}

// --- System Module ---

export interface SystemJob {
  job_id: number;
  type: string;
  status: string;
  progress: number;
  error: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface BuildResponse {
  message: string;
  job_id: number;
  status: string;
}

export interface TrackMetadataEntry {
  window_bp: number | null; // null = interval track (not binned)
  columns: Record<string, string>;
  source?: string;
}

/** /tracks/metadata: per-track definitions plus the shared binning notes. */
export type TrackMetadata = Record<string, TrackMetadataEntry> & { binning?: string[] };

export interface SystemHealth {
  status: string;
  version: string;
}

export interface SystemReady {
  status: string;
  components: Record<string, string>;
}

export interface TrackDataResponse<T = Record<string, unknown>> {
  genome: string;
  contig: string;
  track_name: string;
  data: T[];
}

export interface KmerFrequency {
  kmer: string;
  count: number;
  /** percent (0-100) of all k-mers, not a fraction */
  frequency: number;
}

export interface KmerResponse {
  k: number;
  total_kmers: number;
  unique_kmers: number;
  kmers: KmerFrequency[];
}

/** Descriptive SSR coverage per period (D41). No null model; not an enrichment. */
export interface SSRCoverage {
  period: number;
  name: string;
  count: number;
  observed_bases: number;
  genome_bases: number;
  acgt_bases: number;
  n_bases: number;
  other_bases: number;
  coverage_fraction: number | null;
  density_per_mb: number | null;
  sum_ssr_bases: number;
  union_ssr_bases: number;
  overlap_excess_bases: number;
}
