<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import Icon from '../components/Icon.svelte'
  import {
    getPdfPageCount,
    previewPdf,
    watermarkPdf,
    type OpResult,
  } from '../api'

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
  let position = $state(4)
  let mosaic = $state(false)
  let transparency = $state(50)
  let rotation = $state(45)
  let pageFrom = $state(1)
  let pageTo = $state(1)
  let pageCount = $state(1)
  let layer = $state<'above' | 'below'>('above')
  let previewUrl = $state<string | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  const colors = ['#1a1a1a', '#c0392b', '#1e5bb8', '#1e7a3a']

  $effect(() => {
    const p = paths[0]
    if (!p) {
      previewUrl = null
      pageCount = 1
      return
    }
    void (async () => {
      try {
        pageCount = await getPdfPageCount(p)
        pageTo = pageCount
        pageFrom = 1
        const prev = await previewPdf(p, 1, 720)
        previewUrl = prev.dataUrl
      } catch (e) {
        error = String(e)
      }
    })()
  })

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
    return `left:${left}%;top:${top}%;transform:translate(-50%,-50%) rotate(${rotation}deg);opacity:${1 - transparency / 100}`
  }
</script>

<div class="wm-layout">
  <ResultBanner {loading} {error} {result} toolLabel="Marca de agua" />
  <div class="wm-main">
    <div class="wm-preview">
      <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF" />
      {#if previewUrl}
        <div class="page-wrap">
          <img src={previewUrl} alt="Vista previa" />
          {#if mosaic}
            {#each Array.from({ length: 9 }, (_, i) => i) as i}
              <div class="wm-overlay" style={posStyle(i)}>
                {#if mode === 'text'}
                  <span style="color:{color};font-size:{Math.max(12, fontSize * 0.35)}px;font-weight:{bold ? 700 : 500};font-style:{italic ? 'italic' : 'normal'};text-decoration:{underline ? 'underline' : 'none'}">{text}</span>
                {:else}
                  <span class="img-ph">IMG</span>
                {/if}
              </div>
            {/each}
          {:else}
            <div class="wm-overlay" style={posStyle(position)}>
              {#if mode === 'text'}
                <span style="color:{color};font-size:{Math.max(14, fontSize * 0.4)}px;font-weight:{bold ? 700 : 500};font-style:{italic ? 'italic' : 'normal'};text-decoration:{underline ? 'underline' : 'none'}">{text}</span>
              {:else}
                <span class="img-ph">IMG</span>
              {/if}
            </div>
          {/if}
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
        <div class="toolbar">
          <label>
            Tamaño
            <input class="mp-input" type="number" min="8" max="120" bind:value={fontSize} style="width:4.5rem" />
          </label>
          <button type="button" class="mp-chip" class:is-on={bold} onclick={() => (bold = !bold)}>B</button>
          <button type="button" class="mp-chip" class:is-on={italic} onclick={() => (italic = !italic)}>I</button>
          <button type="button" class="mp-chip" class:is-on={underline} onclick={() => (underline = !underline)}>U</button>
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
      {/if}

      <div class="mp-field">
        <span>Posición</span>
        <div class="pos-grid">
          {#each Array.from({ length: 9 }, (_, i) => i) as i}
            <button type="button" class:is-on={position === i} onclick={() => (position = i)}></button>
          {/each}
        </div>
      </div>

      <label class="check">
        <input type="checkbox" bind:checked={mosaic} />
        Mosaico
      </label>

      <div class="row2">
        <label class="mp-field">
          <span>Transparencia</span>
          <select class="mp-input" bind:value={transparency}>
            <option value={0}>0%</option>
            <option value={25}>25%</option>
            <option value={50}>50%</option>
            <option value={75}>75%</option>
            <option value={100}>100%</option>
          </select>
        </label>
        <label class="mp-field">
          <span>Rotación</span>
          <select class="mp-input" bind:value={rotation}>
            <option value={0}>0°</option>
            <option value={45}>45°</option>
            <option value={90}>90°</option>
            <option value={180}>180°</option>
          </select>
        </label>
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
  .page-wrap {
    position: relative;
    border: 2px solid var(--color-ink);
    box-shadow: 4px 4px 0 var(--color-ink);
    background: #fff;
  }
  .page-wrap img { display: block; width: 100%; height: auto; }
  .wm-overlay {
    position: absolute;
    pointer-events: none;
    white-space: nowrap;
    font-family: Syne, sans-serif;
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
  .toolbar { display: flex; flex-wrap: wrap; gap: 0.35rem; align-items: end; }
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
  .check { display: flex; align-items: center; gap: 0.4rem; font-size: var(--text-sm); }
  .row2 { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
  .hint { font-size: var(--text-xs); margin: 0; color: var(--color-ink-2); word-break: break-all; }
  .w-full { width: 100%; }
</style>
