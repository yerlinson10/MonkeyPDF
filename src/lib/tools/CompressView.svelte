<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { compressPdf, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let quality = $state(70)
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
    if (!output) {
      error = 'Selecciona un archivo de salida'
      return
    }
    loading = true
    try {
      result = await compressPdf(paths[0], quality, output)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Comprimir PDF" />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para comprimir" />

  <div class="mp-field">
    <label for="quality">Calidad JPEG: <span class="mono">{quality}</span></label>
    <input id="quality" class="mp-range" type="range" min="10" max="95" step="5" bind:value={quality} />
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
      Menor calidad = archivo más liviano. Recomprime imágenes y, si hace falta, rasteriza páginas a JPEG. Si el PDF ya está optimizado, el tamaño no bajará.
    </p>
  </div>

  <OutputPicker bind:value={output} tool="compress" defaultName="compressed.pdf" label="PDF de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>Comprimir PDF</button>
</div>
