<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { pdfToJpg, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let outputDir = $state('')
  let dpi = $state(150)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

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
      result = await pdfToJpg(paths[0], dpi, outputDir)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="PDF a JPG" />
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
