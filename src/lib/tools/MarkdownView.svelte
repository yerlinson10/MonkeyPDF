<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { pdfToMarkdown, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
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
      error = 'Selecciona un archivo .md de salida'
      return
    }
    loading = true
    try {
      result = await pdfToMarkdown(paths[0], output)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="PDF → Markdown" />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF con texto" />
  <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
    Extrae texto nativo. PDFs escaneados sin capa de texto: usa la herramienta OCR.
  </p>
  <OutputPicker
    bind:value={output}
    defaultName="documento.md"
    label="Markdown de salida"
    filters={[{ name: 'Markdown', extensions: ['md'] }]}
  />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    Exportar Markdown
  </button>
</div>
