import React from 'react';
import { ResponsiveBar } from '@nivo/bar';
import { Card } from '@/components/ui/card';
import { useChartTheme } from '@/hooks/use-chart-theme';
import { NUCLEOTIDE_COLORS } from '@/lib/constants';

interface NucleotideDistributionProps {
  data: {
    A: number;
    C: number;
    G: number;
    T: number;
    label?: string;
  };
  height?: number;
}

export function NucleotideDistribution({ data, height = 56 }: NucleotideDistributionProps) {
  const chartTheme = useChartTheme();
  
  const chartData = [
    {
      id: data.label || 'Distribution',
      A: data.A,
      C: data.C,
      G: data.G,
      T: data.T,
    }
  ];

  return (
    <Card className="p-1.5 bg-white dark:bg-zinc-950 border-slate-200 dark:border-zinc-800 shadow-sm rounded-full overflow-hidden" style={{ height }}>
      <ResponsiveBar
        data={chartData}
        keys={['A', 'C', 'G', 'T']}
        indexBy="id"
        layout="horizontal"
        margin={{ top: 0, right: 0, bottom: 0, left: 0 }}
        padding={0}
        valueScale={{ type: 'linear' }}
        colors={[NUCLEOTIDE_COLORS.A, NUCLEOTIDE_COLORS.C, NUCLEOTIDE_COLORS.G, NUCLEOTIDE_COLORS.T]}
        enableGridY={false}
        axisLeft={null}
        axisBottom={null}
        enableLabel={true}
        label={d => `${d.id}: ${d.value?.toFixed(1) || 0}%`}
        labelTextColor="#ffffff"
        theme={{
          ...chartTheme,
          text: { fontSize: 11, fontWeight: 'bold' }
        }}
        isInteractive={true}
        animate={true}
      />
    </Card>
  );
}
