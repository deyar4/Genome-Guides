"use client"

import React, { useMemo } from 'react';
import { ResponsiveTreeMap } from '@nivo/treemap';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { formatNumber } from '@/lib/utils';

interface BiotypeTreemapProps {
  data: { biotype: string; count: number }[];
}

export function BiotypeTreemap({ data }: BiotypeTreemapProps) {
  const chartTheme = useChartTheme();

  const hierarchicalData = useMemo(() => {
    // Categorize biotypes into broad classes
    const categories: Record<string, { name: string; children: { name: string; value: number }[] }> = {
      coding: { name: 'Coding', children: [] },
      non_coding: { name: 'Non-coding RNA', children: [] },
      pseudogene: { name: 'Pseudogenes', children: [] },
      other: { name: 'Other Features', children: [] },
    };

    data.forEach(b => {
      const node = { name: b.biotype.replace(/_/g, ' '), value: b.count };
      if (b.biotype === 'protein_coding') {
        categories.coding.children.push(node);
      } else if (b.biotype.includes('pseudogene')) {
        categories.pseudogene.children.push(node);
      } else if (b.biotype.includes('rna') || b.biotype.includes('sno') || b.biotype.includes('sc')) {
        categories.non_coding.children.push(node);
      } else {
        categories.other.children.push(node);
      }
    });

    return {
      name: 'root',
      children: Object.values(categories).filter(c => c.children.length > 0)
    };
  }, [data]);

  return (
    <div className="h-full w-full">
      <ResponsiveTreeMap
        data={hierarchicalData}
        identity="name"
        value="value"
        valueFormat=" >-"
        margin={{ top: 10, right: 10, bottom: 10, left: 10 }}
        labelSkipSize={12}
        label={node => `${node.id}\n(${formatNumber(node.value)})`}
        labelTextColor={{ from: 'color', modifiers: [['darker', 2]] }}
        parentLabelTextColor={{ from: 'color', modifiers: [['darker', 3]] }}
        colors={{ scheme: 'nivo' }}
        borderColor={{ from: 'color', modifiers: [['darker', 0.1]] }}
        theme={chartTheme}
        tooltip={({ node }) => (
          <div className="bg-white dark:bg-zinc-900 p-3 border border-slate-200 dark:border-zinc-800 rounded-lg shadow-xl">
             <div className="text-[10px] font-bold text-slate-400 uppercase tracking-widest mb-1">{node.path.split('.').slice(1, -1).join(' > ')}</div>
             <div className="text-sm font-bold text-slate-900 dark:text-zinc-100 flex items-center justify-between gap-6">
                <span className="capitalize">{node.id}</span>
                <span className="font-mono text-primary">{formatNumber(node.value)}</span>
             </div>
          </div>
        )}
      />
    </div>
  );
}
