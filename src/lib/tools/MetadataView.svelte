<script lang="ts">
  import { onMount } from 'svelte'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import DatePicker from '../components/DatePicker.svelte'
  import {
    getPdfMetadata,
    setPdfMetadata,
    type OpResult,
    type PdfMetadata,
  } from '../api'
  import { isoToPdfDate, pdfDateToIso } from '../pdfDate'

  const emptyMeta = (): PdfMetadata => ({
    title: '',
    author: '',
    subject: '',
    keywords: '',
    creator: '',
    producer: '',
    creationDate: '',
    modDate: '',
    pageCount: 0,
  })

  let paths = $state<string[]>([])
  let output = $state('')
  let meta = $state<PdfMetadata>(emptyMeta())
  let creationIso = $state('')
  let modIso = $state('')
  let loading = $state(false)
  let reading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  const path = $derived(paths[0] ?? '')

  onMount(() => {
    const onRun = () => void run()
    window.addEventListener('mp-run', onRun)
    return () => window.removeEventListener('mp-run', onRun)
  })

  $effect(() => {
    if (path) {
      void loadMeta(path)
    } else {
      meta = emptyMeta()
      creationIso = ''
      modIso = ''
    }
  })

  async function loadMeta(p: string) {
    reading = true
    error = null
    try {
      meta = await getPdfMetadata(p)
      creationIso = pdfDateToIso(meta.creationDate)
      modIso = pdfDateToIso(meta.modDate)
    } catch (e) {
      error = String(e)
      meta = emptyMeta()
      creationIso = ''
      modIso = ''
    } finally {
      reading = false
    }
  }

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
      const payload: PdfMetadata = {
        ...meta,
        creationDate: isoToPdfDate(creationIso, meta.creationDate),
        modDate: isoToPdfDate(modIso, meta.modDate),
      }
      result = await setPdfMetadata(paths[0], output, payload)
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
    toolLabel="Metadatos"
    toolId="metadata"
    inputs={paths}
  />

  <FileDropZone
    bind:paths
    accept=".pdf"
    multiple={false}
    label="Arrastra un PDF para ver / editar metadatos"
  />

  {#if path}
    <div class="mp-field">
      <span class="text-[var(--text-sm)] font-semibold">
        Info del documento
        {#if meta.pageCount}
          <span class="mono text-[var(--color-ink-2)]">· {meta.pageCount} pág.</span>
        {/if}
        {#if reading}
          <span class="text-[var(--color-ink-2)]">· leyendo…</span>
        {/if}
      </span>
    </div>

    <div class="meta-grid">
      <label class="mp-field">
        <span>Título</span>
        <input class="mp-input" bind:value={meta.title} />
      </label>
      <label class="mp-field">
        <span>Autor</span>
        <input class="mp-input" bind:value={meta.author} />
      </label>
      <label class="mp-field">
        <span>Asunto</span>
        <input class="mp-input" bind:value={meta.subject} />
      </label>
      <label class="mp-field">
        <span>Palabras clave</span>
        <input class="mp-input" bind:value={meta.keywords} placeholder="separadas por coma" />
      </label>
      <label class="mp-field">
        <span>Creador</span>
        <input class="mp-input" bind:value={meta.creator} />
      </label>
      <label class="mp-field">
        <span>Productor</span>
        <input class="mp-input" bind:value={meta.producer} />
      </label>
      <DatePicker label="Fecha creación" bind:value={creationIso} placeholder="Sin fecha" />
      <DatePicker label="Fecha modificación" bind:value={modIso} placeholder="Sin fecha" />
    </div>
  {/if}

  <OutputPicker bind:value={output} tool="metadata" defaultName="metadata.pdf" label="PDF de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading || !path} onclick={run}>
    Guardar metadatos
  </button>
</div>

<style>
  .meta-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 0.85rem 1rem;
  }
</style>
