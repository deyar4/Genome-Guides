import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface GenomeState {
  activeGenome: string;
  setActiveGenome: (genome: string) => void;
  availableGenomes: string[];
  setAvailableGenomes: (genomes: string[]) => void;
}

export const useGenomeStore = create<GenomeState>()(
  persist(
    (set) => ({
      activeGenome: 't2t',
      availableGenomes: ['t2t', 'chrm'],
      setActiveGenome: (genome) => set({ activeGenome: genome }),
      setAvailableGenomes: (genomes) => set({ availableGenomes: genomes }),
    }),
    {
      name: 'genome-library-storage',
    }
  )
);
