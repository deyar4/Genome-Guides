"use client"

import React, { useCallback, useMemo, useSyncExternalStore } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { AlertTriangle, RefreshCcw } from "lucide-react"

const SEP = "\u0001"

/**
 * Shared error state for every page: lists the queries of the current page that failed (with the API's message, e.g.
 * "Data not available for this build") and offers one retry. Pages keep their own loading and empty states.
 */
export function QueryErrorBanner() {
  const client = useQueryClient()
  const cache = client.getQueryCache()

  const subscribe = useCallback((notify: () => void) => cache.subscribe(notify), [cache])
  const snapshot = useSyncExternalStore(
    subscribe,
    () =>
      cache
        .getAll()
        .filter((q) => q.state.status === "error" && q.getObserversCount() > 0)
        .map((q) => `${String(q.queryKey[0])}${SEP}${(q.state.error as Error | null)?.message ?? "request failed"}`)
        .join("\u0002"),
    () => ""
  )

  const failed = useMemo(
    () => (snapshot ? snapshot.split("\u0002").map((s) => { const [key, message] = s.split(SEP); return { key, message } }) : []),
    [snapshot]
  )

  const retry = () =>
    client.refetchQueries({ predicate: (q) => q.state.status === "error" && q.getObserversCount() > 0 })

  if (failed.length === 0) return null
  const messages = Array.from(new Set(failed.map((f) => f.message)))

  return (
    <div role="alert" className="mb-6 flex items-start gap-3 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-700 dark:text-red-300">
      <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
      <div className="min-w-0 flex-1">
        <p className="font-semibold">
          {failed.length === 1 ? "One data request failed" : `${failed.length} data requests failed`}; the figures on this page may be incomplete.
        </p>
        <p className="mt-1 break-words text-xs opacity-80">{messages.slice(0, 3).join(" · ")}</p>
      </div>
      <button onClick={retry} className="flex shrink-0 items-center gap-1 rounded-md border border-red-500/40 px-2.5 py-1 text-xs font-semibold hover:bg-red-500/10">
        <RefreshCcw className="h-3 w-3" /> Retry
      </button>
    </div>
  )
}
