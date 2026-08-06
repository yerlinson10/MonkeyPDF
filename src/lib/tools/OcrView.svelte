<script lang="ts">
  import { onMount } from 'svelte'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { checkTesseract, ocrPdf, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let lang = $state('spa+eng')
  let mode = $state<'markdown' | 'txt' | 'searchable_pdf'>('markdown')
  let available = $state<boolean | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  const langs = [
    { id: 'spa+eng', label: 'spa+eng' },
    { id: 'spa', label: 'spa' },
    { id: 'eng', label: 'eng' },
  ]

  const modes = [
    { id: 'markdown' as const, label: 'Markdown', ext: 'md', name: 'ocr.md' },
    { id: 'txt' as const, label: 'TXT', ext: 'txt', name: 'ocr.txt' },
    { id: 'searchable_pdf' as const, label: 'PDF buscable', ext: 'pdf', name: 'ocr.pdf' },
  ]

  const modeMeta = $derived(modes.find((m) => m.id === mode) ?? modes[0])

  onMount(() => {
    checkTesseract()
      .then((ok) => (available = ok))
      .catch(() => (available = false))
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
    loading = true
    try {
      result = await ocrPdf(paths[0], output, lang, mode)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="OCR" />

  {#if available === false}
    <div class="mp-alert is-warn is-sticky" role="alert">
      <div class="mp-alert-mark" aria-hidden="true">!</div>
      <div class="mp-alert-body">
        <span class="mp-alert-kicker">Requisito</span>
        <p class="mp-alert-title">Tesseract no detectado</p>
        <p>
          OCR necesita Tesseract en el sistema. Instálalo con packs
          <code>spa</code> y <code>eng</code>, luego reinicia MonkeyPDF.
          <a
            href="https://github.com/UB-Mannheim/tesseract/wiki"
            target="_blank"
            rel="noreferrer">Descargar Tesseract →</a
          >
        </p>
      </div>
    </div>
  {:else if available === true}
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
      Tesseract detectado. PDFs escaneados → texto local, sin subir nada.
    </p>
  {/if}

  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF escaneado" />

  <div class="mp-field">
    <span>Idioma</span>
    <div class="mp-hint-row" style="margin-top: 0">
      {#each langs as l}
        <button
          type="button"
          class="mp-chip"
          class:is-on={lang === l.id}
          onclick={() => (lang = l.id)}>{l.label}</button
        >
      {/each}
    </div>
  </div>

  <div class="mp-field">
    <span>Salida</span>
    <div class="mp-hint-row" style="margin-top: 0">
      {#each modes as m}
        <button
          type="button"
          class="mp-chip"
          class:is-on={mode === m.id}
          onclick={() => {
            mode = m.id
            output = ''
          }}>{m.label}</button
        >
      {/each}
    </div>
  </div>

  <OutputPicker
    bind:value={output}
    defaultName={modeMeta.name}
    label="Archivo de salida"
    filters={[{ name: modeMeta.label, extensions: [modeMeta.ext] }]}
  />

  {#if available === false}
    <div class="mp-alert is-danger" role="status">
      <div class="mp-alert-mark" aria-hidden="true">!</div>
      <div class="mp-alert-body">
        <span class="mp-alert-kicker">Bloqueado</span>
        <p class="mp-alert-title">No se puede reconocer texto aún</p>
        <p>Instala Tesseract y reinicia la app para activar el botón.</p>
      </div>
    </div>
  {/if}

  <button
    type="button"
    class="mp-btn mp-btn-primary"
    disabled={loading || available === false || available === null}
    onclick={run}
  >
    {#if available === null}
      Buscando Tesseract…
    {:else if available === false}
      Tesseract no detectado
    {:else}
      Reconocer texto
    {/if}
  </button>
</div>
