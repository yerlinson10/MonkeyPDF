<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { mergePdfs, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  async function run() {
    error = null
    result = null
    if (paths.length < 2) {
      error = 'Selecciona al menos 2 PDFs'
      return
    }
    if (!output) {
      error = 'Selecciona un archivo de salida'
      return
    }
    loading = true
    try {
      result = await mergePdfs(paths, output)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Unir PDF" />
  <FileDropZone bind:paths accept=".pdf" multiple={true} label="Arrastra PDFs para unir (ordenables)" />
  <OutputPicker bind:value={output} tool="merge" defaultName="merged.pdf" label="PDF de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>Unir PDFs</button>
</div>
