<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import PageRectEditor, { type NormRect } from '../components/PageRectEditor.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { cropPdf, getPageMediabox, normRectToPdf, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let rects = $state<NormRect[]>([])
  let applyAll = $state(true)
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
      error = 'Dibuja el área de recorte'
      return
    }
    const r = rects[0]
    loading = true
    try {
      const box = await getPageMediabox(paths[0], r.page)
      const crop = normRectToPdf(r.nx, r.ny, r.nw, r.nh, box)
      const pages = applyAll ? null : [r.page]
      result = await cropPdf(paths[0], output, crop, pages)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Recorte" />
  <FileDropZone
    bind:paths
    accept=".pdf"
    multiple={false}
    showPreview={false}
    label="Elige el PDF a recortar"
  />
  {#if paths[0]}
    <PageRectEditor path={paths[0]} mode="single" bind:rects />
  {/if}

  <label class="flex items-center gap-2 text-[var(--text-sm)]">
    <input type="checkbox" bind:checked={applyAll} />
    Aplicar el mismo recorte a todas las páginas
  </label>

  <OutputPicker bind:value={output} defaultName="recortado.pdf" label="PDF de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    Recortar
  </button>
</div>
