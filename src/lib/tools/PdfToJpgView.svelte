<script lang="ts">
  import { onMount } from 'svelte'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { pdfToJpg, type OpResult } from '../api'
  import { runWithProgress, type JobProgress } from '../jobProgress'
  import { loadToolPrefs, saveToolPrefs } from '../settings'

  let paths = $state<string[]>([])
  let outputDir = $state('')
  let dpi = $state(150)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let progress = $state<JobProgress | null>(null)
  let prefsReady = $state(false)

  onMount(() => {
    void loadToolPrefs<{ dpi: number }>('pdf-to-jpg').then((p) => {
      if (typeof p.dpi === 'number') dpi = p.dpi
      prefsReady = true
    })
    const onRun = () => void run()
    window.addEventListener('mp-run', onRun)
    return () => window.removeEventListener('mp-run', onRun)
  })

  $effect(() => {
    if (!prefsReady) return
    void saveToolPrefs('pdf-to-jpg', { dpi })
  })

  async function run() {
    error = null
    result = null
    if (!paths[0]) {
      error = 'Selecciona un PDF'
      return
    }
    if (!outputDir) {
      error = 'Selecciona una carpeta de salida'
      return
    }
    loading = true
    try {
      result = await runWithProgress(
        (p) => (progress = p),
        () => pdfToJpg(paths[0], dpi, outputDir),
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
    toolLabel="PDF a JPG"
    toolId="pdf-to-jpg"
    inputs={paths}
    cancellable={true}
  />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para convertir a JPG" />

  <div class="mp-field">
    <label for="dpi">DPI: <span class="mono">{dpi}</span></label>
    <input id="dpi" class="mp-range" type="range" min="72" max="300" step="1" bind:value={dpi} />
    <div class="flex flex-wrap gap-2">
      {#each [72, 150, 300] as preset}
        <button type="button" class="mp-chip" class:is-on={dpi === preset} onclick={() => (dpi = preset)}>
          {preset} DPI
        </button>
      {/each}
    </div>
  </div>

  <OutputPicker bind:value={outputDir} tool="pdf-to-jpg" mode="directory" label="Carpeta de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>Convertir a JPG</button>
</div>
