"use client"

import React from 'react';
import { cn } from '@/lib/utils';
import { 
  Dna, 
  Library, 
  Search, 
  Target, 
  Shapes,
  Activity,
  Database,
  Info,
  BookOpen,
  Map,
  X
} from 'lucide-react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useGenomeStore } from '@/store/use-genome-store';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface NavSection {
  title: string;
  items: {
    href: string;
    label: string;
    icon: React.ElementType;
  }[];
}

const navigation: NavSection[] = [
  {
    title: "Reference Overview",
    items: [
      { href: '/library/overview', label: 'Genome Summary', icon: BookOpen },
    ]
  },
  {
    title: "DNA: Physical Genome",
    items: [
      { href: '/library/dna/browser', label: 'Genome Browser', icon: Map },
      { href: '/library/dna/metrics', label: 'Chromosome Metrics', icon: Shapes },
      { href: '/library/dna/composition', label: 'Base Composition', icon: Activity },
      { href: '/library/dna/ssr', label: 'SSR Analysis', icon: Database },
      { href: '/library/dna/kmers', label: 'K-Mer Explorer', icon: Target },
      { href: '/library/dna/structure', label: 'Structural Elements', icon: Dna },
      { href: '/library/dna/inspector', label: 'Region Inspector', icon: Search },
    ]
  },
  {
    title: "Genes: Functional Units",
    items: [
      { href: '/library/genes/census', label: 'Biotype Census', icon: Library },
      { href: '/library/genes/search', label: 'Gene Reference', icon: Search },
      { href: '/library/genes/distribution', label: 'Genomic Distribution', icon: Target },
      { href: '/library/genes/correlations', label: 'Genomic Correlations', icon: Activity },
      { href: '/library/genes/spacing', label: 'Spacing & Overlaps', icon: Shapes },
    ]
  },
  {
    title: "RNA: Transcriptomics",
    items: [
      { href: '/library/rna', label: 'RNA Overview', icon: BookOpen },
      { href: '/library/rna/clusters', label: 'RNA Clusters', icon: Shapes },
      { href: '/library/rna/relationships', label: 'Nested Genes', icon: Target },
      { href: '/library/rna/splice-sites', label: 'Splice Junctions', icon: Activity },
    ]
  },
  {
    title: "Proteins: Proteomic Census",
    items: [
      { href: '/library/proteins', label: 'Protein Overview', icon: BookOpen },
      { href: '/library/proteins/composition', label: 'AA Composition', icon: Activity },
      { href: '/library/proteins/features', label: 'Protein Features', icon: Target },
      { href: '/library/proteins/rich', label: 'Rich Proteins', icon: Database },
    ]
  },
  {
    title: "Research Tools",
    items: [
      { href: '/library/tools/sequence', label: 'Sequence Tools', icon: Database },
    ]
  }
];

export function SidebarNav({ className, mobileOpen = false, onClose }: { className?: string; mobileOpen?: boolean; onClose?: () => void }) {
  const pathname = usePathname();
  const { activeGenome, setActiveGenome, availableGenomes } = useGenomeStore();

  return (
    <nav className={cn(
      "flex flex-col h-full bg-card/95 border-r border-border w-72 backdrop-blur-xl",
      "fixed inset-y-0 left-0 z-40 transition-transform duration-300 lg:static lg:translate-x-0",
      mobileOpen ? "translate-x-0" : "-translate-x-full",
      className
    )}>
      <div className="p-6 border-b border-border">
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-xl font-bold tracking-tight text-foreground flex items-center gap-2">
            <Map className="h-6 w-6 text-primary" />
            Genome Guides
          </h1>
          <button className="lg:hidden rounded-lg p-2 text-muted-foreground hover:bg-accent" onClick={onClose} aria-label="Close navigation">
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-1.5">
          <label className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest px-1">
            Current Assembly
          </label>
          <Select value={activeGenome} onValueChange={setActiveGenome}>
            <SelectTrigger className="w-full h-9 bg-background border-border">
              <SelectValue placeholder="Select Assembly" />
            </SelectTrigger>
            <SelectContent>
              {availableGenomes.map((g) => (
                <SelectItem key={g} value={g}>
                  {g === 'hs1' ? 'T2T-CHM13' : g === 'hg38' ? 'GRCh38' : g}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      
      <div className="flex-1 overflow-y-auto py-6 px-3 space-y-8 custom-scrollbar">
        {navigation.map((section) => (
          <div key={section.title} className="space-y-1">
            <h2 className="px-3 text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
              {section.title}
            </h2>
            {section.items.map((item) => {
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  onClick={onClose}
                  className={cn(
                    "group flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all duration-200",
                    isActive 
                      ? "bg-accent text-accent-foreground shadow-sm ring-1 ring-primary/20" 
                      : "text-muted-foreground hover:text-foreground hover:bg-accent/60"
                  )}
                >
                  <item.icon className={cn(
                    "h-4 w-4 transition-colors",
                    isActive ? "text-primary" : "text-muted-foreground group-hover:text-primary"
                  )} />
                  <span className="flex-1">{item.label}</span>
                  {isActive && <div className="h-1.5 w-1.5 rounded-full bg-primary" />}
                </Link>
              );
            })}
          </div>
        ))}
      </div>

      <div className="p-4 border-t border-border bg-background/50">
        <div className="flex items-center gap-3">
          <div className="p-2 bg-card rounded-lg border border-border shadow-sm">
            <Info className="h-4 w-4 text-muted-foreground" />
          </div>
          <div className="flex-1 min-w-0">
            <div className="text-[10px] font-bold text-muted-foreground uppercase mb-0.5 tracking-tight">Active Reference</div>
            <div className="text-xs font-mono font-bold text-foreground truncate">
              {activeGenome === 'hs1' ? 'T2T-CHM13' : activeGenome === 'hg38' ? 'GRCh38.p14' : activeGenome.toUpperCase()}
            </div>
          </div>
        </div>
      </div>
    </nav>
  );
}
