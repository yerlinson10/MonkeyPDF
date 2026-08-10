<script lang="ts">
  import { onMount } from 'svelte'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { extractImages, extractText, type OpResult } from '../api'
  import { runWithProgress, type JobProgress } from '../jobProgress'
  import { loadToolPrefs, saveToolPrefs } from '../settings'

  let paths = $state<string[]>([])
  let mode = $state<'images' | 'text'>('images')
  let outputDir = $state('')
  let outputFile = $state('')
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let progress = $state<JobProgress | null>(null)
  let prefsReady = $state(false)

  onMount(() => {
    void loadToolPrefs<{ mode: 'images' | 'text' }>('extract').then((p) => {
      if (p.mode === 'images' || p.mode === 'text') mode = p.mode
      prefsReady = true
    })
    const onRun = () => void run()
    window.addEventListener('mp-run', onRun)
    return () => window.removeEventListener('mp-run', onRun)
  })

  $effect(() => {
    if (!prefsReady) return
    void saveToolPrefs('extract', { mode })
  })

  async function run() {
    error = null
    result = null
    if (!paths[0]) {
      error = 'Selecciona un PDF'
      return
    }
    if (mode === 'images') {
      if (!outputDir) {
        error = 'Selecciona carpeta de salida'
        return
      }
      loading = true
      try {
        result = await runWithProgress(
          (p) => (progress = p),
          () => extractImages(paths[0], outputDir),
        )
      } catch (e) {
        error = String(e)
      } finally {
        loading = false
        progress = null
      }
      return
    }

    if (!outputFile) {
      error = 'Selecciona archivo de salida'
      return
    }
    loading = true
    try {
      result = await runWithProgress(
        (p) => (progress = p),
        () => extractText(paths[0], outputFile),
      )
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
      progress = null
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner
    {loading}
    {error}
    {result}
    {progress}
    toolLabel="Extraer"
    toolId="extract"
    inputs={paths}
    cancellable={true}
  />

  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para extraer" />

  <div class="mp-field">
    <span>Qué extraer</span>
    <div class="flex flex-wrap gap-2" style="margin-top: 0.35rem">
      <button
        type="button"
        class="mp-chip"
        class:is-on={mode === 'images'}
        onclick={() => (mode = 'images')}
      >
        Imágenes
      </button>
      <button
        type="button"
        class="mp-chip"
        class:is-on={mode === 'text'}
        onclick={() => (mode = 'text')}
      >
        Texto (TXT)
      </button>
    </div>
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]" style="margin-top: 0.35rem">
      {#if mode === 'images'}
        Saca XObjects embebidos (JPEG/PNG). No rasteriza páginas — para eso usa PDF→JPG.
      {:else}
        Texto seleccionable a TXT. Si el PDF es un escaneo, usa OCR.
      {/if}
    </p>
  </div>

  {#if mode === 'images'}
    <OutputPicker bind:value={outputDir} tool="extract" mode="directory" label="Carpeta de salida" />
  {:else}
    <OutputPicker
      bind:value={outputFile}
      tool="extract"
      defaultName="extraido.txt"
      filters={[{ name: 'Texto', extensions: ['txt'] }]}
      label="Archivo TXT"
    />
  {/if}

  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    {mode === 'images' ? 'Extraer imágenes' : 'Extraer texto'}
  </button>
</div>
