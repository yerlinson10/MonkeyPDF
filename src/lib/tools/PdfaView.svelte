<script lang="ts">
  import { onMount } from 'svelte'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { checkLibreOffice, convertToPdfa, type OpResult } from '../api'
  import { loadToolPrefs, saveToolPrefs } from '../settings'

  let paths = $state<string[]>([])
  let outputDir = $state('')
  let version = $state<1 | 2 | 3>(2)
  let available = $state<boolean | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let prefsReady = $state(false)

  const versions: { id: 1 | 2 | 3; label: string; hint: string }[] = [
    { id: 1, label: 'PDF/A-1b', hint: 'Archivo clásico · máxima compatibilidad' },
    { id: 2, label: 'PDF/A-2b', hint: 'Recomendado · PDF 1.7' },
    { id: 3, label: 'PDF/A-3b', hint: 'Permite adjuntos embebidos' },
  ]

  onMount(() => {
    checkLibreOffice()
      .then((ok) => (available = ok))
      .catch(() => (available = false))
    void loadToolPrefs<{ version: 1 | 2 | 3 }>('pdfa').then((p) => {
      if (p.version === 1 || p.version === 2 || p.version === 3) version = p.version
      prefsReady = true
    })
    const onRun = () => void run()
    window.addEventListener('mp-run', onRun)
    return () => window.removeEventListener('mp-run', onRun)
  })

  $effect(() => {
    if (!prefsReady) return
    void saveToolPrefs('pdfa', { version })
  })

  async function run() {
    error = null
    result = null
    if (!paths[0]) {
      error = 'Selecciona un PDF'
      return
    }
    if (!outputDir) {
      error = 'Selecciona carpeta de salida'
      return
    }
    loading = true
    try {
      result = await convertToPdfa(paths[0], version, outputDir)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner
    {loading}
    {error}
    {result}
    toolLabel="PDF/A"
    toolId="pdfa"
    inputs={paths}
  />

  {#if available === false}
    <div class="mp-alert is-warn is-sticky" role="alert">
      <div class="mp-alert-mark" aria-hidden="true">!</div>
      <div class="mp-alert-body">
        <span class="mp-alert-kicker">Requisito</span>
        <p class="mp-alert-title">LibreOffice no detectado</p>
        <p>
          PDF/A se exporta con LibreOffice. Instálalo y reinicia MonkeyPDF.
          <a href="https://www.libreoffice.org/" target="_blank" rel="noreferrer"
            >Descargar LibreOffice →</a
          >
        </p>
      </div>
    </div>
  {:else if available === true}
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
      Re-exporta el PDF vía Writer. Puede cambiar tipografías o layout en documentos complejos.
    </p>
  {/if}

  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para PDF/A" />

  <div class="mp-field">
    <span>Nivel PDF/A</span>
    <div class="flex flex-wrap gap-2" style="margin-top: 0.35rem">
      {#each versions as v}
        <button
          type="button"
          class="mp-chip"
          class:is-on={version === v.id}
          onclick={() => (version = v.id)}
          title={v.hint}
        >
          {v.label}
        </button>
      {/each}
    </div>
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]" style="margin-top: 0.35rem">
      {versions.find((v) => v.id === version)?.hint}
    </p>
  </div>

  <OutputPicker bind:value={outputDir} tool="pdfa" mode="directory" label="Carpeta de salida" />
  <button
    type="button"
    class="mp-btn mp-btn-primary"
    disabled={loading || available === false}
    onclick={run}
  >
    Convertir a PDF/A
  </button>
</div>
