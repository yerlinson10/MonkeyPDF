<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { comparePdfs, type OpResult } from '../api'

  let pathsA = $state<string[]>([])
  let pathsB = $state<string[]>([])
  let outputDir = $state('')
  let mode = $state<'both' | 'text' | 'visual'>('both')
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  const modes = [
    { id: 'both' as const, label: 'Texto + visual' },
    { id: 'text' as const, label: 'Solo texto' },
    { id: 'visual' as const, label: 'Solo visual' },
  ]

  async function run() {
    error = null
    result = null
    if (!pathsA[0] || !pathsB[0]) {
      error = 'Selecciona PDF A y PDF B'
      return
    }
    if (!outputDir) {
      error = 'Selecciona carpeta de salida'
      return
    }
    loading = true
    try {
      result = await comparePdfs(pathsA[0], pathsB[0], outputDir, mode)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Comparar" />

  <FileDropZone bind:paths={pathsA} accept=".pdf" multiple={false} label="PDF A" />
  <FileDropZone bind:paths={pathsB} accept=".pdf" multiple={false} label="PDF B" />

  <div class="mp-field">
    <span>Modo</span>
    <div class="mp-hint-row" style="margin-top: 0">
      {#each modes as m}
        <button
          type="button"
          class="mp-chip"
          class:is-on={mode === m.id}
          onclick={() => (mode = m.id)}>{m.label}</button
        >
      {/each}
    </div>
  </div>

  <OutputPicker
    bind:value={outputDir}
    mode="directory"
    label="Carpeta de salida (compare.md + diffs)"
  />

  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    Comparar
  </button>
</div>
