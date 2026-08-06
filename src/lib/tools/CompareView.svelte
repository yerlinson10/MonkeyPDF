<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import PdfPageView from '../components/PdfPageView.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import {
    compareReport,
    fileName,
    previewPdf,
    type CompareReport,
    type FilePreview,
    type OpResult,
    type TextChange,
  } from '../api'
  import { previewRenderWidth } from '../previewScale'

  /* Hallmark · component: compare-workbench · genre: playful-técnico · theme: press-shop banana
   * states: default · hover · focus · active · disabled · loading · error · success
   */

  let pathsA = $state<string[]>([])
  let pathsB = $state<string[]>([])
  let outputDir = $state('')
  let mode = $state<'both' | 'text' | 'visual'>('both')
  let view = $state<'side' | 'overlay'>('side')
  let syncScroll = $state(true)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let report = $state<CompareReport | null>(null)

  let page = $state(1)
  let previewA = $state<FilePreview | null>(null)
  let previewB = $state<FilePreview | null>(null)
  let previewLoading = $state(false)

  let paneA = $state<HTMLDivElement | null>(null)
  let paneB = $state<HTMLDivElement | null>(null)
  let overlayEl = $state<HTMLDivElement | null>(null)
  let syncLock = false
  let zoom = $state(1)
  let zoomTimer: ReturnType<typeof setTimeout> | null = null
  const ZOOM_MIN = 0.5
  const ZOOM_MAX = 3
  const PREVIEW_BASE = 900

  const modes = [
    { id: 'both' as const, label: 'Texto + visual' },
    { id: 'text' as const, label: 'Texto' },
    { id: 'visual' as const, label: 'Visual' },
  ]

  const maxPages = $derived(
    report ? Math.max(report.pagesA, report.pagesB) : Math.max(1, page),
  )

  const pageTextChange = $derived(
    report?.textChanges.find((c) => c.page === page) ?? null,
  )

  const pageVisual = $derived(
    report?.visualPages.find((v) => v.page === page) ?? null,
  )

  const changeCount = $derived(
    (report?.textChanges.length ?? 0) + (report?.visualPages.length ?? 0),
  )

  const ready = $derived(!!pathsA[0] && !!pathsB[0])

  async function loadPreviews(p: number, z = zoom) {
    if (!pathsA[0] || !pathsB[0]) return
    previewLoading = true
    try {
      const w = previewRenderWidth(z, PREVIEW_BASE)
      const [a, b] = await Promise.all([
        previewPdf(pathsA[0], p, w).catch(() => null),
        previewPdf(pathsB[0], p, w).catch(() => null),
      ])
      previewA = a
      previewB = b
      page = p
    } finally {
      previewLoading = false
    }
  }

  $effect(() => {
    if (pathsA[0] && pathsB[0]) {
      report = null
      result = null
      void loadPreviews(1)
    } else {
      previewA = null
      previewB = null
      report = null
    }
  })

  function onScrollA() {
    if (!syncScroll || syncLock || !paneA || !paneB) return
    syncLock = true
    const maxA = paneA.scrollHeight - paneA.clientHeight
    const maxB = paneB.scrollHeight - paneB.clientHeight
    if (maxA > 0 && maxB > 0) {
      paneB.scrollTop = (paneA.scrollTop / maxA) * maxB
    }
    syncLock = false
  }

  function onScrollB() {
    if (!syncScroll || syncLock || !paneA || !paneB) return
    syncLock = true
    const maxA = paneA.scrollHeight - paneA.clientHeight
    const maxB = paneB.scrollHeight - paneB.clientHeight
    if (maxA > 0 && maxB > 0) {
      paneA.scrollTop = (paneB.scrollTop / maxB) * maxA
    }
    syncLock = false
  }

  function setZoom(next: number) {
    zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(next * 100) / 100))
    if (zoomTimer) clearTimeout(zoomTimer)
    zoomTimer = setTimeout(() => {
      if (!pathsA[0] || !pathsB[0]) return
      void loadPreviews(page, zoom)
    }, 180)
  }

  function onPreviewWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return
    e.preventDefault()
    setZoom(zoom + (e.deltaY > 0 ? -0.1 : 0.1))
  }

  async function runCompare(exportToo: boolean) {
    error = null
    result = null
    if (!pathsA[0] || !pathsB[0]) {
      error = 'Selecciona PDF A y PDF B'
      return
    }
    if (exportToo && !outputDir) {
      error = 'Selecciona carpeta de salida para exportar'
      return
    }
    loading = true
    try {
      report = await compareReport(
        pathsA[0],
        pathsB[0],
        mode,
        exportToo ? outputDir : null,
      )
      if (exportToo && report.outputPaths.length) {
        result = {
          outputPaths: report.outputPaths,
          pageCount: Math.max(report.pagesA, report.pagesB),
          elapsedMs: report.elapsedMs,
        }
      }
      const jump =
        report.textChanges[0]?.page ?? report.visualPages[0]?.page ?? page
      if (jump !== page) await loadPreviews(jump)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  function jumpToChange(c: TextChange) {
    void loadPreviews(c.page)
  }

  function kindLabel(kind: string) {
    if (kind === 'only_a') return 'Solo en A'
    if (kind === 'only_b') return 'Solo en B'
    return 'Distinto'
  }
</script>

<div class="compare-root space-y-4">
  <ResultBanner
    {loading}
    {error}
    {result}
    toolLabel="Comparar"
  />

  <div class="compare-inputs">
    <FileDropZone
      bind:paths={pathsA}
      accept=".pdf"
      multiple={false}
      showPreview={false}
      label="PDF A (original)"
    />
    <FileDropZone
      bind:paths={pathsB}
      accept=".pdf"
      multiple={false}
      showPreview={false}
      label="PDF B (nuevo)"
    />
  </div>

  {#if ready}
    <div class="compare-toolbar">
      <div class="mp-hint-row" style="margin-top: 0">
        {#each modes as m}
          <button
            type="button"
            class="mp-chip"
            class:is-on={mode === m.id}
            onclick={() => (mode = m.id)}>{m.label}</button
          >
        {/each}
      </div>

      <div class="mp-hint-row" style="margin-top: 0">
        <button
          type="button"
          class="mp-chip"
          class:is-on={view === 'side'}
          onclick={() => (view = 'side')}>Lado a lado</button
        >
        <button
          type="button"
          class="mp-chip"
          class:is-on={view === 'overlay'}
          onclick={() => (view = 'overlay')}>Mapa visual</button
        >
      </div>

      <label class="mp-check">
        <input type="checkbox" bind:checked={syncScroll} />
        <span class="mp-check-box" aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="3"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M5 12l5 5L20 7" />
          </svg>
        </span>
        <span class="mp-check-label">Sincronizar desplazamiento</span>
      </label>

      <button
        type="button"
        class="mp-btn mp-btn-primary"
        disabled={loading}
        onclick={() => runCompare(false)}
      >
        {loading ? 'Comparando…' : 'Comparar'}
      </button>
    </div>

    <div class="compare-workbench">
      <div class="compare-stage">
        <div class="compare-pager">
          <button
            type="button"
            class="mp-btn mp-btn-ghost !min-h-9 !px-3"
            disabled={page <= 1 || previewLoading}
            onclick={() => loadPreviews(page - 1)}
          >
            ←
          </button>
          <span class="mono text-[var(--text-xs)]"
            >Pág. {page} / {maxPages}</span
          >
          <button
            type="button"
            class="mp-btn mp-btn-ghost !min-h-9 !px-3"
            disabled={page >= maxPages || previewLoading}
            onclick={() => loadPreviews(page + 1)}
          >
            →
          </button>
          <button
            type="button"
            class="mp-btn mp-btn-ghost !min-h-9 !px-3"
            disabled={zoom <= ZOOM_MIN}
            onclick={() => setZoom(zoom - 0.25)}
            aria-label="Alejar"
          >
            −
          </button>
          <span class="mono text-[var(--text-xs)] w-12 text-center"
            >{Math.round(zoom * 100)}%</span
          >
          <button
            type="button"
            class="mp-btn mp-btn-ghost !min-h-9 !px-3"
            disabled={zoom >= ZOOM_MAX}
            onclick={() => setZoom(zoom + 0.25)}
            aria-label="Acercar"
          >
            +
          </button>
          {#if pageTextChange}
            <span class="mp-stamp-tag">{kindLabel(pageTextChange.kind)}</span>
          {/if}
          {#if pageVisual}
            <span class="mp-stamp-tag">~{pageVisual.changedPx} px</span>
          {/if}
        </div>
        {#if report && (mode === 'visual' || mode === 'both')}
          <p class="compare-legend">
            <span><i class="leg-del" aria-hidden="true"></i> Línea roja = distinto en A</span>
            <span><i class="leg-add" aria-hidden="true"></i> Línea verde = distinto en B</span>
          </p>
        {/if}

        {#if view === 'side'}
          <div class="compare-panes">
            <div class="compare-pane">
              <header class="compare-pane-head">
                <strong class:mark-del={!!pageVisual || pageTextChange?.kind === 'only_a' || pageTextChange?.kind === 'changed'}>A</strong>
                <span class="mono selectable truncate">{fileName(pathsA[0])}</span>
              </header>
              <div
                class="compare-pane-scroll"
                class:has-del={!!pageVisual || pageTextChange?.kind === 'only_a' || pageTextChange?.kind === 'changed'}
                bind:this={paneA}
                onscroll={onScrollA}
              >
                {#if previewA}
                  <PdfPageView
                    bare
                    scrollParent={paneA}
                    src={previewA.dataUrl}
                    alt="PDF A página {page}"
                    widthPercent={zoom * 100}
                    textSpans={previewA.textSpans ?? []}
                    underlines={pageVisual?.underlines ?? []}
                    underlineTone={(pageVisual?.underlines?.length ?? 0) > 0 &&
                    pageTextChange?.kind !== 'only_b'
                      ? 'del'
                      : 'none'}
                    showTextPanel={true}
                    onCtrlWheel={onPreviewWheel}
                    onZoomIn={() => setZoom(zoom + 0.25)}
                    onZoomOut={() => setZoom(zoom - 0.25)}
                    onZoomReset={() => setZoom(1)}
                  />
                {:else}
                  <p class="pane-empty">Sin página en A</p>
                {/if}
              </div>
            </div>
            <div class="compare-pane">
              <header class="compare-pane-head">
                <strong class:mark-add={!!pageVisual || pageTextChange?.kind === 'only_b' || pageTextChange?.kind === 'changed'}>B</strong>
                <span class="mono selectable truncate">{fileName(pathsB[0])}</span>
              </header>
              <div
                class="compare-pane-scroll"
                class:has-add={!!pageVisual || pageTextChange?.kind === 'only_b' || pageTextChange?.kind === 'changed'}
                bind:this={paneB}
                onscroll={onScrollB}
              >
                {#if previewB}
                  <PdfPageView
                    bare
                    scrollParent={paneB}
                    src={previewB.dataUrl}
                    alt="PDF B página {page}"
                    widthPercent={zoom * 100}
                    textSpans={previewB.textSpans ?? []}
                    underlines={pageVisual?.underlines ?? []}
                    underlineTone={(pageVisual?.underlines?.length ?? 0) > 0 &&
                    pageTextChange?.kind !== 'only_a'
                      ? 'add'
                      : 'none'}
                    showTextPanel={true}
                    onCtrlWheel={onPreviewWheel}
                    onZoomIn={() => setZoom(zoom + 0.25)}
                    onZoomOut={() => setZoom(zoom - 0.25)}
                    onZoomReset={() => setZoom(1)}
                  />
                {:else}
                  <p class="pane-empty">Sin página en B</p>
                {/if}
              </div>
            </div>
          </div>
        {:else}
          <div class="compare-overlay" bind:this={overlayEl}>
            {#if pageVisual?.diffDataUrl}
              <PdfPageView
                bare
                scrollParent={overlayEl}
                src={pageVisual.diffDataUrl}
                alt="Diff página {page}"
                widthPercent={zoom * 100}
                onCtrlWheel={onPreviewWheel}
                onZoomIn={() => setZoom(zoom + 0.25)}
                onZoomOut={() => setZoom(zoom - 0.25)}
                onZoomReset={() => setZoom(1)}
              />
              <p class="text-[var(--text-xs)] text-[var(--color-ink-2)] selectable">
                Mapa JPEG (al exportar). En lado a lado: resalte semitransparente + texto seleccionable.
              </p>
            {:else if pageVisual?.underlines?.length && (previewA || previewB)}
              <PdfPageView
                bare
                scrollParent={overlayEl}
                src={(previewB ?? previewA)!.dataUrl}
                alt="Cambios página {page}"
                widthPercent={zoom * 100}
                textSpans={(previewB ?? previewA)?.textSpans ?? []}
                underlines={pageVisual.underlines}
                underlineTone="add"
                onCtrlWheel={onPreviewWheel}
                onZoomIn={() => setZoom(zoom + 0.25)}
                onZoomOut={() => setZoom(zoom - 0.25)}
                onZoomReset={() => setZoom(1)}
              />
              <p class="text-[var(--text-xs)] text-[var(--color-ink-2)] selectable">
                Resalte = zonas distintas. Selecciona el texto encima para copiar.
              </p>
            {:else if report}
              <p class="pane-empty">Sin diff visual en esta página.</p>
            {:else}
              <p class="pane-empty">Pulsa Comparar para marcar diferencias.</p>
            {/if}
          </div>
        {/if}
      </div>

      <aside class="compare-report" aria-label="Informe de cambios">
        <div class="compare-report-head">
          <span class="mp-stamp-tag">Informe</span>
          <h2>
            {#if report}
              {changeCount} cambio{changeCount === 1 ? '' : 's'}
            {:else}
              Sin informe aún
            {/if}
          </h2>
          <p>
            {#if report}
              A {report.pagesA} pág. · B {report.pagesB} pág. · {report.elapsedMs} ms
            {:else}
              Compara para ver texto y diferencias visuales.
            {/if}
          </p>
        </div>

        {#if report}
          <div class="compare-report-body">
            {#if report.textChanges.length === 0 && report.visualPages.length === 0}
              <p class="text-[var(--text-sm)]">Sin diferencias detectadas.</p>
            {/if}

            {#each report.textChanges as c (c.page + c.kind)}
              <div class="change-card" class:is-on={c.page === page}>
                <button
                  type="button"
                  class="change-kicker-btn"
                  onclick={() => jumpToChange(c)}
                >
                  Pág. {c.page} · {kindLabel(c.kind)} · ir
                </button>
                {#if c.kind === 'only_b' || c.kind === 'changed'}
                  <pre class="change-block is-add selectable">{c.textB || '—'}</pre>
                {/if}
                {#if c.kind === 'only_a' || c.kind === 'changed'}
                  <pre class="change-block is-del selectable">{c.textA || '—'}</pre>
                {/if}
              </div>
            {/each}

            {#if report.visualPages.length > 0}
              <h3 class="report-sub">Visual</h3>
              {#each report.visualPages as v (v.page)}
                <button
                  type="button"
                  class="change-card"
                  class:is-on={v.page === page && view === 'overlay'}
                  onclick={() => {
                    view = 'overlay'
                    void loadPreviews(v.page)
                  }}
                >
                  <span class="change-kicker"
                    >Pág. {v.page} · ~{v.changedPx} px</span
                  >
                  <img class="change-thumb" src={v.diffDataUrl} alt="Diff {v.page}" />
                </button>
              {/each}
            {/if}
          </div>
        {/if}

        <div class="compare-export">
          <OutputPicker
            bind:value={outputDir}
            mode="directory"
            label="Exportar informe"
          />
          <button
            type="button"
            class="mp-btn mp-btn-ghost"
            disabled={loading || !ready}
            onclick={() => runCompare(true)}
          >
            Descargar informe
          </button>
        </div>
      </aside>
    </div>
  {/if}
</div>

<style>
  .compare-root {
    width: 100%;
  }

  .compare-inputs {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-4);
  }

  .compare-toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    padding-bottom: var(--space-3);
    border-bottom: 1.5px solid var(--color-rule);
  }

  .compare-workbench {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(16rem, 20rem);
    gap: var(--space-4);
    align-items: stretch;
    min-height: min(70vh, 720px);
  }

  .compare-stage {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
    min-height: 0;
  }

  .compare-pager {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }

  .compare-panes {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3);
    flex: 1;
    min-height: 0;
  }

  .compare-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 2px solid var(--color-ink);
    box-shadow: var(--shadow-stamp);
    background: var(--color-paper);
  }

  .compare-pane-head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-bottom: 1.5px solid var(--color-rule);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .compare-pane-head strong {
    display: grid;
    place-items: center;
    width: 1.4rem;
    height: 1.4rem;
    background: var(--color-accent);
    border: 1.5px solid var(--color-ink);
    flex-shrink: 0;
  }

  .compare-pane-head strong.mark-del {
    background: color-mix(in srgb, var(--color-danger) 55%, white);
    color: var(--color-danger);
  }

  .compare-pane-head strong.mark-add {
    background: color-mix(in srgb, #22a048 40%, white);
    color: #167a36;
  }

  .compare-pane-scroll {
    flex: 1;
    overflow: auto;
    min-height: 280px;
    max-height: min(62vh, 640px);
    background: var(--color-paper-2);
    padding: var(--space-2);
  }

  .compare-pane-scroll.has-del {
    box-shadow: inset 4px 0 0 var(--color-danger);
  }

  .compare-pane-scroll.has-add {
    box-shadow: inset 4px 0 0 #22a048;
  }

  .compare-legend {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
    margin: 0;
    padding: 0 var(--space-1);
    font-size: var(--text-xs);
    font-weight: 700;
    color: var(--color-ink-2);
  }

  .compare-legend span {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }

  .compare-legend i {
    display: inline-block;
    width: 0.65rem;
    height: 0.65rem;
    border: 1.5px solid var(--color-ink);
  }

  .compare-legend .leg-del {
    background: rgba(220, 48, 48, 0.45);
  }

  .compare-legend .leg-add {
    background: rgba(34, 160, 72, 0.45);
  }

  .compare-overlay {
    flex: 1;
    min-height: 280px;
    max-height: min(62vh, 640px);
    overflow: auto;
    border: 2px solid var(--color-ink);
    box-shadow: var(--shadow-stamp);
    background: var(--color-paper-2);
    padding: var(--space-2);
  }

  .pane-empty {
    margin: var(--space-6);
    text-align: center;
    font-size: var(--text-sm);
    color: var(--color-ink-2);
  }

  .compare-report {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 2px solid var(--color-ink);
    box-shadow: var(--shadow-stamp);
    background: var(--color-paper);
  }

  .compare-report-head {
    padding: var(--space-3) var(--space-4);
    border-bottom: 1.5px solid var(--color-rule);
  }

  .compare-report-head h2 {
    margin: var(--space-2) 0 var(--space-1);
    font-size: var(--text-lg);
    font-weight: 800;
  }

  .compare-report-head p {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--color-ink-2);
  }

  .compare-report-body {
    flex: 1;
    overflow: auto;
    padding: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-height: min(50vh, 480px);
  }

  .report-sub {
    margin: var(--space-2) 0 0;
    font-size: var(--text-xs);
    font-weight: 800;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .change-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    text-align: left;
    padding: var(--space-2);
    border: 1.5px solid var(--color-rule);
    background: var(--color-paper);
    border-radius: var(--radius-sm);
    transition:
      border-color var(--dur-ui) var(--ease-hover),
      background-color var(--dur-ui) var(--ease-hover);
  }

  .change-card:hover {
    border-color: var(--color-ink);
  }

  .change-card.is-on {
    border-color: var(--color-ink);
    background: var(--color-accent-soft);
    box-shadow: 2px 2px 0 var(--color-ink);
  }

  .change-kicker,
  .change-kicker-btn {
    font-size: 0.65rem;
    font-weight: 800;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-ink-2);
    background: transparent;
    border: 0;
    padding: 0;
    text-align: left;
    cursor: pointer;
  }

  .change-kicker-btn:hover {
    color: var(--color-ink);
    text-decoration: underline;
  }

  .change-block {
    margin: 0;
    padding: var(--space-2);
    max-height: 7rem;
    overflow: auto;
    font-size: var(--text-xs);
    font-family: inherit;
    white-space: pre-wrap;
    word-break: break-word;
    border: 1.5px solid var(--color-rule);
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }

  .change-block.is-add {
    background: color-mix(in srgb, var(--color-accent) 35%, white);
  }

  .change-block.is-del {
    background: var(--color-danger-soft);
    border-color: var(--color-danger);
  }

  .change-thumb {
    width: 100%;
    height: auto;
    border: 1.5px solid var(--color-rule);
  }

  .compare-export {
    padding: var(--space-3) var(--space-4);
    border-top: 1.5px solid var(--color-rule);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  @media (max-width: 960px) {
    .compare-inputs {
      grid-template-columns: 1fr;
    }

    .compare-workbench {
      grid-template-columns: 1fr;
      min-height: 0;
    }

    .compare-panes {
      grid-template-columns: 1fr;
    }

    .compare-report-body {
      max-height: 240px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .change-card {
      transition: none;
    }
  }
</style>
