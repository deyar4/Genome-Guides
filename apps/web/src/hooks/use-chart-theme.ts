import { useTheme } from 'next-themes';
import { useMemo } from 'react';

/**
 * Hook to provide a consistent Nivo theme that adapts to the current system theme (light/dark).
 * Ensures axis labels, grid lines, and tooltips are readable and aesthetically pleasing.
 */
export function useChartTheme() {
  const { resolvedTheme } = useTheme();
  const isDark = resolvedTheme === 'dark';

  const theme = useMemo(() => ({
    background: 'transparent',
    text: {
      fontSize: 11,
      fill: isDark ? '#a1a1aa' : '#64748b', // zinc-400 : slate-500
      outlineWidth: 0,
      outlineColor: 'transparent',
    },
    axis: {
      domain: {
        line: {
          stroke: isDark ? '#27272a' : '#e2e8f0', // zinc-800 : slate-200
          strokeWidth: 1,
        },
      },
      legend: {
        text: {
          fontSize: 12,
          fill: isDark ? '#d4d4d8' : '#334155', // zinc-300 : slate-700
          fontWeight: 600,
        },
      },
      ticks: {
        line: {
          stroke: isDark ? '#27272a' : '#e2e8f0',
          strokeWidth: 1,
        },
        text: {
          fontSize: 10,
          fill: isDark ? '#71717a' : '#94a3b8', // zinc-500 : slate-400
        },
      },
    },
    grid: {
      line: {
        stroke: isDark ? '#18181b' : '#f1f5f9', // zinc-900 : slate-100
        strokeWidth: 1,
      },
    },
    legends: {
      title: {
        text: {
          fontSize: 11,
          fill: isDark ? '#d4d4d8' : '#334155',
        },
      },
      text: {
        fontSize: 11,
        fill: isDark ? '#a1a1aa' : '#64748b',
      },
      ticks: {
        line: {},
        text: {
          fontSize: 10,
          fill: isDark ? '#a1a1aa' : '#64748b',
        },
      },
    },
    annotations: {
      text: {
        fontSize: 13,
        fill: isDark ? '#d4d4d8' : '#334155',
        outlineWidth: 2,
        outlineColor: isDark ? '#09090b' : '#ffffff',
      },
      link: {
        stroke: isDark ? '#3f3f46' : '#cbd5e1',
        strokeWidth: 1,
        outlineWidth: 2,
        outlineColor: 'transparent',
      },
      outline: {
        stroke: isDark ? '#3f3f46' : '#cbd5e1',
        strokeWidth: 2,
        outlineWidth: 2,
        outlineColor: 'transparent',
      },
      symbol: {
        fill: isDark ? '#3f3f46' : '#cbd5e1',
        outlineWidth: 2,
        outlineColor: 'transparent',
      },
    },
    tooltip: {
      container: {
        background: isDark ? '#18181b' : '#ffffff',
        color: isDark ? '#f4f4f5' : '#1e293b',
        fontSize: 12,
        borderRadius: '8px',
        boxShadow: '0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)',
        border: `1px solid ${isDark ? '#27272a' : '#e2e8f0'}`,
        padding: '8px 12px',
      },
    },
  }), [isDark]);

  return theme;
}
