import { 
  DNAStats, 
  ChromosomeListResponse, 
  TopGene, 
  TelomereHit, 
  CentromereRegion, 
  SSRSummary, 
  SSRDensityWindow,
  SSRHit,
  GenomeDinucleotides,
  AssemblyStats,
  BiotypeStat,
  GeneSummary,
  GeneChromosomeStat,
  IntergenicSpacing,
  SSRCoverage,
  GeneOverlap,
  GeneSearchResponse,
  NestedGeneSummary,
  Gene,
  CpGAssociation,
  RNACluster,
  RNASummary,
  HostGuestRelationship,
  SpliceSiteConsensus,
  SpliceSiteRecord,
  ProteinStats,
  AminoAcidComposition,
  TerminalComposition,
  ProteinBase,
  HomopolymerTract,
  IsochoreStats,
  CpGIslandResponse,
  AssemblyGapHit,
  KmerResponse,
  SystemHealth,
  SystemReady,
  TrackDataResponse
} from "@/types/api";

const BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000/api/v1";

export async function fetcher<T>(endpoint: string, params?: Record<string, string | number | boolean>): Promise<T> {
  const url = new URL(`${BASE_URL}${endpoint}`);
  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined) url.searchParams.append(key, String(value));
    });
  }
  
  console.debug(`[API Request] ${url.toString()}`);

  try {
    const response = await fetch(url.toString(), {
      headers: { "Content-Type": "application/json" },
    });

    if (!response.ok) {
      let errorDetail = "API request failed";
      try {
        const errorData = await response.json();
        errorDetail = errorData.message || errorData.detail || JSON.stringify(errorData);
      } catch {
        // Fallback if not JSON
        errorDetail = response.statusText;
      }
      
      console.error(`[API Error] ${response.status} ${endpoint}:`, errorDetail);
      throw new Error(errorDetail);
    }

    const data = await response.json();
    return data;
  } catch (error) {
    if (error instanceof Error) {
      console.error(`[API Fetch Exception] ${endpoint}:`, error.message);
      throw error;
    }
    console.error(`[API Unknown Error] ${endpoint}:`, error);
    throw new Error("An unexpected error occurred");
  }
}

export const dnaApi = {
  getSummary: (build: string) => fetcher<DNAStats>("/dna/stats/summary", { build }),
  getChromosomes: (build: string) => fetcher<ChromosomeListResponse>("/dna/stats/chromosomes", { build }),
  getDinucleotides: (build: string) => fetcher<GenomeDinucleotides>("/dna/stats/dinucleotides", { build }),
  getTelomeres: (build: string) => fetcher<TelomereHit[]>("/dna/telomeres", { build }),
  getCentromeres: (build: string) => fetcher<CentromereRegion[]>("/dna/centromeres", { build }),
  getSSRSummary: (build: string, chrom: string) => fetcher<SSRSummary[]>("/dna/ssr/summary", { build, chrom }),
  getSSRHits: (build: string, params: { chrom: string, limit?: number, offset?: number, min_repeats?: number }) => 
    fetcher<SSRHit[]>("/dna/ssr", { build, ...params }),
  getSSR: (build: string, chrom: string, params: { start?: number, end?: number, min_repeats?: number, period?: number, limit?: number, offset?: number } = {}) => 
    fetcher<SSRHit[]>("/dna/ssr", { build, chrom, ...params }),
  getSSRDensity: (build: string, chrom: string, window_size = 1000000) =>
    fetcher<SSRDensityWindow[]>("/dna/ssr/density", { build, chrom, window_size }),
  getSSRCoverage: (build: string) =>
    fetcher<SSRCoverage[]>("/dna/ssr/coverage", { build }),
  getAssembly: (build: string) => fetcher<AssemblyStats>("/dna/stats/assembly", { build }),
  getSequence: (build: string, chrom: string, start: number, end: number) => 
    fetcher<string>("/dna/sequence", { build, chrom, start, end }),
  getSequenceWindows: (build: string, chrom: string, params: { start?: number, end?: number, step?: number, limit?: number, offset?: number } = {}) => 
    fetcher<{ start: number; end: number; gc_content: number; gc_skew: number | null }[]>("/dna/sequence/window", { build, chrom, ...params }),
  getGCSkew: (build: string, chrom: string, window_size = 10000) => 
    fetcher<{ start: number; gc_skew: number }[]> (`/dna/stats/${chrom}/gc-skew`, { build, window_size }),
  getIsochores: (build: string, chrom?: string) => fetcher<IsochoreStats[]>("/dna/isochores", { build, ...(chrom ? { chrom } : {}) }),
  getCPGIslands: (build: string, opts: { chrom?: string; strict?: boolean; limit?: number } = {}) =>
    fetcher<CpGIslandResponse>("/dna/cpg-islands", { build, ...(opts.chrom ? { chrom: opts.chrom } : {}), ...(opts.strict ? { strict: true } : {}), ...(opts.limit ? { limit: opts.limit } : {}) }),
  getGaps: (build: string, chrom?: string) => fetcher<AssemblyGapHit[]>("/dna/gaps", { build, ...(chrom ? { chrom } : {}) }),
  getKmers: (build: string, chrom: string, params: { k?: number, start?: number, end?: number, top_n?: number, normalize?: boolean } = {}) => 
    fetcher<KmerResponse>(`/dna/stats/${chrom}/kmers`, { build, ...params }),
};

export const genesApi = {
  getSummary: (build: string) => fetcher<GeneSummary>("/genes/stats/summary", { build }),
  getBiotypes: (build: string) => fetcher<BiotypeStat[]>("/genes/stats/biotypes", { build }),
  getLargest: (build: string) => fetcher<TopGene[]>("/genes/top/largest", { build }),
  getMostExons: (build: string) => fetcher<TopGene[]>("/genes/top/most-exons", { build }),
  search: (build: string, q: string) => fetcher<GeneSearchResponse[]>("/genes/search", { build, q }),
  getChromosomeStats: (build: string) => fetcher<GeneChromosomeStat[]>("/genes/stats/chromosomes", { build }),
  getNested: (build: string) => fetcher<NestedGeneSummary>("/genes/nested/summary", { build }),
  getRegion: (build: string, chrom: string, start: number, end: number) => 
    fetcher<Gene[]>("/genes/region", { build, chrom, start, end }),
  getDensity: (build: string, chrom: string, bin_size = 1000000) => 
    fetcher<{ start: number; count: number }[]>("/genes/density", { build, chrom, bin_size }),
  getSizeStats: (build: string) => fetcher<Record<string, Record<string, number>>>("/genes/stats/sizes", { build }),
  getSpacing: (build: string, chrom?: string) => fetcher<IntergenicSpacing[]>("/genes/spacing", { build, ...(chrom ? { chrom } : {}) }),
  getOverlaps: (build: string, chrom?: string) => fetcher<GeneOverlap[]>("/genes/overlaps", { build, ...(chrom ? { chrom } : {}) }),
  getCpgAssociation: (build: string, promoter_distance = 2000) => fetcher<CpGAssociation>("/genes/cpg-association", { build, promoter_distance }),
  getGene: (build: string, id: string) => fetcher<Gene>(`/genes/${id}`, { build }),
};

export const rnaApi = {
  getSummary: (build: string) => fetcher<RNASummary>("/rna/stats/summary", { build }),
  getClusters: (build: string, params: { rna_type?: string, chrom?: string } = {}) => 
    fetcher<RNACluster[]>("/rna/clusters", { build, ...params }),
  getRelationships: (build: string, params: { host_gene_id?: string, guest_gene_id?: string } = {}) => 
    fetcher<HostGuestRelationship[]>("/rna/relationships", { build, ...params }),
  getSpliceConsensus: (build: string, site_type?: string) => 
    fetcher<SpliceSiteConsensus[]>("/rna/splice-sites", { build, ...(site_type ? { site_type } : {}) }),
  getSpliceRecords: (build: string, params: { site_type?: string; chrom?: string; limit?: number } = {}) =>
    fetcher<SpliceSiteRecord[]>("/rna/splice-sites/records", { build, ...params }),
};

export const proteinsApi = {
  getSizeStats: (build: string) => fetcher<ProteinStats>("/proteins/stats/size", { build }),
  // The backend takes no mitochondrial / terminal / k parameters (they were silently ignored).
  getComposition: (build: string) => 
    fetcher<AminoAcidComposition[]>("/proteins/stats/composition", { build }),
  getTerminalComposition: (build: string) => 
    fetcher<TerminalComposition[]>("/proteins/stats/terminal", { build }),
  getLargest: (build: string, limit = 10) => 
    fetcher<ProteinBase[]>("/proteins/top/largest", { build, limit }),
  getHomopolymerTracts: (build: string, params: { min_length?: number, limit?: number } = {}) => 
    fetcher<HomopolymerTract[]>("/proteins/tracts/homopolymers", { build, ...params }),
  getRich: (build: string, params: { aa: string, threshold?: number, limit?: number }) => 
    fetcher<ProteinBase[]>("/proteins/rich", { build, ...params }),
};

export const systemApi = {
  getGenomes: () => fetcher<string[]>("/system/genomes"),
  getHealth: () => fetcher<SystemHealth>("/system/health"),
  getReady: () => fetcher<SystemReady>("/system/ready"),
};

export const tracksApi = {
  getSummary: <T = Record<string, unknown>>(genome: string, trackName: string) =>
    fetcher<TrackDataResponse<T>>(`/summary/${genome}/${trackName}`),
};
