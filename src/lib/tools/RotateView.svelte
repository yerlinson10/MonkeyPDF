<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { rotatePdf, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let angle = $state(90)
  let pagesText = $state('')
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

    let pages: number[] | null = null
    if (pagesText.trim()) {
      try {
        pages = pagesText
          .split(',')
          .map((s) => s.trim())
          .filter(Boolean)
          .map((s) => {
            const n = Number(s)
            if (!Number.isInteger(n) || n < 1) throw new Error(`Página inválida: ${s}`)
            return n
          })
      } catch (e) {
        error = String(e)
        return
      }
    }

    loading = true
    try {
      result = await rotatePdf(paths[0], angle, pages, output)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Rotar PDF" />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para rotar" />

  <div class="mp-field">
    <span class="text-[var(--text-sm)] font-semibold">Ángulo</span>
    <div class="flex flex-wrap gap-2">
      {#each [90, 180, 270] as a}
        <button type="button" class="mp-chip" class:is-on={angle === a} onclick={() => (angle = a)}>
          {a}°
        </button>
      {/each}
    </div>
  </div>

  <div class="mp-field">
    <label for="pages">Páginas (opcional, ej. 1,3,5)</label>
    <input
      id="pages"
      class="mp-input mono"
      bind:value={pagesText}
      placeholder="Vacío = todas las páginas"
    />
  </div>

  <OutputPicker bind:value={output} tool="rotate" defaultName="rotated.pdf" label="PDF de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>Rotar PDF</button>
</div>
