<script lang="ts">
  import { getPageMediabox, previewPdf, type PageMediaBox } from '../api'
  import { attachMiddlePan } from '../panScroll'
  import { previewRenderWidth } from '../previewScale'

  /* Hallmark · component: page-rect-editor · genre: playful-técnico · theme: press-shop banana
   * states: default · hover · focus · active · disabled · loading · error · success
   * contrast: pass
   */

  export interface NormRect {
    id: string
    page: number
    /** Normalized 0–1 relative to preview bitmap, origin top-left */
    nx: number
    ny: number
    nw: number
    nh: number
  }

  type Handle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w'

  type DragMode =
    | { kind: 'draw'; ox: number; oy: number }
    | { kind: 'move'; id: string; ox: number; oy: number; snx: number; sny: number; snw: number; snh: number }
    | {
        kind: 'resize'
        id: string
        handle: Handle
        ox: number
        oy: number
        snx: number
        sny: number
        snw: number
        snh: number
      }

  interface Props {
    path: string
    mode?: 'multi' | 'single'
    rects?: NormRect[]
  }

  let {
    path,
    mode = 'multi',
    rects = $bindable([] as NormRect[]),
  }: Props = $props()

  let page = $state(1)
  let pageCount = $state(1)
  let previewUrl = $state<string | null>(null)
  let mediabox = $state<PageMediaBox | null>(null)
  let loading = $state(false)
  let loadError = $state<string | null>(null)

  let zoom = $state(1)
  let zoomTimer: ReturnType<typeof setTimeout> | null = null
  let selectedId = $state<string | null>(null)
  let drag = $state<DragMode | null>(null)
  let draft = $state<{ nx: number; ny: number; nw: number; nh: number } | null>(null)

  let surfaceEl = $state<HTMLDivElement | null>(null)
  let viewportEl = $state<HTMLDivElement | null>(null)

  const MIN_SIZE = 0.012
  const ZOOM_MIN = 0.5
  const ZOOM_MAX = 3
  const HANDLES: Handle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']

  const pageRects = $derived(rects.filter((r) => r.page === page))

  $effect(() => {
    if (!viewportEl) return
    return attachMiddlePan(viewportEl)
  })

  async function loadPage(p: number, z = zoom) {
    if (!path) return
    loading = true
    loadError = null
    selectedId = null
    try {
      const w = previewRenderWidth(z, 1000)
      const [prev, box] = await Promise.all([
        previewPdf(path, p, w),
        getPageMediabox(path, p),
      ])
      previewUrl = prev.dataUrl
      pageCount = prev.pageCount
      page = prev.page
      mediabox = box
      void mediabox
    } catch (e) {
      loadError = String(e)
      previewUrl = null
    } finally {
      loading = false
    }
  }

  $effect(() => {
    if (path) {
      zoom = 1
      void loadPage(1)
    } else {
      previewUrl = null
      pageCount = 1
      page = 1
      mediabox = null
      selectedId = null
    }
  })

  function clamp01(v: number) {
    return Math.min(1, Math.max(0, v))
  }

  function normFromEvent(e: PointerEvent): { x: number; y: number } | null {
    if (!surfaceEl) return null
    const rect = surfaceEl.getBoundingClientRect()
    if (rect.width <= 0 || rect.height <= 0) return null
    return {
      x: clamp01((e.clientX - rect.left) / rect.width),
      y: clamp01((e.clientY - rect.top) / rect.height),
    }
  }

  function updateRect(id: string, patch: Partial<NormRect>) {
    rects = rects.map((r) => (r.id === id ? { ...r, ...patch } : r))
  }

  function clampRect(nx: number, ny: number, nw: number, nh: number) {
    let w = Math.max(MIN_SIZE, nw)
    let h = Math.max(MIN_SIZE, nh)
    let x = clamp01(nx)
    let y = clamp01(ny)
    if (x + w > 1) {
      x = Math.max(0, 1 - w)
      w = Math.min(w, 1 - x)
    }
    if (y + h > 1) {
      y = Math.max(0, 1 - h)
      h = Math.min(h, 1 - y)
    }
    return { nx: x, ny: y, nw: Math.max(MIN_SIZE, w), nh: Math.max(MIN_SIZE, h) }
  }

  function applyResize(
    handle: Handle,
    snx: number,
    sny: number,
    snw: number,
    snh: number,
    dx: number,
    dy: number,
  ) {
    let nx = snx
    let ny = sny
    let nw = snw
    let nh = snh

    if (handle.includes('e')) nw = snw + dx
    if (handle.includes('w')) {
      nx = snx + dx
      nw = snw - dx
    }
    if (handle.includes('s')) nh = snh + dy
    if (handle.includes('n')) {
      ny = sny + dy
      nh = snh - dy
    }

    if (nw < MIN_SIZE) {
      if (handle.includes('w')) nx = snx + snw - MIN_SIZE
      nw = MIN_SIZE
    }
    if (nh < MIN_SIZE) {
      if (handle.includes('n')) ny = sny + snh - MIN_SIZE
      nh = MIN_SIZE
    }

    return clampRect(nx, ny, nw, nh)
  }

  function onPointerDownSurface(e: PointerEvent) {
    if (e.button !== 0) return
    if ((e.target as HTMLElement).closest('.rect-box')) return
    const n = normFromEvent(e)
    if (!n) return
    selectedId = null
    drag = { kind: 'draw', ox: n.x, oy: n.y }
    draft = { nx: n.x, ny: n.y, nw: 0, nh: 0 }
    surfaceEl?.setPointerCapture(e.pointerId)
  }

  function onRectPointerDown(e: PointerEvent, id: string) {
    if (e.button !== 0) return
    e.stopPropagation()
    const n = normFromEvent(e)
    if (!n) return
    const r = rects.find((x) => x.id === id)
    if (!r) return
    selectedId = id
    drag = {
      kind: 'move',
      id,
      ox: n.x,
      oy: n.y,
      snx: r.nx,
      sny: r.ny,
      snw: r.nw,
      snh: r.nh,
    }
    surfaceEl?.setPointerCapture(e.pointerId)
  }

  function onHandlePointerDown(e: PointerEvent, id: string, handle: Handle) {
    if (e.button !== 0) return
    e.stopPropagation()
    const n = normFromEvent(e)
    if (!n) return
    const r = rects.find((x) => x.id === id)
    if (!r) return
    selectedId = id
    drag = {
      kind: 'resize',
      id,
      handle,
      ox: n.x,
      oy: n.y,
      snx: r.nx,
      sny: r.ny,
      snw: r.nw,
      snh: r.nh,
    }
    surfaceEl?.setPointerCapture(e.pointerId)
  }

  function onPointerMoveAll(e: PointerEvent) {
    const d = drag
    if (!d) return
    const n = normFromEvent(e)
    if (!n) return

    if (d.kind === 'draw') {
      const x = Math.min(d.ox, n.x)
      const y = Math.min(d.oy, n.y)
      draft = { nx: x, ny: y, nw: Math.abs(n.x - d.ox), nh: Math.abs(n.y - d.oy) }
      return
    }

    if (d.kind === 'move') {
      updateRect(d.id, {
        nx: clamp01(Math.min(Math.max(0, d.snx + (n.x - d.ox)), 1 - d.snw)),
        ny: clamp01(Math.min(Math.max(0, d.sny + (n.y - d.oy)), 1 - d.snh)),
      })
      return
    }

    if (d.kind === 'resize') {
      updateRect(
        d.id,
        applyResize(d.handle, d.snx, d.sny, d.snw, d.snh, n.x - d.ox, n.y - d.oy),
      )
    }
  }

  function onPointerUpAll(e: PointerEvent) {
    const d = drag
    if (!d) return
    try {
      surfaceEl?.releasePointerCapture(e.pointerId)
    } catch {
      /* ignore */
    }

    if (d.kind === 'draw' && draft && draft.nw >= MIN_SIZE && draft.nh >= MIN_SIZE) {
      const next: NormRect = {
        id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
        page,
        ...clampRect(draft.nx, draft.ny, draft.nw, draft.nh),
      }
      rects = mode === 'single' ? [next] : [...rects, next]
      selectedId = next.id
    }

    drag = null
    draft = null
  }

  function removeRect(id: string) {
    rects = rects.filter((r) => r.id !== id)
    if (selectedId === id) selectedId = null
  }

  function clearPage() {
    rects = rects.filter((r) => r.page !== page)
    selectedId = null
  }

  function setZoom(next: number) {
    zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(next * 100) / 100))
    if (zoomTimer) clearTimeout(zoomTimer)
    zoomTimer = setTimeout(() => {
      if (!path) return
      void loadPage(page, zoom)
    }, 160)
  }

  function onWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return
    e.preventDefault()
    setZoom(zoom + (e.deltaY > 0 ? -0.1 : 0.1))
  }
</script>

{#if !path}
  <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">Selecciona un PDF para dibujar zonas.</p>
{:else}
  <div class="space-y-3">
    {#if loadError}
      <p class="text-[var(--text-xs)] text-[var(--color-danger)]" role="alert">{loadError}</p>
    {/if}

    <div class="editor-toolbar">
      <div class="flex items-center gap-2 flex-wrap">
        <button
          type="button"
          class="mp-btn mp-btn-ghost !min-h-9 !px-3"
          disabled={page <= 1 || loading}
          onclick={() => loadPage(page - 1)}
          aria-label="Página anterior"
        >
          ←
        </button>
        <span class="text-[var(--text-xs)] mono">Pág. {page} / {pageCount}</span>
        <button
          type="button"
          class="mp-btn mp-btn-ghost !min-h-9 !px-3"
          disabled={page >= pageCount || loading}
          onclick={() => loadPage(page + 1)}
          aria-label="Página siguiente"
        >
          →
        </button>
      </div>

      <div class="flex items-center gap-1 flex-wrap">
        <button
          type="button"
          class="mp-btn mp-btn-ghost !min-h-9 !px-3"
          onclick={() => setZoom(zoom - 0.25)}
          disabled={zoom <= ZOOM_MIN}
          aria-label="Alejar"
        >
          −
        </button>
        <span class="mono text-[var(--text-xs)] w-12 text-center">{Math.round(zoom * 100)}%</span>
        <button
          type="button"
          class="mp-btn mp-btn-ghost !min-h-9 !px-3"
          onclick={() => setZoom(zoom + 0.25)}
          disabled={zoom >= ZOOM_MAX}
          aria-label="Acercar"
        >
          +
        </button>
        <button
          type="button"
          class="mp-btn mp-btn-ghost !min-h-9 !px-3"
          onclick={() => setZoom(1)}
          disabled={zoom === 1}
        >
          100%
        </button>
        {#if pageRects.length > 0}
          <button type="button" class="mp-btn mp-btn-ghost !min-h-9 !px-3" onclick={clearPage}>
            Limpiar página
          </button>
        {/if}
      </div>
    </div>

    <div class="rect-viewport" bind:this={viewportEl} onwheel={onWheel}>
      <div
        class="rect-surface"
        bind:this={surfaceEl}
        style="width: {zoom * 100}%;"
        onpointerdown={onPointerDownSurface}
        onpointermove={onPointerMoveAll}
        onpointerup={onPointerUpAll}
        onpointercancel={onPointerUpAll}
        role="application"
        aria-label="Editor de zonas: arrastra para marcar, selecciona para mover o redimensionar. Ctrl+rueda para zoom."
      >
        {#if previewUrl}
          <img src={previewUrl} alt="Página {page}" draggable="false" />
        {:else if loading}
          <div class="rect-placeholder">Cargando…</div>
        {:else}
          <div class="rect-placeholder">Sin vista previa</div>
        {/if}

        {#each pageRects as r (r.id)}
          <div
            class="rect-box"
            class:is-selected={selectedId === r.id}
            style="left: {r.nx * 100}%; top: {r.ny * 100}%; width: {r.nw * 100}%; height: {r.nh * 100}%;"
            onpointerdown={(e) => onRectPointerDown(e, r.id)}
            role="button"
            tabindex="0"
            aria-label="Zona de censura"
          >
            {#if selectedId === r.id}
              {#each HANDLES as h}
                <span
                  class="rect-handle handle-{h}"
                  onpointerdown={(e) => onHandlePointerDown(e, r.id, h)}
                  role="presentation"
                ></span>
              {/each}
            {/if}
          </div>
        {/each}
        {#if draft && (draft.nw > 0.005 || draft.nh > 0.005)}
          <div
            class="rect-box is-draft"
            style="left: {draft.nx * 100}%; top: {draft.ny * 100}%; width: {draft.nw * 100}%; height: {draft.nh * 100}%;"
          ></div>
        {/if}
      </div>
    </div>

    {#if rects.length > 0}
      <ul class="rect-list">
        {#each rects as r (r.id)}
          <li class:is-on={selectedId === r.id}>
            <button type="button" class="rect-list-select" onclick={() => (selectedId = r.id)}>
              <span class="mono"
                >p{r.page} · {Math.round(r.nw * 100)}×{Math.round(r.nh * 100)}%</span
              >
            </button>
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              onclick={() => removeRect(r.id)}
            >
              Quitar
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
      Arrastra en vacío para crear. Clic en un cuadro para moverlo; tiradores para redimensionar.
      Ctrl + rueda = zoom · rueda pulsada = panear.
    </p>
  </div>
{/if}

<style>
  .editor-toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .rect-viewport {
    overflow: auto;
    max-height: min(70vh, 640px);
    border: 2px solid var(--color-ink);
    box-shadow: var(--shadow-stamp);
    background: var(--color-paper-2, #f3f1ea);
    padding: var(--space-2);
  }

  .rect-surface {
    position: relative;
    display: block;
    max-width: none;
    cursor: crosshair;
    user-select: none;
    touch-action: none;
    background: var(--color-paper, #fff);
    border: 1.5px solid var(--color-rule, #ccc);
  }

  .rect-surface img {
    display: block;
    width: 100%;
    height: auto;
    pointer-events: none;
  }

  .rect-placeholder {
    width: 100%;
    min-height: 280px;
    aspect-ratio: 3 / 4;
    display: grid;
    place-items: center;
    font-size: var(--text-xs);
    color: var(--color-ink-2);
  }

  .rect-box {
    position: absolute;
    border: 2px solid var(--color-accent-ink);
    background: color-mix(in srgb, var(--color-accent) 40%, transparent);
    cursor: move;
    box-sizing: border-box;
    transition: box-shadow 150ms ease;
  }

  .rect-box.is-selected {
    border-color: var(--color-ink);
    box-shadow: 0 0 0 2px var(--color-accent);
    z-index: 2;
  }

  .rect-box.is-draft {
    border-style: dashed;
    pointer-events: none;
    opacity: 0.85;
  }

  .rect-handle {
    position: absolute;
    width: 10px;
    height: 10px;
    background: var(--color-accent);
    border: 1.5px solid var(--color-ink);
    box-sizing: border-box;
    z-index: 3;
  }

  .handle-nw {
    left: -5px;
    top: -5px;
    cursor: nwse-resize;
  }
  .handle-n {
    left: calc(50% - 5px);
    top: -5px;
    cursor: ns-resize;
  }
  .handle-ne {
    right: -5px;
    top: -5px;
    cursor: nesw-resize;
  }
  .handle-e {
    right: -5px;
    top: calc(50% - 5px);
    cursor: ew-resize;
  }
  .handle-se {
    right: -5px;
    bottom: -5px;
    cursor: nwse-resize;
  }
  .handle-s {
    left: calc(50% - 5px);
    bottom: -5px;
    cursor: ns-resize;
  }
  .handle-sw {
    left: -5px;
    bottom: -5px;
    cursor: nesw-resize;
  }
  .handle-w {
    left: -5px;
    top: calc(50% - 5px);
    cursor: ew-resize;
  }

  .rect-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .rect-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: var(--text-xs);
    padding: 0.2rem 0.35rem;
    border: 1.5px solid transparent;
  }

  .rect-list li.is-on {
    border-color: var(--color-ink);
    background: var(--color-accent-soft);
  }

  .rect-list-select {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: inherit;
  }

  @media (prefers-reduced-motion: reduce) {
    .rect-box {
      transition: none;
    }
  }
</style>
