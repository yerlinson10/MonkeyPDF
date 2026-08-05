<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { jpgToPdf, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  async function run() {
    error = null
    result = null
    if (paths.length < 1) {
      error = 'Selecciona al menos una imagen'
      return
    }
    if (!output) {
      error = 'Selecciona un archivo de salida'
      return
    }
    loading = true
    try {
      result = await jpgToPdf(paths, output)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="JPG a PDF" />
  <FileDropZone
    bind:paths
    accept=".jpg,.jpeg,.png,.webp"
    multiple={true}
    label="Arrastra imágenes (ordenables) para crear un PDF"
  />
  {#if paths.length > 0}
    <p class="text-[var(--text-sm)] text-[var(--color-ink-2)]">
      {paths.length} {paths.length === 1 ? 'imagen' : 'imágenes'} → {paths.length}
      {paths.length === 1 ? 'página' : 'páginas'} (una por imagen, a página completa)
    </p>
  {/if}
  <OutputPicker
    bind:value={output}
    defaultName="images.pdf"
    label="PDF de salida"
    filters={[{ name: 'PDF', extensions: ['pdf'] }]}
  />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>Crear PDF</button>
</div>
