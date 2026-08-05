<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { getPdfPageCount, splitPdf, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let outputDir = $state('')
  let rangesText = $state('1-1')
  let pageCount = $state<number | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  $effect(() => {
    const path = paths[0]
    if (!path) {
      pageCount = null
      return
    }
    getPdfPageCount(path)
      .then((n) => {
        pageCount = n
        if (rangesText === '1-1' && n > 1) rangesText = `1-${n}`
      })
      .catch(() => {
        pageCount = null
      })
  })

  function parseRanges(text: string): Array<[number, number]> {
    return text
      .split(',')
      .map((part) => part.trim())
      .filter(Boolean)
      .map((part) => {
        const m = part.match(/^(\d+)\s*-\s*(\d+)$/) || part.match(/^(\d+)$/)
        if (!m) throw new Error(`Rango inválido: "${part}" (usa 1-3 o 5)`)
        const start = Number(m[1])
        const end = Number(m[2] ?? m[1])
        return [start, end] as [number, number]
      })
  }

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
    let ranges: Array<[number, number]>
    try {
      ranges = parseRanges(rangesText)
    } catch (e) {
      error = String(e)
      return
    }
    loading = true
    try {
      result = await splitPdf(paths[0], ranges, outputDir)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Dividir PDF" />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para dividir" />
  {#if pageCount !== null}
    <p class="mono text-[var(--text-sm)] text-[var(--color-ink-2)]">
      Páginas: <span class="text-[var(--color-ink)]">{pageCount}</span>
    </p>
  {/if}
  <div class="mp-field">
    <label for="ranges">Rangos (ej. 1-3, 5, 7-9)</label>
    <input id="ranges" class="mp-input mono" bind:value={rangesText} />
  </div>
  <OutputPicker bind:value={outputDir} mode="directory" label="Carpeta de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>Dividir PDF</button>
</div>
