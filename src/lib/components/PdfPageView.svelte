<script lang="ts">
  import type { DiffUnderline, PreviewTextSpan } from '../api'
  import { copyText, selectedText } from '../clipboard'
  import { openContextMenu, type CtxItem } from '../contextMenu'
  import { attachMiddlePan } from '../panScroll'

  /* Hallmark · component: pdf-page-view · image + translucent marks + selectable text + pan */

  interface Props {
    src: string
    alt?: string
    widthPercent?: number
    textSpans?: PreviewTextSpan[]
    underlines?: DiffUnderline[]
    underlineTone?: 'del' | 'add' | 'none'
    bare?: boolean
    scrollParent?: HTMLElement | null
    onCtrlWheel?: (e: WheelEvent) => void
    onZoomIn?: () => void
    onZoomOut?: () => void
    onZoomReset?: () => void
    /** Show a selectable text block under the page (always copyable). */
    showTextPanel?: boolean
  }

  let {
    src,
    alt = 'Vista previa PDF',
    widthPercent = 100,
    textSpans = [],
    underlines = [],
    underlineTone = 'none',
    bare = false,
    scrollParent = null,
    onCtrlWheel,
    onZoomIn,
    onZoomOut,
    onZoomReset,
    showTextPanel = true,
  }: Props = $props()

  let viewportEl = $state<HTMLDivElement | null>(null)
  let pageEl = $state<HTMLDivElement | null>(null)
  let pageH = $state(0)
  let toast = $state<string | null>(null)
  let toastTimer: ReturnType<typeof setTimeout> | null = null

  const showMarks = $derived(underlines.length > 0 && underlineTone !== 'none')
  const pageText = $derived(
    textSpans
      .map((s) => s.text)
      .join(' ')
      .replace(/\s+/g, ' ')
      .trim(),
  )

  function flash(msg: string) {
    toast = msg
    if (toastTimer) clearTimeout(toastTimer)
    toastTimer = setTimeout(() => {
      toast = null
    }, 1400)
  }

  function onWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return
    e.preventDefault()
    onCtrlWheel?.(e)
  }

  function onContextMenu(e: MouseEvent) {
    e.preventDefault()
    e.stopPropagation()
    const sel = selectedText().trim()
    const items: CtxItem[] = [
      {
        id: 'copy-sel',
        label: 'Copiar selección',
        hint: 'Ctrl+C',
        disabled: !sel,
        run: async () => {
          if (await copyText(sel)) flash('Copiado')
        },
      },
      {
        id: 'copy-page',
        label: 'Copiar texto de la página',
        disabled: !pageText,
        run: async () => {
          if (await copyText(pageText)) flash('Texto de página copiado')
        },
      },
      { id: 'sep-1', separator: true },
      {
        id: 'zoom-in',
        label: 'Acercar',
        hint: 'Ctrl+rueda',
        disabled: !onZoomIn,
        run: () => onZoomIn?.(),
      },
      {
        id: 'zoom-out',
        label: 'Alejar',
        disabled: !onZoomOut,
        run: () => onZoomOut?.(),
      },
      {
        id: 'zoom-reset',
        label: 'Zoom 100%',
        disabled: !onZoomReset,
        run: () => onZoomReset?.(),
      },
    ]
    openContextMenu(e.clientX, e.clientY, items)
  }

  $effect(() => {
    const el = pageEl
    if (!el) return
    const handler = (e: WheelEvent) => onWheel(e)
    el.addEventListener('wheel', handler, { passive: false })
    return () => el.removeEventListener('wheel', handler)
  })

  function measure() {
    if (!pageEl) return
    pageH = pageEl.getBoundingClientRect().height
  }

  $effect(() => {
    void src
    void widthPercent
    const id = requestAnimationFrame(() => measure())
    return () => cancelAnimationFrame(id)
  })

  $effect(() => {
    const el = pageEl
    if (!el || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(() => measure())
    ro.observe(el)
    return () => ro.disconnect()
  })

  $effect(() => {
    const target = bare ? scrollParent : viewportEl
    if (!target) return
    return attachMiddlePan(target)
  })
</script>

{#if bare}
  <div
    class="pdf-wrap"
    role="application"
    aria-label="Vista previa del documento"
    oncontextmenu={onContextMenu}
  >
    <div class="pdf-page" bind:this={pageEl} style="width: {widthPercent}%;">
      <img {src} {alt} draggable="false" onload={measure} />
      {#if showMarks}
        <svg class="pdf-marks" viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true">
          {#each underlines as u, i (i)}
            {@const ww = Math.max(u.w, 0.01)}
            {@const y2 = Math.min((u.y ?? 0) + Math.max(u.h ?? 0.004, 0.003), 0.999)}
            <line
              class={underlineTone === 'del' ? 'line-del' : 'line-add'}
              x1={u.x}
              y1={y2}
              x2={u.x + ww}
              y2={y2}
            />
          {/each}
        </svg>
      {/if}
      {#if textSpans.length > 0 && pageH > 0}
        <div class="pdf-text-layer" aria-label="Texto seleccionable">
          {#each textSpans as span, i (`${i}-${span.x.toFixed(3)}`)}
            <span
              class="pdf-text-span selectable"
              style="left:{span.x * 100}%;top:{span.y * 100}%;width:{span.w * 100}%;height:{span.h * 100}%;font-size:{Math.max(span.h * pageH, 8)}px;"
              >{span.text}</span
            >
          {/each}
        </div>
      {/if}
    </div>
    {#if showTextPanel && pageText}
      <details class="pdf-text-panel">
        <summary>Texto de la página (seleccionar / copiar)</summary>
        <pre class="selectable">{pageText}</pre>
      </details>
    {/if}
    {#if toast}
      <p class="pdf-toast" role="status">{toast}</p>
    {/if}
  </div>
{:else}
  <div
    class="pdf-viewport"
    role="application"
    aria-label="Vista previa del documento"
    bind:this={viewportEl}
    oncontextmenu={onContextMenu}
  >
    <div class="pdf-page" bind:this={pageEl} style="width: {widthPercent}%;">
      <img {src} {alt} draggable="false" onload={measure} />
      {#if showMarks}
        <svg class="pdf-marks" viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true">
          {#each underlines as u, i (i)}
            {@const ww = Math.max(u.w, 0.01)}
            {@const y2 = Math.min((u.y ?? 0) + Math.max(u.h ?? 0.004, 0.003), 0.999)}
            <line
              class={underlineTone === 'del' ? 'line-del' : 'line-add'}
              x1={u.x}
              y1={y2}
              x2={u.x + ww}
              y2={y2}
            />
          {/each}
        </svg>
      {/if}
      {#if textSpans.length > 0 && pageH > 0}
        <div class="pdf-text-layer" aria-label="Texto seleccionable">
          {#each textSpans as span, i (`${i}-${span.x.toFixed(3)}`)}
            <span
              class="pdf-text-span selectable"
              style="left:{span.x * 100}%;top:{span.y * 100}%;width:{span.w * 100}%;height:{span.h * 100}%;font-size:{Math.max(span.h * pageH, 8)}px;"
              >{span.text}</span
            >
          {/each}
        </div>
      {/if}
    </div>
    {#if showTextPanel && pageText}
      <details class="pdf-text-panel">
        <summary>Texto de la página (seleccionar / copiar)</summary>
        <pre class="selectable">{pageText}</pre>
      </details>
    {/if}
    {#if toast}
      <p class="pdf-toast" role="status">{toast}</p>
    {/if}
  </div>
{/if}

<style>
  .pdf-wrap {
    width: 100%;
  }

  .pdf-viewport {
    min-height: 220px;
    max-height: min(60vh, 520px);
    overflow: auto;
    padding: var(--space-4, 1rem);
    background: var(--color-paper-2, #f3efe6);
  }

  :global(.is-panning) {
    cursor: grabbing !important;
  }

  :global(.is-panning) .pdf-text-layer {
    pointer-events: none;
  }

  .pdf-page {
    position: relative;
    margin-inline: auto;
    max-width: none;
    line-height: 0;
  }

  .pdf-page img {
    display: block;
    width: 100%;
    height: auto;
    border: 1.5px solid var(--color-rule, #cfc6b4);
    background: #fff;
    pointer-events: none;
    user-select: none;
    -webkit-user-drag: none;
  }

  .pdf-marks {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 1;
    overflow: visible;
  }

  .pdf-marks .line-del {
    stroke: #e04040;
    stroke-opacity: 0.5;
    stroke-width: 2.5px;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
  }

  .pdf-marks .line-add {
    stroke: #2a9a4a;
    stroke-opacity: 0.5;
    stroke-width: 2.5px;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
  }

  .pdf-text-layer {
    position: absolute;
    inset: 0;
    z-index: 2;
    overflow: hidden;
  }

  .pdf-text-span {
    position: absolute;
    margin: 0;
    padding: 0;
    border: 0;
    color: transparent;
    caret-color: #111;
    white-space: pre;
    line-height: 1;
    transform-origin: 0 0;
    cursor: text;
    user-select: text;
    -webkit-user-select: text;
    overflow: hidden;
  }

  .pdf-text-span::selection {
    background: rgba(245, 215, 110, 0.55);
    color: transparent;
  }

  .pdf-text-panel {
    margin-top: 0.65rem;
    border: 1.5px solid var(--color-rule, #cfc6b4);
    background: var(--color-paper, #fff);
    padding: 0.4rem 0.6rem;
    font-size: var(--text-xs, 0.75rem);
    line-height: 1.35;
  }

  .pdf-text-panel summary {
    cursor: pointer;
    font-weight: 700;
    color: var(--color-ink-2, #555);
    user-select: none;
  }

  .pdf-text-panel pre {
    margin: 0.5rem 0 0;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 8rem;
    overflow: auto;
    font-family: inherit;
    font-size: inherit;
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }

  .pdf-toast {
    margin: 0.4rem 0 0;
    padding: 0.35rem 0.5rem;
    border: 1.5px solid var(--color-ink, #1a1a1a);
    background: var(--color-accent, #f5d76e);
    font-size: var(--text-xs, 0.75rem);
    font-weight: 800;
  }
</style>
