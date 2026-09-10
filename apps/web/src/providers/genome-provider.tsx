"use client"

import { useEffect } from "react"
import { useGenomeStore } from "@/store/use-genome-store"
import { systemApi } from "@/lib/api"

export function useGenome() {
  const { activeGenome, availableGenomes, setActiveGenome } = useGenomeStore()
  return { activeGenome, availableGenomes, setActiveGenome }
}

export function GenomeProvider({ children }: { children: React.ReactNode }) {
  const { activeGenome, setActiveGenome, setAvailableGenomes } = useGenomeStore()

  useEffect(() => {
    systemApi.getGenomes()
      .then(genomes => {
        if (genomes && genomes.length > 0) {
          setAvailableGenomes(genomes)
          if (!genomes.includes(activeGenome)) {
            setActiveGenome(genomes[0])
          }
        }
      })
      .catch(err => console.error("Failed to fetch available genomes:", err))
  }, [activeGenome, setActiveGenome, setAvailableGenomes])

  return <>{children}</>
}
