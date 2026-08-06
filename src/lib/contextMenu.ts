export type CtxItem =
  | {
      id: string
      label: string
      hint?: string
      disabled?: boolean
      danger?: boolean
      run: () => void | Promise<void>
    }
  | { id: string; separator: true }

export type CtxState = {
  x: number
  y: number
  items: CtxItem[]
} | null

type Listener = (state: CtxState) => void

let state: CtxState = null
const listeners = new Set<Listener>()

function emit() {
  for (const l of listeners) l(state)
}

export function subscribeContextMenu(listener: Listener): () => void {
  listeners.add(listener)
  listener(state)
  return () => listeners.delete(listener)
}

export function openContextMenu(x: number, y: number, items: CtxItem[]) {
  const pad = 8
  const menuW = 220
  const menuH = Math.min(320, 12 + items.length * 36)
  const left = Math.min(x, window.innerWidth - menuW - pad)
  const top = Math.min(y, window.innerHeight - menuH - pad)
  state = { x: Math.max(pad, left), y: Math.max(pad, top), items }
  emit()
}

export function closeContextMenu() {
  if (!state) return
  state = null
  emit()
}

export function getContextMenuState() {
  return state
}
