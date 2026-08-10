import { LazyStore } from '@tauri-apps/plugin-store'
import type { OutputToolId } from './settings'
import { TOOLS } from './api'

export interface HistoryEntry {
  id: string
  toolId: OutputToolId
  toolLabel: string
  inputs: string[]
  outputs: string[]
  pageCount: number
  elapsedMs: number
  ts: number
}

const MAX_ENTRIES = 30

let store: LazyStore | null = null

function getStore(): LazyStore {
  if (!store) store = new LazyStore('monkeypdf-settings.json')
  return store
}

export async function loadHistory(): Promise<HistoryEntry[]> {
  try {
    const s = getStore()
    const saved = await s.get<HistoryEntry[]>('history')
    return Array.isArray(saved) ? saved : []
  } catch {
    return []
  }
}

export async function pushHistory(entry: Omit<HistoryEntry, 'id' | 'ts' | 'toolLabel'> & {
  toolLabel?: string
}): Promise<HistoryEntry[]> {
  const s = getStore()
  const list = await loadHistory()
  const meta = TOOLS.find((t) => t.id === entry.toolId)
  const next: HistoryEntry = {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    ts: Date.now(),
    toolLabel: entry.toolLabel ?? meta?.title ?? entry.toolId,
    toolId: entry.toolId,
    inputs: entry.inputs,
    outputs: entry.outputs,
    pageCount: entry.pageCount,
    elapsedMs: entry.elapsedMs,
  }
  const updated = [next, ...list.filter((e) => e.id !== next.id)].slice(0, MAX_ENTRIES)
  await s.set('history', updated)
  await s.save()
  return updated
}

export async function clearHistory(): Promise<void> {
  const s = getStore()
  await s.set('history', [])
  await s.save()
}

export function fileName(path: string): string {
  const parts = path.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || path
}
