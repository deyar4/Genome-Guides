import React from 'react';
import { Card } from '@/components/ui/card';
import { Info } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

interface MetricCardProps {
  title: string;
  value: string | number;
  description: string;
  icon?: React.ReactNode;
  info?: string;
  className?: string;
}

export function MetricCard({ title, value, description, icon, info, className }: MetricCardProps) {
  return (
    <Card className={cn("p-6 bg-card border-border shadow-sm", className)}>
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-1.5">
          <div className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">{title}</div>
          {info && (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger>
                  <Info className="h-3 w-3 text-muted-foreground hover:text-primary transition-colors" />
                </TooltipTrigger>
                <TooltipContent className="max-w-[200px] text-[10px] leading-relaxed">
                  {info}
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
        </div>
        <div className="p-1.5 bg-muted rounded-md border border-border">
          {icon}
        </div>
      </div>
      <div className="text-2xl font-mono font-bold text-foreground mb-1">{value}</div>
      <p className="text-[11px] text-muted-foreground leading-relaxed italic">{description}</p>
    </Card>
  );
}

interface SectionHeaderProps {
  title: string;
  subtitle: string;
  badge?: string;
}

export function SectionHeader({ title, subtitle, badge }: SectionHeaderProps) {
  return (
    <div className="flex flex-col gap-1 mb-8">
      <div className="flex items-center gap-3">
        <h2 className="text-2xl font-bold tracking-tight text-foreground">{title}</h2>
        {badge && (
          <span className="px-2 py-0.5 rounded-full bg-primary/10 text-primary text-[10px] font-bold uppercase tracking-widest border border-primary/20">
            {badge}
          </span>
        )}
      </div>
      <p className="text-sm text-muted-foreground italic max-w-2xl">{subtitle}</p>
    </div>
  );
}
