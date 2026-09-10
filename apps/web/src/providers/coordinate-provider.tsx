"use client"

import React, { createContext, useContext, useState } from 'react';

type CoordinateSystem = '0-based' | '1-based';

interface CoordinateContextType {
  system: CoordinateSystem;
  setSystem: (system: CoordinateSystem) => void;
  formatCoord: (pos: number) => number;
  label: string;
}

const CoordinateContext = createContext<CoordinateContextType | undefined>(undefined);

export function CoordinateProvider({ children }: { children: React.ReactNode }) {
  const [system, setSystem] = useState<CoordinateSystem>('0-based');

  const formatCoord = (pos: number) => {
    return system === '1-based' ? pos + 1 : pos;
  };

  const label = system === '0-based' ? '0-based (Engineering)' : '1-based (Academic)';

  return (
    <CoordinateContext.Provider value={{ system, setSystem, formatCoord, label }}>
      {children}
    </CoordinateContext.Provider>
  );
}

export function useCoordinates() {
  const context = useContext(CoordinateContext);
  if (context === undefined) {
    throw new Error('useCoordinates must be used within a CoordinateProvider');
  }
  return context;
}
