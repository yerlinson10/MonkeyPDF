/** Render width for sharp PDF previews at a given CSS zoom. */
export const PREVIEW_BASE_W = 1000
export const PREVIEW_MAX_W = 2400

export function previewRenderWidth(zoom: number, base = PREVIEW_BASE_W): number {
  const dpr =
    typeof window !== 'undefined' ? Math.min(window.devicePixelRatio || 1, 2) : 1
  const z = Number.isFinite(zoom) ? Math.max(zoom, 0.5) : 1
  return Math.min(PREVIEW_MAX_W, Math.max(160, Math.round(base * z * dpr)))
}
