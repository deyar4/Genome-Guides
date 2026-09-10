"use client"

import React from 'react';
import { SidebarNav } from './sidebar-nav';
import { Moon, Sun } from 'lucide-react';
import { useTheme } from 'next-themes';
import { CommandMenu } from './command-menu';

interface LibraryShellProps {
  children: React.ReactNode;
  title?: string;
  subtitle?: string;
}

export function LibraryShell({ children, title, subtitle }: LibraryShellProps) {
  const { theme, setTheme } = useTheme();

  return (
    <div className="flex h-screen w-full bg-background text-foreground overflow-hidden">
      {/* Sidebar */}
      <SidebarNav />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0 h-full overflow-hidden">
        
        {/* Top Navigation / Header */}
        <header className="h-14 border-b border-border flex items-center justify-between px-8 bg-background/85 backdrop-blur-md z-20">
          <div className="flex items-center gap-4 flex-1 max-w-xl">
            <CommandMenu />
          </div>

          <div className="flex items-center gap-4">
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
          <div className="w-full max-w-[1600px] mx-auto px-6 lg:px-10 py-10">
            {(title || subtitle) && (
              <div className="mb-12 border-b border-border pb-8">
                {title && <h1 className="text-4xl font-bold tracking-tight text-foreground">{title}</h1>}
                {subtitle && <p className="text-lg text-muted-foreground mt-2">{subtitle}</p>}
              </div>
            )}
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
