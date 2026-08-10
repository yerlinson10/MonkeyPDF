<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import PdfPageView from '../components/PdfPageView.svelte'
  import Icon from '../components/Icon.svelte'
  import {
    fileName,
    getPdfPageCount,
    previewImage,
    previewPdf,
    watermarkPdf,
    type OpResult,
    type PreviewTextSpan,
    type WatermarkSpec,
  } from '../api'
  import { previewRenderWidth } from '../previewScale'
  import { runWithProgress, type JobProgress } from '../jobProgress'
  import { loadToolPrefs, saveToolPrefs, resolveOutputDir } from '../settings'
  import { batchSiblingOutput, listPdfsInDir } from '../batch'

  let paths = $state<string[]>([])
  let output = $state('')
  let outputDir = $state('')
  let sourceMode = $state<'file' | 'folder'>('file')
  let folderPath = $state('')
  let batchFiles = $state<string[]>([])
  let mode = $state<'text' | 'image'>('text')
  let text = $state('CONFIDENCIAL')
  let fontSize = $state(36)
  let bold = $state(false)
  let italic = $state(false)
  let underline = $state(false)
  let color = $state('#1a1a1a')
  let imagePath = $state('')
  let imagePreviewUrl = $state<string | null>(null)
  let imageScale = $state(28)
  let position = $state(4)
  let mosaic = $state(false)
  let transparency = $state(50)
  let rotation = $state(45)
  let pageFrom = $state(1)
  let pageTo = $state(1)
  let page = $state(1)
  let pageCount = $state(1)
  let layer = $state<'above' | 'below'>('above')
  let previewUrl = $state<string | null>(null)
  let textSpans = $state<PreviewTextSpan[]>([])
  let previewLoading = $state(false)
  let zoom = $state(1)
  let renderWidth = $state(previewRenderWidth(1))
  let zoomTimer: ReturnType<typeof setTimeout> | null = null
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let progress = $state<JobProgress | null>(null)
  let prefsReady = $state(false)

  const colors = ['#1a1a1a', '#c0392b', '#1e5bb8', '#1e7a3a']
  const ZOOM_MIN = 0.5
  const ZOOM_MAX = 3

  const path = $derived(paths[0] ?? '')
  const showOverlay = $derived(page >= pageFrom && page <= pageTo)

  onMount(() => {
    void loadToolPrefs<{
      mode: 'text' | 'image'
      text: string
      fontSize: number
      position: number
      mosaic: boolean
      transparency: number
      rotation: number
      layer: 'above' | 'below'
      sourceMode: 'file' | 'folder'
    }>('watermark').then((p) => {
      if (p.mode === 'text' || p.mode === 'image') mode = p.mode
      if (typeof p.text === 'string') text = p.text
      if (typeof p.fontSize === 'number') fontSize = p.fontSize
      if (typeof p.position === 'number') position = p.position
      if (typeof p.mosaic === 'boolean') mosaic = p.mosaic
      if (typeof p.transparency === 'number') transparency = p.transparency
      if (typeof p.rotation === 'number') rotation = p.rotation
      if (p.layer === 'above' || p.layer === 'below') layer = p.layer
      if (p.sourceMode === 'file' || p.sourceMode === 'folder') sourceMode = p.sourceMode
      prefsReady = true
    })
    const onRun = () => void run()
    window.addEventListener('mp-run', onRun)
    return () => window.removeEventListener('mp-run', onRun)
  })

  $effect(() => {
    if (!prefsReady) return
    void saveToolPrefs('watermark', {
      mode,
      text,
      fontSize,
      position,
      mosaic,
      transparency,
      rotation,
      layer,
      sourceMode,
    })
  })

  $effect(() => {
    if (path) {
      void loadDocument(path)
    } else {
      previewUrl = null
      textSpans = []
      pageCount = 1
      page = 1
      pageFrom = 1
      pageTo = 1
      zoom = 1
      renderWidth = previewRenderWidth(1)
    }
  })

  $effect(() => {
    const img = imagePath
    if (!img) {
      imagePreviewUrl = null
      return
    }
    void (async () => {
      try {
        const prev = await previewImage(img, 640)
        imagePreviewUrl = prev.dataUrl
      } catch {
        imagePreviewUrl = null
      }
    })()
  })

  async function loadDocument(p: string) {
    previewLoading = true
    error = null
    zoom = 1
    renderWidth = previewRenderWidth(1)
    try {
      pageCount = await getPdfPageCount(p)
      page = 1
      pageFrom = 1
      pageTo = pageCount
      await loadPage(p, 1, renderWidth)
    } catch (e) {
      error = String(e)
      previewUrl = null
      textSpans = []
    } finally {
      previewLoading = false
    }
  }

  async function loadPage(p: string, pg: number, width: number) {
    const prev = await previewPdf(p, pg, width)
    previewUrl = prev.dataUrl
    textSpans = prev.textSpans ?? []
    page = prev.page
    pageCount = prev.pageCount
  }

  function setPage(next: number) {
    if (!path || next < 1 || next > pageCount || next === page) return
    previewLoading = true
    void loadPage(path, next, renderWidth).finally(() => {
      previewLoading = false
    })
  }

  function setZoom(next: number) {
    zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(next * 100) / 100))
    if (zoomTimer) clearTimeout(zoomTimer)
    zoomTimer = setTimeout(() => {
      const w = previewRenderWidth(zoom)
      if (w === renderWidth || !path) return
      renderWidth = w
      previewLoading = true
      void loadPage(path, page, w).finally(() => {
        previewLoading = false
      })
    }, 160)
  }

  function onPreviewWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return
    e.preventDefault()
    setZoom(zoom + (e.deltaY > 0 ? -0.1 : 0.1))
  }

  async function pickImage() {
    const file = await open({
      multiple: false,
      filters: [{ name: 'Imagen', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    })
    if (typeof file === 'string') imagePath = file
  }

  async function pickFolder() {
    const dir = await open({ directory: true, multiple: false })
    if (typeof dir !== 'string') return
    folderPath = dir
    try {
      batchFiles = await listPdfsInDir(dir)
      if (!outputDir) {
        outputDir = (await resolveOutputDir('watermark')) || dir
      }
    } catch (e) {
      error = String(e)
      batchFiles = []
    }
  }

  function buildSpec(): WatermarkSpec {
    return {
      mode,
      text: text.trim(),
      size: Number(fontSize),
      bold,
      italic,
      underline,
      color,
      imagePath: mode === 'image' ? imagePath : null,
      position: Number(position),
      mosaic,
      transparency: Number(transparency),
      rotation: Number(rotation),
      pageFrom: Number(pageFrom),
      pageTo: Number(pageTo),
      layer,
      imageScale: Number(imageScale),
    }
  }

  async function run() {
    error = null
    result = null
    if (mode === 'text' && !text.trim()) {
      error = 'Escribe el texto de la marca'
      return
    }
    if (mode === 'image' && !imagePath) {
      error = 'Añade una imagen'
      return
    }
    if (sourceMode === 'folder') {
      await runBatch()
      return
    }
    if (!paths[0]) {
      error = 'Selecciona un PDF'
      return
    }
    if (!output) {
      error = 'Selecciona un archivo de salida'
      return
    }
    loading = true
    try {
      result = await runWithProgress(
        (p) => (progress = p),
        () => watermarkPdf(paths[0], output, buildSpec()),
      )
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
      progress = null
    }
  }

  async function runBatch() {
    if (batchFiles.length === 0) {
      error = 'Selecciona una carpeta con PDFs'
      return
    }
    if (!outputDir) {
      error = 'Selecciona una carpeta de salida'
      return
    }
    loading = true
    const outputs: string[] = []
    const started = performance.now()
    let pages = 0
    const spec = buildSpec()
    try {
      await runWithProgress(
        (p) => (progress = p),
        async () => {
          for (let i = 0; i < batchFiles.length; i++) {
            const input = batchFiles[i]
            progress = {
              current: i + 1,
              total: batchFiles.length,
              label: `Marca ${fileName(input)} (${i + 1}/${batchFiles.length})`,
            }
            const out = batchSiblingOutput(input, outputDir, '_wm')
            const r = await watermarkPdf(input, out, spec)
            outputs.push(...r.outputPaths)
            pages += r.pageCount
          }
        },
      )
      result = {
        outputPaths: outputs,
        pageCount: pages,
        elapsedMs: Math.round(performance.now() - started),
      }
    } catch (e) {
      const msg = String(e)
      if (outputs.length) {
        result = {
          outputPaths: outputs,
          pageCount: pages,
          elapsedMs: Math.round(performance.now() - started),
          partial: true,
          warnings: [msg],
        }
      } else {
        error = msg
      }
    } finally {
      loading = false
      progress = null
    }
  }

  function posStyle(i: number) {
    // Match backend anchor_point: ~6% margin, box aligned to edges (not centered on corners).
    const margin = 6
    const col = i % 3
    const row = Math.floor(i / 3)
    const tx = col === 1 ? '-50%' : '0'
    const ty = row === 1 ? '-50%' : '0'
    const parts = [
      `opacity:${1 - transparency / 100}`,
      `transform:translate(${tx},${ty}) rotate(${rotation}deg)`,
      'transform-origin:center center',
    ]
    if (mode === 'image') parts.push(`width:${imageScale}%`)

    if (col === 0) parts.push(`left:${margin}%`)
    else if (col === 1) parts.push('left:50%')
    else parts.push(`right:${margin}%`)

    if (row === 0) parts.push(`top:${margin}%`)
    else if (row === 1) parts.push('top:50%')
    else parts.push(`bottom:${margin}%`)

    return parts.join(';')
  }
</script>

<div class="wm-layout">
  <ResultBanner
    {loading}
    {error}
    {result}
    {progress}
    toolLabel="Marca de agua"
    toolId="watermark"
    inputs={sourceMode === 'folder' ? batchFiles : paths}
    cancellable={sourceMode === 'folder'}
  />
  <div class="wm-main">
    <div class="wm-preview">
      <div class="mp-field" style="margin-bottom: 0.75rem">
        <div class="flex flex-wrap gap-2">
          <button
            type="button"
            class="mp-chip"
            class:is-on={sourceMode === 'file'}
            onclick={() => (sourceMode = 'file')}
          >
            Archivo
          </button>
          <button
            type="button"
            class="mp-chip"
            class:is-on={sourceMode === 'folder'}
            onclick={() => (sourceMode = 'folder')}
          >
            Carpeta (lote)
          </button>
        </div>
      </div>

      {#if sourceMode === 'file'}
        <FileDropZone
          bind:paths
          accept=".pdf"
          multiple={false}
          showPreview={false}
          label="Arrastra un PDF"
        />
      {:else}
        <div class="mp-field">
          <button type="button" class="mp-btn mp-btn-ghost" onclick={pickFolder}>Elegir carpeta…</button>
          {#if folderPath}
            <p class="mono text-[var(--text-xs)] text-[var(--color-ink-2)]">{folderPath}</p>
            <p class="text-[var(--text-xs)]">{batchFiles.length} PDF(s)</p>
          {/if}
        </div>
      {/if}

      {#if path}
        <div class="mp-preview-stage">
          <div class="mp-preview-meta">
            <span class="mono selectable truncate" title={path}>{fileName(path)}</span>
            <div class="mp-preview-tools">
              {#if pageCount > 1}
                <div class="mp-preview-pager">
                  <button
                    type="button"
                    class="mp-btn mp-btn-ghost !min-h-8 !px-2"
                    disabled={previewLoading || page <= 1}
                    onclick={() => setPage(page - 1)}
                    aria-label="Página anterior"
                  >
                    <Icon name="arrow-up" size={14} />
                  </button>
                  <span class="mono text-[var(--text-xs)]">{page} / {pageCount}</span>
                  <button
                    type="button"
                    class="mp-btn mp-btn-ghost !min-h-8 !px-2"
                    disabled={previewLoading || page >= pageCount}
                    onclick={() => setPage(page + 1)}
                    aria-label="Página siguiente"
                  >
                    <Icon name="arrow-down" size={14} />
                  </button>
                </div>
              {/if}
              <div class="mp-preview-pager">
                <button
                  type="button"
                  class="mp-btn mp-btn-ghost !min-h-8 !px-2"
                  disabled={zoom <= ZOOM_MIN}
                  onclick={() => setZoom(zoom - 0.25)}
                  aria-label="Alejar"
                >
                  −
                </button>
                <span class="mono text-[var(--text-xs)] w-10 text-center"
                  >{Math.round(zoom * 100)}%</span
                >
                <button
                  type="button"
                  class="mp-btn mp-btn-ghost !min-h-8 !px-2"
                  disabled={zoom >= ZOOM_MAX}
                  onclick={() => setZoom(zoom + 0.25)}
                  aria-label="Acercar"
                >
                  +
                </button>
              </div>
            </div>
          </div>

          {#if previewLoading && !previewUrl}
            <div class="mp-preview-frame">
              <p class="text-[var(--text-sm)] text-[var(--color-ink-2)]">Renderizando vista…</p>
            </div>
          {:else if previewUrl}
            <div class="mp-preview-frame is-live" class:is-loading={previewLoading}>
              <PdfPageView
                src={previewUrl}
                alt="Vista previa de {fileName(path)}"
                widthPercent={zoom * 100}
                {textSpans}
                onCtrlWheel={onPreviewWheel}
                onZoomIn={() => setZoom(zoom + 0.25)}
                onZoomOut={() => setZoom(zoom - 0.25)}
                onZoomReset={() => setZoom(1)}
              >
                {#snippet children()}
                  {#if showOverlay}
                    <div class="wm-clip">
                      {#if mosaic}
                        {#each Array.from({ length: 9 }, (_, i) => i) as i}
                          <div
                            class="wm-overlay"
                            class:is-below={layer === 'below'}
                            class:is-image={mode === 'image'}
                            style={posStyle(i)}
                          >
                            {#if mode === 'text'}
                              <span
                                class="wm-text"
                                style="color:{color};font-size:clamp(10px, {fontSize * 0.11}cqw, {Math.max(
                                  12,
                                  fontSize * 0.42,
                                )}px);font-weight:{bold
                                  ? 700
                                  : 500};font-style:{italic ? 'italic' : 'normal'};text-decoration:{underline
                                  ? 'underline'
                                  : 'none'}">{text}</span
                              >
                            {:else if imagePreviewUrl}
                              <img class="wm-img" src={imagePreviewUrl} alt="" draggable="false" />
                            {:else}
                              <span class="img-ph">IMG</span>
                            {/if}
                          </div>
                        {/each}
                      {:else}
                        <div
                          class="wm-overlay"
                          class:is-below={layer === 'below'}
                          class:is-image={mode === 'image'}
                          style={posStyle(position)}
                        >
                          {#if mode === 'text'}
                            <span
                              class="wm-text"
                              style="color:{color};font-size:clamp(10px, {fontSize * 0.11}cqw, {Math.max(
                                12,
                                fontSize * 0.42,
                              )}px);font-weight:{bold
                                ? 700
                                : 500};font-style:{italic ? 'italic' : 'normal'};text-decoration:{underline
                                ? 'underline'
                                : 'none'}">{text}</span
                            >
                          {:else if imagePreviewUrl}
                            <img class="wm-img" src={imagePreviewUrl} alt="" draggable="false" />
                          {:else}
                            <span class="img-ph">IMG</span>
                          {/if}
                        </div>
                      {/if}
                    </div>
                  {/if}
                {/snippet}
              </PdfPageView>
            </div>
          {/if}

          <p class="mp-preview-hint">
            Rueda pulsada = mover · Ctrl + rueda = zoom · clic derecho = copiar / zoom
          </p>
        </div>
      {/if}
    </div>

    <aside class="wm-side">
      <header class="wm-side-head">
        <strong>Marca de agua</strong>
        <div class="mode-row">
          <button type="button" class="mode-btn" class:is-on={mode === 'text'} onclick={() => (mode = 'text')}>
            Texto
          </button>
          <button type="button" class="mode-btn" class:is-on={mode === 'image'} onclick={() => (mode = 'image')}>
            Imagen
          </button>
        </div>
      </header>

      <div class="wm-side-scroll">
        <section class="wm-sec">
          {#if mode === 'text'}
            <div class="mp-field">
              <label for="wm-text">Texto</label>
              <input id="wm-text" class="mp-input wm-input" bind:value={text} />
            </div>
            <div class="style-row">
              <div class="toolbar">
                <button type="button" class="mp-chip" class:is-on={bold} onclick={() => (bold = !bold)}>B</button>
                <button type="button" class="mp-chip" class:is-on={italic} onclick={() => (italic = !italic)}>I</button>
                <button
                  type="button"
                  class="mp-chip"
                  class:is-on={underline}
                  onclick={() => (underline = !underline)}>U</button
                >
              </div>
              <div class="swatches">
                {#each colors as c}
                  <button
                    type="button"
                    class="swatch"
                    class:is-on={color === c}
                    style="background:{c}"
                    onclick={() => (color = c)}
                    aria-label={c}
                  ></button>
                {/each}
              </div>
            </div>
            <div class="mp-field">
              <label for="wm-fs">Tamaño <span class="mono">{fontSize}</span></label>
              <input id="wm-fs" class="mp-range" type="range" min="8" max="120" step="1" bind:value={fontSize} />
            </div>
          {:else}
            <button type="button" class="mp-btn mp-btn-ghost wm-img-btn" onclick={pickImage}>
              <Icon name="upload" size={16} />
              {imagePath ? 'Cambiar imagen' : 'Añadir imagen'}
            </button>
            {#if imagePath}
              <p class="hint mono">{imagePath.split(/[\\/]/).pop()}</p>
            {/if}
            <div class="mp-field">
              <label for="wm-img-scale">Tamaño <span class="mono">{imageScale}%</span></label>
              <input
                id="wm-img-scale"
                class="mp-range"
                type="range"
                min="5"
                max="90"
                step="1"
                bind:value={imageScale}
              />
            </div>
          {/if}
        </section>

        <section class="wm-sec">
          <div class="sec-label">Colocación</div>
          <div class="place-row">
            <div class="pos-grid" class:is-dim={mosaic}>
              {#each Array.from({ length: 9 }, (_, i) => i) as i}
                <button
                  type="button"
                  class:is-on={position === i}
                  disabled={mosaic}
                  onclick={() => (position = i)}
                  aria-label="Posición {i + 1}"
                ></button>
              {/each}
            </div>
            <label class="mp-check mosaic-check">
              <input type="checkbox" bind:checked={mosaic} />
              <span class="mp-check-box" aria-hidden="true">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M5 12l5 5L20 7" />
                </svg>
              </span>
              <span class="mp-check-label">Mosaico</span>
            </label>
          </div>
        </section>

        <section class="wm-sec">
          <div class="sec-label">Ajustes</div>
          <div class="mp-field">
            <label for="wm-tr">Transparencia <span class="mono">{transparency}%</span></label>
            <input id="wm-tr" class="mp-range" type="range" min="0" max="100" step="1" bind:value={transparency} />
          </div>
          <div class="mp-field">
            <label for="wm-rot">Rotación <span class="mono">{rotation}°</span></label>
            <input id="wm-rot" class="mp-range" type="range" min="0" max="360" step="1" bind:value={rotation} />
          </div>
          <div class="mode-row layer-row">
            <button
              type="button"
              class="mode-btn"
              class:is-on={layer === 'above'}
              onclick={() => (layer = 'above')}>Encima</button
            >
            <button
              type="button"
              class="mode-btn"
              class:is-on={layer === 'below'}
              onclick={() => (layer = 'below')}>Debajo</button
            >
          </div>
        </section>

        <details class="wm-more">
          <summary>Páginas <span class="mono">{pageFrom}–{pageTo}</span></summary>
          <div class="row2">
            <label class="mp-field">
              <span>Desde</span>
              <input class="mp-input wm-input" type="number" min="1" max={pageCount} bind:value={pageFrom} />
            </label>
            <label class="mp-field">
              <span>Hasta</span>
              <input class="mp-input wm-input" type="number" min="1" max={pageCount} bind:value={pageTo} />
            </label>
          </div>
        </details>
      </div>

      <footer class="wm-side-foot">
        {#if sourceMode === 'file'}
          <OutputPicker
            bind:value={output}
            tool="watermark"
            defaultName="marca-agua.pdf"
            label="PDF de salida"
          />
        {:else}
          <OutputPicker
            bind:value={outputDir}
            tool="watermark"
            mode="directory"
            label="Carpeta de salida"
          />
        {/if}
        <button type="button" class="mp-btn mp-btn-primary w-full" disabled={loading} onclick={run}>
          {sourceMode === 'folder' ? 'Marcar carpeta' : 'Insertar marca de agua'}
        </button>
      </footer>
    </aside>
  </div>
</div>

<style>
  .wm-layout { display: flex; flex-direction: column; gap: 1rem; }
  .wm-main {
    display: grid;
    grid-template-columns: 1fr min(280px, 34%);
    gap: 1rem;
    align-items: stretch;
    min-height: 0;
  }
  @media (max-width: 900px) {
    .wm-main { grid-template-columns: 1fr; }
  }
  .wm-preview { display: flex; flex-direction: column; gap: 0.75rem; min-width: 0; }
  .wm-clip {
    position: absolute;
    inset: 0;
    overflow: hidden;
    container-type: inline-size;
    pointer-events: none;
  }
  .wm-overlay {
    position: absolute;
    pointer-events: none;
    z-index: 2;
    max-width: calc(100% - 12%);
  }
  .wm-overlay.is-image {
    max-width: none;
  }
  .wm-overlay.is-below {
    z-index: 1;
    mix-blend-mode: multiply;
  }
  .wm-text {
    font-family: Syne, sans-serif;
    display: inline-block;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    line-height: 1.1;
  }
  .wm-img {
    display: block;
    width: 100%;
    height: auto;
    object-fit: contain;
    background: transparent;
  }
  .img-ph {
    border: 1px dashed var(--color-ink);
    padding: 0.35rem 0.6rem;
    background: color-mix(in srgb, var(--color-banana) 40%, transparent);
    font-size: 11px;
    font-weight: 700;
  }

  .wm-side {
    border: 2px solid var(--color-ink);
    box-shadow: 4px 4px 0 var(--color-ink);
    background: var(--color-paper, #fff);
    color: var(--color-ink);
    display: flex;
    flex-direction: column;
    min-height: 0;
    max-height: min(78vh, 820px);
    overflow: hidden;
  }
  .wm-side-head {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    padding: 0.65rem 0.7rem 0.55rem;
    border-bottom: 1.5px solid var(--color-rule);
    flex-shrink: 0;
  }
  .wm-side-head strong {
    font-size: var(--text-sm);
    letter-spacing: 0.02em;
  }
  .wm-side-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.55rem 0.7rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }
  .wm-side-foot {
    flex-shrink: 0;
    padding: 0.65rem 0.7rem;
    border-top: 1.5px solid var(--color-rule);
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    background: var(--color-paper);
  }

  .wm-sec {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .sec-label {
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--color-ink-2);
  }

  .mode-row { display: flex; gap: 0.3rem; }
  .mode-btn {
    flex: 1;
    font: inherit;
    font-size: 11px;
    font-weight: 700;
    padding: 0.35rem 0.25rem;
    border: 1.5px solid var(--color-ink);
    background: transparent;
    cursor: pointer;
  }
  .mode-btn.is-on { background: var(--color-banana, #f5d547); }
  .layer-row .mode-btn { font-size: 11px; }

  .wm-input { min-height: 36px; padding: 0 0.55rem; font-size: var(--text-xs); }
  .wm-img-btn { width: 100%; justify-content: center; min-height: 36px; }

  .style-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .toolbar { display: flex; gap: 0.25rem; align-items: center; }
  .toolbar :global(.mp-chip) {
    min-height: 30px;
    padding: 0 0.55rem;
  }
  .swatches { display: flex; gap: 0.3rem; }
  .swatch {
    width: 18px;
    height: 18px;
    border-radius: 999px;
    border: 2px solid transparent;
    cursor: pointer;
  }
  .swatch.is-on {
    border-color: var(--color-ink);
    box-shadow: 1px 1px 0 var(--color-ink);
  }

  .place-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .pos-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.25rem;
    width: 72px;
    flex-shrink: 0;
  }
  .pos-grid.is-dim { opacity: 0.35; }
  .pos-grid button {
    aspect-ratio: 1;
    border: 1.5px solid var(--color-ink);
    background: #fff;
    cursor: pointer;
    padding: 0;
  }
  .pos-grid button:disabled { cursor: default; }
  .pos-grid button.is-on { background: var(--color-banana, #f5d547); }
  .mosaic-check { min-height: 0; gap: 0.45rem; }
  .mosaic-check :global(.mp-check-label) { font-size: var(--text-xs); }

  .wm-more {
    border: 1.5px solid var(--color-rule);
    border-radius: var(--radius-sm);
    padding: 0.35rem 0.5rem;
  }
  .wm-more summary {
    cursor: pointer;
    font-size: var(--text-xs);
    font-weight: 700;
    list-style: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    min-height: 28px;
  }
  .wm-more summary::-webkit-details-marker { display: none; }
  .wm-more summary::before {
    content: '+';
    font-weight: 800;
    width: 1rem;
  }
  .wm-more[open] summary::before { content: '−'; }
  .wm-more[open] summary { margin-bottom: 0.4rem; }
  .row2 { display: grid; grid-template-columns: 1fr 1fr; gap: 0.4rem; }

  .hint {
    font-size: 10px;
    margin: 0;
    color: var(--color-ink-2);
    word-break: break-all;
    line-height: 1.3;
  }
  .w-full { width: 100%; }

  .wm-sec :global(.mp-field) { gap: 0.2rem; }
  .wm-sec :global(.mp-field label),
  .wm-sec :global(.mp-field > span),
  .wm-more :global(.mp-field label),
  .wm-more :global(.mp-field > span) {
    font-size: 10px;
  }
</style>
