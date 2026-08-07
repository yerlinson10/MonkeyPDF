<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import PageRectEditor, { type NormRect } from '../components/PageRectEditor.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { normRectsToPdfRegions, redactPdf, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let rects = $state<NormRect[]>([])
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  $effect(() => {
    if (!paths[0]) rects = []
  })

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
    if (rects.length === 0) {
      error = 'Dibuja al menos una zona de censura'
      return
    }
    loading = true
    try {
      const regions = await normRectsToPdfRegions(paths[0], rects)
      result = await redactPdf(paths[0], output, regions)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Censura" />

  <div class="mp-alert is-warn" role="status">
    <div class="mp-alert-mark" aria-hidden="true">!</div>
    <div class="mp-alert-body">
      <span class="mp-alert-kicker">Seguro</span>
      <p class="mp-alert-title">Censura permanente</p>
      <p>
        Las páginas se aplanan a imagen: el texto censurado no se puede copiar y los campos de
        formulario debajo desaparecen.
      </p>
    </div>
  </div>

  <FileDropZone
    bind:paths
    accept=".pdf"
    multiple={false}
    showPreview={false}
    label="Elige el PDF a censurar"
  />

  {#if paths[0]}
    <PageRectEditor path={paths[0]} mode="multi" bind:rects />
  {/if}

  <OutputPicker bind:value={output} tool="redact" defaultName="censurado.pdf" label="PDF de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    Censurar
  </button>
</div>
