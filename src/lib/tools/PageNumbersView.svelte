<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { addPageNumbers, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let position = $state('bottom-center')
  let format = $state('{n} / {total}')
  let startFrom = $state(1)
  let fontSize = $state(10)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  const positions = [
    { id: 'bottom-center', label: 'Abajo centro' },
    { id: 'bottom-right', label: 'Abajo derecha' },
    { id: 'bottom-left', label: 'Abajo izquierda' },
    { id: 'top-center', label: 'Arriba centro' },
  ]

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
      result = await addPageNumbers(paths[0], output, position, format, startFrom, fontSize)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Numerar páginas" />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para numerar" />

  <div class="mp-field">
    <span>Posición</span>
    <div class="mp-hint-row" style="margin-top: 0">
      {#each positions as p}
        <button
          type="button"
          class="mp-chip"
          class:is-on={position === p.id}
          onclick={() => (position = p.id)}>{p.label}</button
        >
      {/each}
    </div>
  </div>

  <div class="mp-field">
    <label for="fmt">Formato ({'{n}'} = página, {'{total}'} = total)</label>
    <input id="fmt" class="mp-input" bind:value={format} />
  </div>

  <div class="mp-field">
    <label for="start">Empezar en</label>
    <input id="start" class="mp-input" type="number" min="1" bind:value={startFrom} />
  </div>

  <div class="mp-field">
    <label for="fs">Tamaño: <span class="mono">{fontSize}</span></label>
    <input id="fs" class="mp-range" type="range" min="8" max="24" step="1" bind:value={fontSize} />
  </div>

  <OutputPicker bind:value={output} defaultName="numbered.pdf" label="PDF de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    Numerar páginas
  </button>
</div>
