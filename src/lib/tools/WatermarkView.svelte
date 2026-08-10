<script lang="ts">
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
  } from '../api'
  import { previewRenderWidth } from '../previewScale'

  let paths = $state<string[]>([])
  let output = $state('')
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

  const colors = ['#1a1a1a', '#c0392b', '#1e5bb8', '#1e7a3a']
  const ZOOM_MIN = 0.5
  const ZOOM_MAX = 3

  const path = $derived(paths[0] ?? '')
  const showOverlay = $derived(page >= pageFrom && page <= pageTo)

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

  async function run() {
    error = null
    result = null
    if (!paths[0]) {
      error = 'Selecciona un PDF'
      return
    }
    if (!output) {
      error = 'Selecciona un archivo de salida'
      return
    }
    if (mode === 'text' && !text.trim()) {
      error = 'Escribe el texto de la marca'
      return
    }
    if (mode === 'image' && !imagePath) {
      error = 'Añade una imagen'
      return
    }
    loading = true
    try {
      result = await watermarkPdf(paths[0], output, {
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
      })
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  function posStyle(i: number) {
    const col = i % 3
    const row = Math.floor(i / 3)
    const left = col === 0 ? 8 : col === 1 ? 50 : 92
    const top = row === 0 ? 10 : row === 1 ? 50 : 90
    const size =
      mode === 'image' ? `width:${imageScale}%;` : ''
    return `${size}left:${left}%;top:${top}%;transform:translate(-50%,-50%) rotate(${rotation}deg);opacity:${1 - transparency / 100}`
  }
</script>

<div class="wm-layout">
  <ResultBanner {loading} {error} {result} toolLabel="Marca de agua" />
  <div class="wm-main">
    <div class="wm-preview">
      <FileDropZone
        bind:paths
        accept=".pdf"
        multiple={false}
        showPreview={false}
        label="Arrastra un PDF"
      />

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
                    {#if mosaic}
                      {#each Array.from({ length: 9 }, (_, i) => i) as i}
                        <div
                          class="wm-overlay"
                          class:is-below={layer === 'below'}
                          style={posStyle(i)}
                        >
                          {#if mode === 'text'}
                            <span
                              class="wm-text"
                              style="color:{color};font-size:{Math.max(12, fontSize * 0.35)}px;font-weight:{bold
                                ? 700
                                : 500};font-style:{italic ? 'italic' : 'normal'};text-decoration:{underline
                                ? 'underline'
                                : 'none'}">{text}</span
                            >
                          {:else if imagePreviewUrl}
                            <img
                              class="wm-img"
                              src={imagePreviewUrl}
                              alt=""
                              draggable="false"
                            />
                          {:else}
                            <span class="img-ph">IMG</span>
                          {/if}
                        </div>
                      {/each}
                    {:else}
                      <div
                        class="wm-overlay"
                        class:is-below={layer === 'below'}
                        style={posStyle(position)}
                      >
                        {#if mode === 'text'}
                          <span
                            class="wm-text"
                            style="color:{color};font-size:{Math.max(14, fontSize * 0.4)}px;font-weight:{bold
                              ? 700
                              : 500};font-style:{italic ? 'italic' : 'normal'};text-decoration:{underline
                              ? 'underline'
                              : 'none'}">{text}</span
                          >
                        {:else if imagePreviewUrl}
                          <img
                            class="wm-img"
                            src={imagePreviewUrl}
                            alt=""
                            draggable="false"
                          />
                        {:else}
                          <span class="img-ph">IMG</span>
                        {/if}
                      </div>
                    {/if}
                  {/if}
                {/snippet}
              </PdfPageView>
            </div>
          {/if}

          <p class="mp-preview-hint">
            Rueda pulsada = mover · Ctrl + rueda = zoom · clic derecho = copiar / zoom
            {#if layer === 'below'}
              · Capa: por debajo del contenido (en el PDF exportado)
            {/if}
          </p>
        </div>
      {/if}
    </div>

    <aside class="wm-side">
      <strong>Opciones de marca de agua</strong>
      <div class="mode-row">
        <button type="button" class="mode-btn" class:is-on={mode === 'text'} onclick={() => (mode = 'text')}>
          Agregar texto
        </button>
        <button type="button" class="mode-btn" class:is-on={mode === 'image'} onclick={() => (mode = 'image')}>
          Agregar imagen
        </button>
      </div>

      {#if mode === 'text'}
        <div class="mp-field">
          <label for="wm-text">Texto</label>
          <input id="wm-text" class="mp-input" bind:value={text} />
        </div>
        <div class="mp-field">
          <label for="wm-fs">Tamaño: <span class="mono">{fontSize}</span></label>
          <input id="wm-fs" class="mp-range" type="range" min="8" max="120" step="1" bind:value={fontSize} />
        </div>
        <div class="toolbar">
          <button type="button" class="mp-chip" class:is-on={bold} onclick={() => (bold = !bold)}>B</button>
          <button type="button" class="mp-chip" class:is-on={italic} onclick={() => (italic = !italic)}>I</button>
          <button type="button" class="mp-chip" class:is-on={underline} onclick={() => (underline = !underline)}
            >U</button
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
      {:else}
        <button type="button" class="mp-btn mp-btn-ghost" onclick={pickImage}>
          <Icon name="upload" size={16} />
          {imagePath ? 'Cambiar imagen' : 'Añadir imagen'}
        </button>
        {#if imagePath}
          <p class="hint mono">{imagePath.split(/[\\/]/).pop()}</p>
        {/if}
        <div class="mp-field">
          <label for="wm-img-scale">Tamaño imagen: <span class="mono">{imageScale}%</span></label>
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

      <div class="mp-field">
        <span>Posición</span>
        <div class="pos-grid">
          {#each Array.from({ length: 9 }, (_, i) => i) as i}
            <button type="button" class:is-on={position === i} onclick={() => (position = i)}></button>
          {/each}
        </div>
      </div>

      <label class="mp-check">
        <input type="checkbox" bind:checked={mosaic} />
        <span class="mp-check-box" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
            <path d="M5 12l5 5L20 7" />
          </svg>
        </span>
        <span class="mp-check-label">Mosaico</span>
      </label>

      <div class="mp-field">
        <label for="wm-tr">Transparencia: <span class="mono">{transparency}%</span></label>
        <input id="wm-tr" class="mp-range" type="range" min="0" max="100" step="1" bind:value={transparency} />
      </div>

      <div class="mp-field">
        <label for="wm-rot">Rotación: <span class="mono">{rotation}°</span></label>
        <input id="wm-rot" class="mp-range" type="range" min="0" max="360" step="1" bind:value={rotation} />
      </div>

      <div class="row2">
        <label class="mp-field">
          <span>De la página</span>
          <input class="mp-input" type="number" min="1" max={pageCount} bind:value={pageFrom} />
        </label>
        <label class="mp-field">
          <span>a</span>
          <input class="mp-input" type="number" min="1" max={pageCount} bind:value={pageTo} />
        </label>
      </div>

      <div class="mode-row">
        <button type="button" class="mode-btn" class:is-on={layer === 'above'} onclick={() => (layer = 'above')}>
          Por encima
        </button>
        <button type="button" class="mode-btn" class:is-on={layer === 'below'} onclick={() => (layer = 'below')}>
          Por debajo
        </button>
      </div>

      <OutputPicker bind:value={output} defaultName="marca-agua.pdf" label="PDF de salida" />
      <button type="button" class="mp-btn mp-btn-primary w-full" disabled={loading} onclick={run}>
        Insertar marca de agua
      </button>
    </aside>
  </div>
</div>

<style>
  .wm-layout { display: flex; flex-direction: column; gap: 1rem; }
  .wm-main {
    display: grid;
    grid-template-columns: 1fr min(300px, 36%);
    gap: 1rem;
    align-items: start;
  }
  @media (max-width: 900px) {
    .wm-main { grid-template-columns: 1fr; }
  }
  .wm-preview { display: flex; flex-direction: column; gap: 0.75rem; }
  .wm-overlay {
    position: absolute;
    pointer-events: none;
    white-space: nowrap;
    z-index: 2;
  }
  .wm-overlay.is-below {
    z-index: 1;
    mix-blend-mode: multiply;
  }
  .wm-text {
    font-family: Syne, sans-serif;
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
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }
  .mode-row { display: flex; gap: 0.35rem; }
  .mode-btn {
    flex: 1;
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    padding: 0.45rem;
    border: 2px solid var(--color-ink);
    background: transparent;
    cursor: pointer;
  }
  .mode-btn.is-on { background: var(--color-banana, #f5d547); }
  .toolbar { display: flex; flex-wrap: wrap; gap: 0.35rem; align-items: center; }
  .swatches { display: flex; gap: 0.4rem; }
  .swatch {
    width: 22px; height: 22px; border-radius: 999px;
    border: 2px solid transparent; cursor: pointer;
  }
  .swatch.is-on { border-color: var(--color-ink); box-shadow: 2px 2px 0 var(--color-ink); }
  .pos-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 0.3rem;
    max-width: 120px;
  }
  .pos-grid button {
    aspect-ratio: 1;
    border: 2px solid var(--color-ink);
    background: #fff;
    cursor: pointer;
  }
  .pos-grid button.is-on { background: var(--color-banana, #f5d547); }
  .row2 { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
  .hint { font-size: var(--text-xs); margin: 0; color: var(--color-ink-2); word-break: break-all; }
  .w-full { width: 100%; }
</style>
