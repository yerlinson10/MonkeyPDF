<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { fileName, rotatePdf, type OpResult } from '../api'
  import { runWithProgress, type JobProgress } from '../jobProgress'
  import { loadToolPrefs, saveToolPrefs, resolveOutputDir } from '../settings'
  import { batchSiblingOutput, listPdfsInDir } from '../batch'

  let paths = $state<string[]>([])
  let output = $state('')
  let outputDir = $state('')
  let angle = $state(90)
  let pagesText = $state('')
  let mode = $state<'file' | 'folder'>('file')
  let folderPath = $state('')
  let batchFiles = $state<string[]>([])
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let progress = $state<JobProgress | null>(null)
  let prefsReady = $state(false)

  onMount(() => {
    void loadToolPrefs<{ angle: number; pagesText: string; mode: 'file' | 'folder' }>('rotate').then(
      (p) => {
        if (typeof p.angle === 'number') angle = p.angle
        if (typeof p.pagesText === 'string') pagesText = p.pagesText
        if (p.mode === 'file' || p.mode === 'folder') mode = p.mode
        prefsReady = true
      },
    )
    const onRun = () => void run()
    window.addEventListener('mp-run', onRun)
    return () => window.removeEventListener('mp-run', onRun)
  })

  $effect(() => {
    if (!prefsReady) return
    void saveToolPrefs('rotate', { angle, pagesText, mode })
  })

  function parsePages(): number[] | null {
    if (!pagesText.trim()) return null
    return pagesText
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
      .map((s) => {
        const n = Number(s)
        if (!Number.isInteger(n) || n < 1) throw new Error(`Página inválida: ${s}`)
        return n
      })
  }

  async function pickFolder() {
    const dir = await open({ directory: true, multiple: false })
    if (typeof dir !== 'string') return
    folderPath = dir
    try {
      batchFiles = await listPdfsInDir(dir)
      if (!outputDir) {
        outputDir = (await resolveOutputDir('rotate')) || dir
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

    let pages: number[] | null = null
    try {
      pages = parsePages()
    } catch (e) {
      error = String(e)
      return
    }

    loading = true
    try {
      result = await runWithProgress(
        (p) => (progress = p),
        () => rotatePdf(paths[0], angle, pages, output),
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
    let pages: number[] | null = null
    try {
      pages = parsePages()
    } catch (e) {
      error = String(e)
      return
    }

    loading = true
    const outputs: string[] = []
    const started = performance.now()
    let pageCount = 0
    try {
      await runWithProgress(
        (p) => (progress = p),
        async () => {
          for (let i = 0; i < batchFiles.length; i++) {
            const input = batchFiles[i]
            progress = {
              current: i + 1,
              total: batchFiles.length,
              label: `Rotando ${fileName(input)} (${i + 1}/${batchFiles.length})`,
            }
            const out = batchSiblingOutput(input, outputDir, `_rot${angle}`)
            const r = await rotatePdf(input, angle, pages, out)
            outputs.push(...r.outputPaths)
            pageCount += r.pageCount
          }
        },
      )
      result = {
        outputPaths: outputs,
        pageCount,
        elapsedMs: Math.round(performance.now() - started),
      }
    } catch (e) {
      const msg = String(e)
      if (outputs.length) {
        result = {
          outputPaths: outputs,
          pageCount,
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
    toolLabel="Rotar PDF"
    toolId="rotate"
    inputs={mode === 'folder' ? batchFiles : paths}
    cancellable={mode === 'folder'}
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
    <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF para rotar" />
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

  {#if mode === 'file'}
    <OutputPicker bind:value={output} tool="rotate" defaultName="rotated.pdf" label="PDF de salida" />
  {:else}
    <OutputPicker bind:value={outputDir} tool="rotate" mode="directory" label="Carpeta de salida" />
  {/if}

  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    {mode === 'folder' ? 'Rotar carpeta' : 'Rotar PDF'}
  </button>
</div>
