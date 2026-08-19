/** Render width for sharp PDF previews at a given CSS zoom. */
export const PREVIEW_BASE_W = 1400
export const PREVIEW_MAX_W = 3400

function dpr(): number {
  return typeof window !== 'undefined' ? Math.min(window.devicePixelRatio || 1, 3) : 1
}

export function previewRenderWidth(zoom: number, base = PREVIEW_BASE_W): number {
  const z = Number.isFinite(zoom) ? Math.max(zoom, 0.5) : 1
  return Math.min(PREVIEW_MAX_W, Math.max(160, Math.round(base * z * dpr())))
}

/**
 * Pixels needed to show a page at `cssWidth` with no resampling. Rendering to
 * anything else makes the browser rescale the bitmap, which is what turns a
 * vector page into a soft, photocopied-looking image.
 */
export function renderWidthForCss(cssWidth: number): number {
  if (!Number.isFinite(cssWidth) || cssWidth <= 0) return PREVIEW_BASE_W
  return Math.min(PREVIEW_MAX_W, Math.max(320, Math.round(cssWidth * dpr())))
}

/** Avoids re-rendering the page for sub-pixel layout jitter. */
export function needsRerender(current: number, target: number): boolean {
  if (current <= 0) return true
  return target > current * 1.08 || target < current * 0.7
}
