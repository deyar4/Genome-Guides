/**
 * Standardized genomic colors for consistent visualization.
 * These are used across Nivo charts, UI components, and icons.
 */
export const NUCLEOTIDE_COLORS = {
  A: '#3b82f6', // blue-500
  C: '#10b981', // emerald-500
  G: '#f59e0b', // amber-500
  T: '#ef4444', // red-500
  U: '#8b5cf6', // violet-500 (for RNA)
  N: '#94a3b8', // slate-400 (unknown/gap)
} as const;

export const GENOMIC_COLORS = {
  GC: '#06b6d4', // cyan-500
  AT: '#f97316', // orange-500
  EXON: '#4f46e5', // indigo-600
  INTRON: '#94a3b8', // slate-400
  INTERGENIC: '#e2e8f0', // slate-200
  REPEAT: '#d946ef', // fuchsia-500
  PRIMARY: '#0f172a', // slate-900 (light) or zinc-100 (dark)
} as const;

export const CHROMOSOME_TYPES = {
  AUTOSOME: '#64748b',
  SEX_CHROM: '#ec4899',
  MITOCHONDRIAL: '#f43f5e',
} as const;

export const CHART_PALETTE = [
  '#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', 
  '#06b6d4', '#f97316', '#d946ef', '#6366f1', '#14b8a6'
];

/**
 * A telomere record counts as an end cap when it starts within this many bp of position 0 (5' end)
 * or ends within this many bp of the chromosome length (3' end). 0 = exact match.
 */
export const TELOMERE_END_TOLERANCE_BP = 0;
