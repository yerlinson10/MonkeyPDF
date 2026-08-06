<script lang="ts">
  import { onMount } from 'svelte'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { checkLibreOffice, convertOffice, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let outputDir = $state('')
  let target = $state('pdf')
  let available = $state<boolean | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  const targets = [
    { id: 'pdf', label: '→ PDF' },
    { id: 'docx', label: '→ DOCX' },
    { id: 'xlsx', label: '→ XLSX' },
    { id: 'pptx', label: '→ PPTX' },
    { id: 'html', label: '→ HTML' },
  ]

  onMount(() => {
    checkLibreOffice()
      .then((ok) => (available = ok))
      .catch(() => (available = false))
  })

  async function run() {
    error = null
    result = null
    if (!paths[0]) {
      error = 'Selecciona un archivo'
      return
    }
    if (!outputDir) {
      error = 'Selecciona carpeta de salida'
      return
    }
    loading = true
    try {
      result = await convertOffice(paths[0], target, outputDir)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Office convert" />

  {#if available === false}
    <div class="mp-alert is-warn is-sticky" role="alert">
      <div class="mp-alert-mark" aria-hidden="true">!</div>
      <div class="mp-alert-body">
        <span class="mp-alert-kicker">Requisito</span>
        <p class="mp-alert-title">LibreOffice no detectado</p>
        <p>
          Office necesita LibreOffice en el sistema. Instálalo y reinicia MonkeyPDF.
          <a href="https://www.libreoffice.org/" target="_blank" rel="noreferrer"
            >Descargar LibreOffice →</a
          >
        </p>
      </div>
    </div>
  {:else if available === true}
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
      LibreOffice detectado. PDF→Word usa import Writer. Si falla, cierra LibreOffice y elige carpeta
      local.
    </p>
  {/if}

  <FileDropZone
    bind:paths
    accept=".pdf,.doc,.docx,.xls,.xlsx,.ppt,.pptx,.html,.htm,.odt,.ods,.odp"
    multiple={false}
    label="Arrastra Office / HTML / PDF"
  />

  <div class="mp-field">
    <span>Convertir a</span>
    <div class="mp-hint-row" style="margin-top: 0">
      {#each targets as t}
        <button
          type="button"
          class="mp-chip"
          class:is-on={target === t.id}
          onclick={() => (target = t.id)}>{t.label}</button
        >
      {/each}
    </div>
  </div>

  <OutputPicker bind:value={outputDir} mode="directory" label="Carpeta de salida" />
  <button
    type="button"
    class="mp-btn mp-btn-primary"
    disabled={loading || available === false}
    onclick={run}
  >
    Convertir
  </button>
</div>
