"use client"

import React from 'react';
import { SidebarNav } from './sidebar-nav';
import { Menu, Moon, Server, Sun } from 'lucide-react';
import { useTheme } from 'next-themes';
import { CommandMenu } from './command-menu';
import { QueryErrorBanner } from './query-error-banner';
import { useQuery } from '@tanstack/react-query';
import { systemApi } from '@/lib/api';
import { cn } from '@/lib/utils';

interface LibraryShellProps {
  children: React.ReactNode;
  title?: string;
  subtitle?: string;
}

export function LibraryShell({ children, title, subtitle }: LibraryShellProps) {
  const { theme, setTheme } = useTheme();
  const [mobileNavOpen, setMobileNavOpen] = React.useState(false);
  const { data: readiness } = useQuery({
    queryKey: ['system-ready'],
    queryFn: systemApi.getReady,
    staleTime: 30_000,
    retry: 1,
  });
  const { data: health } = useQuery({
    queryKey: ['system-health'],
    queryFn: systemApi.getHealth,
    staleTime: 300_000,
    retry: 1,
  });
  const serviceReady = readiness?.status === 'ready';

  return (
    <div className="flex h-screen w-full bg-background text-foreground overflow-hidden">
      {/* Sidebar */}
      <SidebarNav mobileOpen={mobileNavOpen} onClose={() => setMobileNavOpen(false)} />
      {mobileNavOpen && (
        <button
          className="fixed inset-0 z-30 bg-slate-950/45 backdrop-blur-sm lg:hidden"
          onClick={() => setMobileNavOpen(false)}
          aria-label="Close navigation overlay"
        />
      )}

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0 h-full overflow-hidden">
        
        {/* Top Navigation / Header */}
        <header className="min-h-14 border-b border-border flex items-center justify-between gap-4 px-4 sm:px-8 py-2 bg-background/85 backdrop-blur-md z-20">
          <div className="flex items-center gap-4 flex-1 max-w-xl">
            <button className="lg:hidden rounded-lg p-2 hover:bg-accent" onClick={() => setMobileNavOpen(true)} aria-label="Open navigation">
              <Menu className="h-5 w-5" />
            </button>
            <CommandMenu />
          </div>

          <div className="flex items-center gap-4">
            <div title={health ? `API version ${health.version}` : undefined} className={cn(
              "hidden sm:flex items-center gap-2 rounded-full border px-3 py-1.5 text-[11px] font-semibold",
              serviceReady ? "border-emerald-500/25 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400" : "border-amber-500/25 bg-amber-500/10 text-amber-600 dark:text-amber-400"
            )}>
              <span className={cn("h-1.5 w-1.5 rounded-full", serviceReady ? "bg-emerald-500" : "bg-amber-500")} />
              <Server className="h-3.5 w-3.5" />
              {serviceReady ? "Data service ready" : "Connecting to data service"}
            </div>
            <button 
              onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
              className="p-2 text-slate-500 hover:text-slate-900 dark:hover:text-zinc-100 transition-colors"
            >
              <Sun className="h-4 w-4 dark:hidden" />
              <Moon className="h-4 w-4 hidden dark:block" />
            </button>
          </div>
        </header>

        {/* Content Section */}
        <main className="flex-1 overflow-y-auto custom-scrollbar bg-background">
          <div className="w-full max-w-[1600px] mx-auto px-4 sm:px-6 lg:px-10 py-6 sm:py-10">
            {(title || subtitle) && (
              <div className="mb-8 sm:mb-12 border-b border-border pb-6 sm:pb-8">
                {title && <h1 className="text-3xl sm:text-4xl font-bold tracking-tight text-foreground">{title}</h1>}
                {subtitle && <p className="text-base sm:text-lg text-muted-foreground mt-2 max-w-3xl">{subtitle}</p>}
              </div>
            )}
            <QueryErrorBanner />
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
