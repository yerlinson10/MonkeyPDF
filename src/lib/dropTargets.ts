/**
 * Routes Tauri OS file-drops to the dropzone under the cursor.
 * Without this, every FileDropZone listens globally and all receive the same drop.
 */
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { getCurrentWindow } from '@tauri-apps/api/window'

export type DropHandler = (paths: string[]) => void

interface DropTarget {
  id: string
  el: HTMLElement
  onDrop: DropHandler
  onHover?: (active: boolean) => void
}

const targets = new Map<string, DropTarget>()
let started = false
let unlisten: (() => void) | undefined
let lastOverId: string | null = null

function pointInRect(x: number, y: number, el: HTMLElement): boolean {
  const r = el.getBoundingClientRect()
  return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
}

async function toCssPoint(physical: { x: number; y: number }): Promise<{ x: number; y: number }> {
  try {
    const factor = await getCurrentWindow().scaleFactor()
    return { x: physical.x / factor, y: physical.y / factor }
  } catch {
    const dpr = window.devicePixelRatio || 1
    return { x: physical.x / dpr, y: physical.y / dpr }
  }
}

function findTargetAt(x: number, y: number): DropTarget | null {
  // Prefer the smallest (most nested) matching target.
  let best: DropTarget | null = null
  let bestArea = Number.POSITIVE_INFINITY
  for (const t of targets.values()) {
    if (!pointInRect(x, y, t.el)) continue
    const r = t.el.getBoundingClientRect()
    const area = r.width * r.height
    if (area < bestArea) {
      best = t
      bestArea = area
    }
  }
  return best
}

function setHover(id: string | null) {
  if (lastOverId === id) return
  if (lastOverId) targets.get(lastOverId)?.onHover?.(false)
  lastOverId = id
  if (id) targets.get(id)?.onHover?.(true)
}

async function ensureListener() {
  if (started) return
  started = true
  try {
    unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
      const payload = event.payload
      if (payload.type === 'leave') {
        setHover(null)
        return
      }

      if (payload.type === 'over' || payload.type === 'enter') {
        const pt = await toCssPoint(payload.position)
        const hit = findTargetAt(pt.x, pt.y)
        setHover(hit?.id ?? null)
        return
      }

      if (payload.type === 'drop') {
        const pt = await toCssPoint(payload.position)
        const hit = findTargetAt(pt.x, pt.y) ?? (lastOverId ? targets.get(lastOverId) : null)
        setHover(null)
        if (hit) hit.onDrop(payload.paths)
      }
    })
  } catch (err) {
    started = false
    console.warn('dropTargets listener failed', err)
  }
}

export function registerDropTarget(target: DropTarget): () => void {
  targets.set(target.id, target)
  void ensureListener()
  return () => {
    targets.delete(target.id)
    if (lastOverId === target.id) {
      lastOverId = null
      target.onHover?.(false)
    }
    if (targets.size === 0 && unlisten) {
      unlisten()
      unlisten = undefined
      started = false
    }
  }
}
