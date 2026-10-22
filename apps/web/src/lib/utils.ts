import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** Gene page link; chrom selects one copy of a gene that exists on several contigs (PAR genes). */
export function geneHref(geneId: string, chrom?: string | null): string {
  const base = `/library/genes/${encodeURIComponent(geneId)}`;
  return chrom ? `${base}?chrom=${encodeURIComponent(chrom)}` : base;
}

export function formatNumber(num: number): string {
  return new Intl.NumberFormat('en-US').format(num);
}

export function formatGb(bases: number): string {
  return new Intl.NumberFormat('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(bases / 1e9);
}

export function formatMb(bases: number): string {
  return new Intl.NumberFormat('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(bases / 1e6);
}

export function formatKb(bases: number): string {
  return new Intl.NumberFormat('en-US', { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(bases / 1e3);
}

export function formatBp(bases: number): string {
  return new Intl.NumberFormat('en-US').format(bases);
}


/** The API only bins in whole multiples of the 1 kb base window (D31, D36): round a wanted bin size up to one. */
export function alignToBaseWindow(bp: number, baseWindow = 1000): number {
  return Math.max(baseWindow, Math.ceil(bp / baseWindow) * baseWindow);
}
