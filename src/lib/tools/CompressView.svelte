<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { compressPdf, fileName, type OpResult } from '../api'
  import { runWithProgress, type JobProgress } from '../jobProgress'
  import { loadToolPrefs, saveToolPrefs, resolveOutputDir } from '../settings'
  import { batchSiblingOutput, listPdfsInDir } from '../batch'

  let paths = $state<string[]>([])
  let output = $state('')
  let outputDir = $state('')
  let quality = $state(70)
  let mode = $state<'file' | 'folder'>('file')
  let folderPath = $state('')
  let batchFiles = $state<string[]>([])
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let progress = $state<JobProgress | null>(null)

  let prefsReady = $state(false)

  onMount(() => {
    void loadToolPrefs<{ quality: number; mode: 'file' | 'folder' }>('compress').then((p) => {
      if (typeof p.quality === 'number') quality = p.quality
      if (p.mode === 'file' || p.mode === 'folder') mode = p.mode
      prefsReady = true
    })
    const onRun = () => void run()
    const onOpen = () => void openPicker()
    window.addEventListener('mp-run', onRun)
    window.addEventListener('mp-open', onOpen)
    return () => {
      window.removeEventListener('mp-run', onRun)
      window.removeEventListener('mp-open', onOpen)
    }
  })

  $effect(() => {
    if (!prefsReady) return
    void saveToolPrefs('compress', { quality, mode })
  })

  async function openPicker() {
    if (mode === 'folder') {
      await pickFolder()
      return
    }
    // FileDropZone has its own picker; hotkey focuses via custom open not wired — noop
  }

  async function pickFolder() {
    const dir = await open({ directory: true, multiple: false })
    if (typeof dir !== 'string') return
    folderPath = dir
    try {
      batchFiles = await listPdfsInDir(dir)
      if (!outputDir) {
        outputDir = (await resolveOutputDir('compress')) || dir
      }
    } catch (e) {
      error = String(e)
      batchFiles = []
    }
  }

  async function run() {
    error = null
    result = null
    if (mode === 'folder') {
      await runBatch()
      return
    }
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
      result = await runWithProgress(
        (p) => (progress = p),
        () => compressPdf(paths[0], quality, output),
      )
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
      progress = null
    }
  }

  async function runBatch() {
    if (batchFiles.length === 0) {
      error = 'Selecciona una carpeta con PDFs'
      return
    }
    if (!outputDir) {
      error = 'Selecciona una carpeta de salida'
      return
    }
    loading = true
    const outputs: string[] = []
    const started = performance.now()
    let pages = 0
    try {
      await runWithProgress(
        (p) => (progress = p),
        async () => {
          for (let i = 0; i < batchFiles.length; i++) {
            const input = batchFiles[i]
            progress = {
              current: i + 1,
              total: batchFiles.length,
              label: `Comprimiendo ${fileName(input)} (${i + 1}/${batchFiles.length})`,
            }
            const out = batchSiblingOutput(input, outputDir, '_compressed')
            const r = await compressPdf(input, quality, out)
            outputs.push(...r.outputPaths)
            pages += r.pageCount
          }
        },
      )
      result = {
        outputPaths: outputs,
        pageCount: pages,
        elapsedMs: Math.round(performance.now() - started),
      }
    } catch (e) {
      const msg = String(e)
      if (outputs.length) {
        result = {
          outputPaths: outputs,
          pageCount: pages,
          elapsedMs: Math.round(performance.now() - started),
          partial: true,
          warnings: [msg],
        }
      } else {
        error = msg
      }
    } finally {
      loading = false
      progress = null
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner
    {loading}
    {error}
    {result}
    {progress}
    toolLabel="Comprimir PDF"
    toolId="compress"
    inputs={mode === 'folder' ? batchFiles : paths}
    cancellable={true}
  />

  <div class="mp-field">
    <span class="text-[var(--text-sm)] font-semibold">Modo</span>
    <div class="flex flex-wrap gap-2">
      <button type="button" class="mp-chip" class:is-on={mode === 'file'} onclick={() => (mode = 'file')}>
        Archivo
      </button>
      <button
        type="button"
        class="mp-chip"
        class:is-on={mode === 'folder'}
        onclick={() => (mode = 'folder')}
      >
        Carpeta (lote)
      </button>
    </div>
  </div>

  {#if mode === 'file'}
    <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para comprimir" />
  {:else}
    <div class="mp-field">
      <button type="button" class="mp-btn mp-btn-ghost" onclick={pickFolder}>Elegir carpeta…</button>
      {#if folderPath}
        <p class="mono text-[var(--text-xs)] text-[var(--color-ink-2)]">{folderPath}</p>
        <p class="text-[var(--text-xs)]">{batchFiles.length} PDF(s)</p>
      {/if}
    </div>
  {/if}

  <div class="mp-field">
    <label for="quality">Calidad JPEG: <span class="mono">{quality}</span></label>
    <input id="quality" class="mp-range" type="range" min="10" max="95" step="5" bind:value={quality} />
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
      Menor calidad = archivo más liviano. Recomprime imágenes y, si hace falta, rasteriza páginas a JPEG.
    </p>
  </div>

  {#if mode === 'file'}
    <OutputPicker bind:value={output} tool="compress" defaultName="compressed.pdf" label="PDF de salida" />
  {:else}
    <OutputPicker bind:value={outputDir} tool="compress" mode="directory" label="Carpeta de salida" />
  {/if}

  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    {mode === 'folder' ? 'Comprimir carpeta' : 'Comprimir PDF'}
  </button>
</div>
