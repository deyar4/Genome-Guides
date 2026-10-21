"use client"

import React from 'react';
import { useQuery } from '@tanstack/react-query';
import { useGenomeStore } from '@/store/use-genome-store';
import { genesApi } from '@/lib/api';
import { LibraryShell } from '@/components/layout/library-shell';
import { useParams } from 'next/navigation';
import { Card, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { formatNumber } from '@/lib/utils';
import { Info, Activity, Layers, ChevronRight } from 'lucide-react';
import { Transcript, Exon } from '@/types/api';
import { GenomicMetricCard } from '@/components/library/dna/genomic-metric-card';
import { SectionHeader } from '@/components/library/dna/shared';
import { NeighborhoodBrowser } from '@/components/library/genes/neighborhood-browser';
import { GeneArchitectureTrack } from '@/components/library/genes/gene-architecture-track';
import { SequenceExporter } from '@/components/library/genes/sequence-exporter';

export default function GeneFactSheetPage() {
  const params = useParams();
  const id = params.id as string;
  const { activeGenome } = useGenomeStore();
  const [showAllTranscripts, setShowAllTranscripts] = React.useState(false);

  const { data: gene, isLoading } = useQuery({
    queryKey: ['gene', activeGenome, id],
    queryFn: () => genesApi.getGene(activeGenome, id),
  });

  if (isLoading) {
    return (
      <LibraryShell title="Loading Gene Details...">
        <div className="space-y-8 animate-pulse">
          <div className="grid grid-cols-3 gap-6">
             {[...Array(3)].map((_, i) => <div key={i} className="h-32 bg-slate-50 dark:bg-zinc-900 rounded-2xl" />)}
          </div>
          <div className="h-96 bg-slate-50 dark:bg-zinc-900 rounded-2xl" />
        </div>
      </LibraryShell>
    );
  }

  if (!gene) {
    return (
      <LibraryShell title="Gene Not Found">
        <div className="p-20 text-center border-2 border-dashed border-slate-200 dark:border-zinc-800 rounded-3xl">
          <div className="h-16 w-16 bg-slate-100 dark:bg-zinc-900 rounded-2xl flex items-center justify-center mx-auto mb-6">
             <Info className="h-8 w-8 text-slate-300" />
          </div>
          <h3 className="text-xl font-bold text-slate-900 dark:text-zinc-100 mb-2">Record Not Found</h3>
          <p className="text-slate-500 max-w-sm mx-auto">The gene identifier &quot;{id}&quot; was not found in the current assembly ({activeGenome}).</p>
        </div>
      </LibraryShell>
    );
  }

  return (
    <LibraryShell 
      title={gene.gene_name || gene.gene_id} 
      subtitle={`Genomic fact-sheet for ${gene.gene_id} in assembly ${activeGenome}.`}
    >
      <div className="space-y-16 pb-20">
        
        {/* HEADER STATS */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
          <GenomicMetricCard 
            label="Genomic Locus"
            value={`${gene.chrom}:${formatNumber(gene.start)}`}
            description={`Resides on the ${gene.strand === '+' ? 'Forward' : 'Reverse'} strand.`}
            color="#3b82f6"
          />
          <GenomicMetricCard 
            label="Gene Biotype"
            value={gene.biotype?.replace('_', ' ') || 'N/A'}
            description="Biological classification of this feature."
            color="#10b981"
          />
          <GenomicMetricCard 
            label="Locus Span"
            value={`${formatNumber(gene.end - gene.start)} bp`}
            description={`Total genomic footprint (~${((gene.end - gene.start) / 1000).toFixed(1)} kb).`}
            color="#8b5cf6"
          />
        </div>

        {/* NEIGHBORHOOD */}
        <section>
          <NeighborhoodBrowser 
            chrom={gene.chrom} 
            start={gene.start} 
            end={gene.end} 
            centerGeneId={gene.gene_id} 
          />
        </section>

        {/* VISUAL ARCHITECTURE */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Architecture Visualizer" 
              subtitle="Interactive track showing exon/intron structure for all annotated transcripts."
            />
          </div>
          <Card className="p-8 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm">
             <GeneArchitectureTrack 
               geneStart={gene.start} 
               geneEnd={gene.end} 
               transcripts={showAllTranscripts ? gene.transcripts : gene.transcripts.slice(0, 20)} 
               strand={gene.strand} 
             />
          </Card>
        </section>

        {/* SEQUENCE EXPORTER */}
        <section>
           <SequenceExporter 
             chrom={gene.chrom} 
             start={gene.start} 
             end={gene.end} 
             geneName={gene.gene_name || gene.gene_id} 
           />
        </section>

        {/* TRANSCRIPTS SECTION */}
        <section>
          <div className="flex items-center justify-between mb-8">
            <SectionHeader 
              title="Structural Architecture" 
              subtitle="Detailed breakdown of transcript models and exonic coordinates."
            />
            <div className="px-3 py-1 bg-slate-50 dark:bg-zinc-900 rounded-full border border-slate-200 dark:border-zinc-800 text-[10px] font-bold text-slate-400 uppercase tracking-widest flex items-center gap-2">
               <Layers className="h-3 w-3" />
               Annotation Track
            </div>
          </div>

          <div className="space-y-12">
            {(showAllTranscripts ? gene.transcripts : gene.transcripts.slice(0, 20)).map((transcript: Transcript) => (
              <Card key={transcript.transcript_id} className="overflow-hidden border-slate-200 dark:border-zinc-800 shadow-sm group">
                <div className="p-6 bg-slate-50 dark:bg-zinc-900/50 border-b border-slate-100 dark:border-zinc-800 flex items-center justify-between">
                  <div className="flex items-center gap-4">
                    <div className="h-10 w-10 rounded-xl bg-white dark:bg-zinc-800 flex items-center justify-center border border-slate-200 dark:border-zinc-700">
                       <ChevronRight className="h-5 w-5 text-slate-400 group-hover:text-primary transition-colors" />
                    </div>
                    <div>
                      <div className="text-lg font-mono font-bold text-slate-900 dark:text-zinc-100">{transcript.transcript_id}</div>
                      <div className="flex items-center gap-2 mt-0.5">
                         <Badge variant="outline" className="text-[9px] font-bold uppercase tracking-tighter px-1.5 py-0 bg-white dark:bg-zinc-950">
                           {transcript.biotype?.replace('_', ' ')}
                         </Badge>
                         <span className="text-[10px] font-bold text-slate-400 uppercase tracking-widest">{transcript.exons.length} Exons</span>
                      </div>
                    </div>
                  </div>
                </div>
                
                <CardContent className="p-0">
                  <Table>
                    <TableHeader className="bg-white dark:bg-zinc-950 border-b">
                      <TableRow>
                        <TableHead className="w-24 pl-6">Exon</TableHead>
                        <TableHead>Genomic Coordinates (0-based, [start, end))</TableHead>
                        <TableHead className="text-right pr-6">Length</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {transcript.exons.map((exon: Exon) => (
                        <TableRow key={exon.exon_number} className="hover:bg-slate-50/50 dark:hover:bg-zinc-900/50 transition-colors">
                          <TableCell className="font-bold text-slate-400 pl-6 text-xs">#{exon.exon_number}</TableCell>
                          <TableCell className="font-mono text-xs font-bold text-slate-600 dark:text-zinc-400">
                            {formatNumber(exon.start)} <span className="mx-2 opacity-30">—</span> {formatNumber(exon.end)}
                          </TableCell>
                          <TableCell className="text-right font-mono text-xs font-bold text-primary pr-6">
                            {formatNumber(exon.end - exon.start)} <span className="text-[9px] opacity-50 uppercase ml-1">bp</span>
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </CardContent>
              </Card>
            ))}
          </div>
          {gene.transcripts.length > 20 && (
            <button
              className="mt-6 text-xs font-bold text-primary underline"
              onClick={() => setShowAllTranscripts(v => !v)}
            >
              {showAllTranscripts ? 'Show first 20 transcripts only' : `Show all ${gene.transcripts.length} transcripts`}
            </button>
          )}
        </section>

        {/* ACADEMIC FOOTER */}
        <div className="p-10 bg-primary text-primary-foreground rounded-[2.5rem] relative overflow-hidden shadow-2xl">
           <div className="relative z-10 grid grid-cols-1 lg:grid-cols-2 gap-12 items-center">
              <div>
                 <div className="h-12 w-12 rounded-2xl bg-white/10 flex items-center justify-center mb-6">
                    <Activity className="h-6 w-6 text-primary" />
                 </div>
                 <h3 className="text-2xl font-bold tracking-tight mb-4">Transcription Summary</h3>
                 <p className="text-slate-400 leading-relaxed text-sm">
                   The coordinates listed above represent the physical mapping of exons within the primary assembly. 
                   During transcription, these segments are joined together to form the mature mRNA molecule, 
                   with introns being precisely excised by the spliceosome.
                 </p>
              </div>
              <div className="grid grid-cols-2 gap-6">
                 <div className="p-6 bg-primary-foreground/5 rounded-2xl border border-primary-foreground/10">
                    <div className="text-[10px] font-bold uppercase tracking-widest text-slate-500 mb-2">Exon Count (first transcript)</div>
                    <div className="text-3xl font-mono font-bold">{gene.transcripts[0]?.exons.length || 0}</div>
                 </div>
                 <div className="p-6 bg-primary-foreground/5 rounded-2xl border border-primary-foreground/10">
                    <div className="text-[10px] font-bold uppercase tracking-widest text-slate-500 mb-2">Summed Exon Length, bp (first transcript)</div>
                    <div className="text-3xl font-mono font-bold text-emerald-400">
                       {formatNumber(gene.transcripts[0]?.exons.reduce((acc, e) => acc + (e.end - e.start), 0) || 0)}
                    </div>
                 </div>
              </div>
           </div>
           <Layers className="absolute -right-20 -bottom-20 w-96 h-96 text-primary-foreground/10 pointer-events-none" />
        </div>

      </div>
    </LibraryShell>
  );
}
