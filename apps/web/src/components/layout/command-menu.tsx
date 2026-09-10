"use client"

import * as React from "react"
import { Search, Loader2, FileText, Layout, Dna, Target, Shapes, Activity, Database, Crosshair } from "lucide-react"
import { Command } from "cmdk"
import * as DialogPrimitive from "@radix-ui/react-dialog"
import { useQuery } from "@tanstack/react-query"
import { useGenomeStore } from "@/store/use-genome-store"
import { genesApi } from "@/lib/api"
import { useRouter } from "next/navigation"

export function CommandMenu() {
  const [open, setOpen] = React.useState(false)
  const [search, setSearch] = React.useState("")
  const { activeGenome } = useGenomeStore()
  const router = useRouter()

  React.useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        setOpen((open) => !open)
      }
    }
    document.addEventListener("keydown", down)
    return () => document.removeEventListener("keydown", down)
  }, [])

  const { data: results, isLoading } = useQuery({
    queryKey: ["cmdk-search", activeGenome, search],
    queryFn: () => genesApi.search(activeGenome, search),
    enabled: search.length >= 2,
  })

  const onSelect = (id: string) => {
    setOpen(false)
    router.push(`/library/genes/${id}`)
  }

  return (
    <>
      <button 
        onClick={() => setOpen(true)}
        className="w-full max-w-xl h-9 bg-slate-100 dark:bg-zinc-900 border-transparent rounded-full pl-4 pr-4 flex items-center gap-3 text-slate-400 hover:bg-slate-200/50 dark:hover:bg-zinc-800 transition-all outline-none group"
      >
        <Search className="h-4 w-4 group-hover:text-slate-900 dark:group-hover:text-zinc-100 transition-colors" />
        <span className="text-sm">Search the library (⌘K)...</span>
        <kbd className="ml-auto hidden sm:inline-flex h-5 select-none items-center gap-1 rounded border border-slate-200 bg-white px-1.5 font-mono text-[10px] font-medium text-slate-400 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-500">
          <span className="text-xs">⌘</span>K
        </kbd>
      </button>

      <Command.Dialog 
        open={open} 
        onOpenChange={setOpen} 
        label="Global Search"
        className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh] bg-slate-900/50 backdrop-blur-sm p-4"
      >
        <div className="w-full max-w-2xl bg-white dark:bg-zinc-900 rounded-2xl shadow-2xl border border-slate-200 dark:border-zinc-800 overflow-hidden">
          {/* Accessibility Title */}
          <DialogPrimitive.Title className="sr-only">Search Genomic Library</DialogPrimitive.Title>
          <DialogPrimitive.Description className="sr-only">Search for genes, chromosomes, or navigate to different library sections.</DialogPrimitive.Description>

          <div className="flex items-center border-b border-slate-100 dark:border-zinc-800 px-4">
            <Search className="h-5 w-5 text-slate-400" />
            <Command.Input 
              value={search}
              onValueChange={setSearch}
              placeholder="Search genes, chromosomes, or metrics..." 
              className="flex-1 h-14 border-none bg-transparent outline-none px-4 text-slate-900 dark:text-zinc-100 text-base"
            />
            {isLoading && <Loader2 className="h-4 w-4 animate-spin text-slate-400" />}
          </div>

          <Command.List className="max-h-[300px] overflow-y-auto p-2 custom-scrollbar">
            <Command.Empty className="py-12 text-center text-sm text-slate-400 italic">
              {search.length < 2 ? "Type at least 2 characters to search..." : "No results found."}
            </Command.Empty>

            <Command.Group heading="Genes" className="px-2 py-2 text-[10px] font-bold uppercase text-slate-400 tracking-widest">
              {results?.map((gene) => (
                <Command.Item
                  key={gene.gene_id}
                  onSelect={() => onSelect(gene.gene_id)}
                  className="flex items-center gap-3 px-3 py-3 rounded-xl cursor-pointer aria-selected:bg-slate-100 dark:aria-selected:bg-zinc-800 transition-colors"
                >
                  <div className="p-2 bg-slate-50 dark:bg-zinc-950 rounded-lg">
                    <FileText className="h-4 w-4 text-primary" />
                  </div>
                  <div className="flex-1">
                    <div className="font-bold text-slate-900 dark:text-zinc-100">{gene.gene_name || gene.gene_id}</div>
                    <div className="text-xs text-slate-500 dark:text-zinc-500">{gene.chrom} • {gene.biotype?.replace('_', ' ')}</div>
                  </div>
                </Command.Item>
              ))}
            </Command.Group>

            <Command.Group heading="Library Sections" className="px-2 py-2 text-[10px] font-bold uppercase text-slate-400 tracking-widest mt-4">
              <SectionItem icon={<Layout className="h-4 w-4" />} label="Genome Overview" onSelect={() => { setOpen(false); router.push('/library/overview'); }} />
              <SectionItem icon={<Dna className="h-4 w-4" />} label="Chromosome Metrics" onSelect={() => { setOpen(false); router.push('/library/dna/metrics'); }} />
              <SectionItem icon={<Activity className="h-4 w-4" />} label="Base Composition" onSelect={() => { setOpen(false); router.push('/library/dna/composition'); }} />
              <SectionItem icon={<Database className="h-4 w-4" />} label="SSR Analysis" onSelect={() => { setOpen(false); router.push('/library/dna/ssr'); }} />
              <SectionItem icon={<Crosshair className="h-4 w-4" />} label="Region Inspector" onSelect={() => { setOpen(false); router.push('/library/dna/inspector'); }} />
              <SectionItem icon={<Search className="h-4 w-4" />} label="Gene Reference" onSelect={() => { setOpen(false); router.push('/library/genes/search'); }} />
              <SectionItem icon={<FileText className="h-4 w-4" />} label="Biotype Census" onSelect={() => { setOpen(false); router.push('/library/genes/census'); }} />
              <SectionItem icon={<Target className="h-4 w-4" />} label="Genomic Distribution" onSelect={() => { setOpen(false); router.push('/library/genes/distribution'); }} />
              <SectionItem icon={<Shapes className="h-4 w-4" />} label="Structural Elements" onSelect={() => { setOpen(false); router.push('/library/dna/structure'); }} />
            </Command.Group>
          </Command.List>
          
          <div className="p-3 bg-slate-50 dark:bg-zinc-950 border-t border-slate-100 dark:border-zinc-800 flex justify-end gap-4">
             <div className="flex items-center gap-1.5">
                <kbd className="px-1.5 py-0.5 rounded border border-slate-200 bg-white dark:bg-zinc-800 text-[10px] font-mono text-slate-400">ESC</kbd>
                <span className="text-[10px] text-slate-400 uppercase font-bold tracking-tighter">to close</span>
             </div>
          </div>
        </div>
      </Command.Dialog>
    </>
  )
}

function SectionItem({ icon, label, onSelect }: { icon: React.ReactNode, label: string, onSelect: () => void }) {
  return (
    <Command.Item
      onSelect={onSelect}
      className="flex items-center gap-3 px-3 py-2.5 rounded-xl cursor-pointer aria-selected:bg-slate-100 dark:aria-selected:bg-zinc-800 transition-colors"
    >
      <div className="p-1.5 text-slate-400">{icon}</div>
      <span className="text-sm font-medium text-slate-700 dark:text-zinc-300">{label}</span>
    </Command.Item>
  )
}
