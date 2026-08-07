<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import Icon from '../components/Icon.svelte'
  import {
    fileName,
    getPdfPageCount,
    organizePdf,
    previewPdf,
    type OpResult,
  } from '../api'

  interface PageItem {
    id: string
    sourcePath: string
    page: number
    rotate: 0 | 90 | 180 | 270
    thumb?: string
  }

  let paths = $state<string[]>([])
  let output = $state('')
  let pages = $state<PageItem[]>([])
  let baseline = $state<PageItem[]>([])
  let loading = $state(false)
  let building = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  let dragId = $state<string | null>(null)
  let insertAt = $state<number | null>(null)

  $effect(() => {
    const list = [...paths]
    if (list.length === 0) {
      pages = []
      baseline = []
      return
    }
    void rebuildFromPaths(list)
  })

  async function rebuildFromPaths(list: string[]) {
    building = true
    error = null
    try {
      const items: PageItem[] = []
      for (const sourcePath of list) {
        const count = await getPdfPageCount(sourcePath)
        for (let p = 1; p <= count; p++) {
          items.push({
            id: `${sourcePath}::${p}`,
            sourcePath,
            page: p,
            rotate: 0,
          })
        }
      }
      // Load thumbs in background (cap for speed)
      const maxThumbs = Math.min(items.length, 60)
      for (let i = 0; i < maxThumbs; i++) {
        try {
          const prev = await previewPdf(items[i].sourcePath, items[i].page, 200)
          items[i] = { ...items[i], thumb: prev.dataUrl }
        } catch {
          /* ignore thumb errors */
        }
      }
      pages = items
      baseline = items.map((p) => ({ ...p }))
    } catch (e) {
      error = String(e)
    } finally {
      building = false
    }
  }

  function reset() {
    pages = baseline.map((p) => ({ ...p }))
  }

  function rotatePage(id: string) {
    pages = pages.map((p) =>
      p.id === id
        ? { ...p, rotate: ((p.rotate + 90) % 360) as 0 | 90 | 180 | 270 }
        : p,
    )
  }

  function removePage(id: string) {
    pages = pages.filter((p) => p.id !== id)
  }

  function onDragStart(id: string) {
    dragId = id
  }

  function onDragOver(index: number) {
    if (dragId == null) return
    const from = pages.findIndex((p) => p.id === dragId)
    if (from < 0 || from === index) return
    const next = [...pages]
    const [item] = next.splice(from, 1)
    next.splice(index, 0, item)
    pages = next
  }

  function onDragEnd() {
    dragId = null
  }

  async function insertPdfAt(index: number) {
    const file = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    })
    if (typeof file !== 'string') return
    building = true
    try {
      const count = await getPdfPageCount(file)
      const extras: PageItem[] = []
      for (let p = 1; p <= count; p++) {
        let thumb: string | undefined
        if (p <= 8) {
          try {
            thumb = (await previewPdf(file, p, 200)).dataUrl
          } catch {
            /* ignore */
          }
        }
        extras.push({
          id: `${file}::${p}::${Date.now()}`,
          sourcePath: file,
          page: p,
          rotate: 0,
          thumb,
        })
      }
      const next = [...pages]
      next.splice(index, 0, ...extras)
      pages = next
      if (!paths.includes(file)) paths = [...paths, file]
    } catch (e) {
      error = String(e)
    } finally {
      building = false
      insertAt = null
    }
  }

  function moveFile(from: number, to: number) {
    if (to < 0 || to >= paths.length) return
    const next = [...paths]
    const [item] = next.splice(from, 1)
    next.splice(to, 0, item)
    paths = next
  }

  async function run() {
    error = null
    result = null
    if (pages.length === 0) {
      error = 'No hay páginas para ordenar'
      return
    }
    if (!output) {
      error = 'Selecciona un archivo de salida'
      return
    }
    loading = true
    try {
      result = await organizePdf(
        pages.map((p) => ({
          sourcePath: p.sourcePath,
          page: p.page,
          rotate: p.rotate,
        })),
        output,
      )
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="org-layout">
  <ResultBanner {loading} {error} {result} toolLabel="Ordenar PDF" />
  <div class="org-main">
    <div class="org-workspace">
      <FileDropZone bind:paths accept=".pdf" multiple={true} label="Arrastra uno o más PDFs" />
      {#if building}
        <p class="hint">Cargando miniaturas…</p>
      {/if}
      <div class="thumb-grid">
        {#each pages as p, i (p.id)}
          <div class="thumb-slot">
            <button
              type="button"
              class="insert-btn"
              title="Insertar PDF aquí"
              onclick={() => insertPdfAt(i)}
            >
              +
            </button>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="thumb"
              class:is-dragging={dragId === p.id}
              draggable="true"
              ondragstart={() => onDragStart(p.id)}
              ondragover={(e) => {
                e.preventDefault()
                onDragOver(i)
              }}
              ondragend={onDragEnd}
            >
              <div class="thumb-actions">
                <button type="button" title="Rotar" onclick={() => rotatePage(p.id)}>↻</button>
                <button type="button" title="Eliminar" onclick={() => removePage(p.id)}>×</button>
              </div>
              <div class="thumb-body" style="transform: rotate({p.rotate}deg)">
                {#if p.thumb}
                  <img src={p.thumb} alt="Página {p.page}" draggable="false" />
                {:else}
                  <div class="thumb-ph">p{p.page}</div>
                {/if}
              </div>
              <div class="thumb-meta">
                <span>{i + 1}</span>
                <small>{fileName(p.sourcePath)} · p{p.page}</small>
              </div>
            </div>
          </div>
        {/each}
        {#if pages.length > 0}
          <button type="button" class="insert-btn end" onclick={() => insertPdfAt(pages.length)}>+</button>
        {/if}
      </div>
    </div>

    <aside class="org-side">
      <strong>Ordenar PDF</strong>
      <div class="side-head">
        <span>Archivos:</span>
        <button type="button" class="link" onclick={reset}>Restablecer</button>
      </div>
      <ul class="file-list">
        {#each paths as p, i (p)}
          <li>
            <span class="mono">{fileName(p)}</span>
            <span class="file-actions">
              <button type="button" class="mp-btn mp-btn-ghost" disabled={i === 0} onclick={() => moveFile(i, i - 1)}>
                <Icon name="arrow-up" size={12} />
              </button>
              <button
                type="button"
                class="mp-btn mp-btn-ghost"
                disabled={i === paths.length - 1}
                onclick={() => moveFile(i, i + 1)}
              >
                <Icon name="arrow-down" size={12} />
              </button>
            </span>
          </li>
        {/each}
      </ul>
      <p class="hint">{pages.length} página(s) en el documento final.</p>
      <OutputPicker bind:value={output} defaultName="ordenado.pdf" label="PDF de salida" />
      <button type="button" class="mp-btn mp-btn-primary w-full" disabled={loading || building} onclick={run}>
        Ordenar
      </button>
    </aside>
  </div>
</div>

<style>
  .org-layout { display: flex; flex-direction: column; gap: 1rem; }
  .org-main {
    display: grid;
    grid-template-columns: 1fr min(280px, 34%);
    gap: 1rem;
    align-items: start;
  }
  @media (max-width: 900px) {
    .org-main { grid-template-columns: 1fr; }
  }
  .org-workspace { display: flex; flex-direction: column; gap: 0.75rem; min-width: 0; }
  .thumb-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: flex-start;
    max-height: 65vh;
    overflow: auto;
    padding: 0.25rem;
  }
  .thumb-slot { display: flex; align-items: center; gap: 0.25rem; }
  .thumb {
    width: 120px;
    border: 2px solid var(--color-ink);
    box-shadow: 3px 3px 0 var(--color-ink);
    background: #fff;
    cursor: grab;
    position: relative;
    color: var(--color-ink);
  }
  .thumb.is-dragging { opacity: 0.55; }
  .thumb-body {
    height: 150px;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: #f7f7f7;
  }
  .thumb-body img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .thumb-ph { font-weight: 700; font-size: 12px; }
  .thumb-meta {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0.25rem 0.35rem;
    font-size: 10px;
    border-top: 1px solid color-mix(in srgb, var(--color-ink) 30%, transparent);
  }
  .thumb-meta small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .thumb-actions {
    position: absolute;
    top: 2px;
    right: 2px;
    display: flex;
    gap: 2px;
    z-index: 2;
  }
  .thumb-actions button {
    width: 22px;
    height: 22px;
    border: 1px solid var(--color-ink);
    background: var(--color-banana, #f5d547);
    cursor: pointer;
    font-weight: 700;
    line-height: 1;
  }
  .insert-btn {
    width: 22px;
    height: 22px;
    border: 1px solid var(--color-ink);
    background: #fff;
    color: var(--color-ink);
    font-weight: 800;
    cursor: pointer;
    flex-shrink: 0;
  }
  .insert-btn.end { align-self: center; }
  .org-side {
    border: 2px solid var(--color-ink);
    box-shadow: 4px 4px 0 var(--color-ink);
    background: var(--color-paper, #fff);
    color: var(--color-ink);
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }
  .side-head { display: flex; justify-content: space-between; align-items: center; }
  .link {
    border: 0;
    background: transparent;
    color: #b33;
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
    text-decoration: underline;
  }
  .file-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .file-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.35rem;
    border: 1px dashed color-mix(in srgb, var(--color-ink) 40%, transparent);
    padding: 0.35rem;
    font-size: 12px;
  }
  .file-actions { display: flex; gap: 0.15rem; }
  .hint { font-size: var(--text-xs); color: var(--color-ink-2); margin: 0; }
  .w-full { width: 100%; }
</style>
