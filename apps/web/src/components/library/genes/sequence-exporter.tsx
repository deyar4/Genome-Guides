"use client"

import React, { useState } from 'react';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { dnaApi } from '@/lib/api';
import { useGenomeStore } from '@/store/use-genome-store';
import { Download, FileDown, Loader2, Check } from 'lucide-react';

interface SequenceExporterProps {
  chrom: string;
  start: number;
  end: number;
  geneName: string;
}

export function SequenceExporter({ chrom, start, end, geneName }: SequenceExporterProps) {
  const { activeGenome } = useGenomeStore();
  const [loading, setLoading] = useState(false);
  const [downloaded, setDownloaded] = useState(false);

  const downloadFasta = async () => {
    setLoading(true);
    try {
      const seq = await dnaApi.getSequence(activeGenome, chrom, start, end);
      const fasta = `>${activeGenome}_${chrom}:${start}-${end} [gene=${geneName}]\n${seq}`;
      const blob = new Blob([fasta], { type: 'text/plain' });
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${geneName}_genomic.fasta`;
      a.click();
      window.URL.revokeObjectURL(url);
      
      setDownloaded(true);
      setTimeout(() => setDownloaded(false), 3000);
    } catch (err) {
      console.error("Failed to download sequence:", err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card className="p-6 border-slate-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 shadow-sm">
       <div className="flex items-center justify-between gap-8">
          <div className="space-y-1">
             <h4 className="text-xs font-bold text-slate-900 dark:text-zinc-100 uppercase tracking-widest flex items-center gap-2">
                <FileDown className="h-4 w-4 text-emerald-500" />
                Sequence Exporter
             </h4>
             <p className="text-[10px] text-slate-400">Download the raw genomic sequence for this locus.</p>
          </div>
          
          <Button 
            variant="default" 
            size="sm" 
            onClick={downloadFasta}
            disabled={loading}
            className="h-10 gap-2 font-bold uppercase text-[10px]"
          >
             {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : downloaded ? <Check className="h-4 w-4" /> : <Download className="h-4 w-4" />}
             {downloaded ? 'Downloaded' : 'Download FASTA'}
          </Button>
       </div>
    </Card>
  );
}
