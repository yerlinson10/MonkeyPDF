<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWebview } from '@tauri-apps/api/webview'
  import { open } from '@tauri-apps/plugin-dialog'
  import {
    fileName,
    isImagePath,
    isPdfPath,
    previewFile,
    type FilePreview,
  } from '../api'
  import Icon from './Icon.svelte'

  /* Hallmark · component: file-drop · genre: playful-técnico · theme: press-shop banana
   * states: default · hover · focus · active · disabled · loading · error · success
   * contrast: pass
   */

  interface Props {
    paths?: string[]
    accept?: string
    multiple?: boolean
    label?: string
    /** When false, skip the large preview stage (e.g. tool has its own editor). */
    showPreview?: boolean
  }

  let {
    paths = $bindable<string[]>([]),
    accept = '.pdf',
    multiple = true,
    label = 'Arrastra archivos aquí o elige desde disco',
    showPreview = true,
  }: Props = $props()

  let dragging = $state(false)
  let thumbs = $state<Record<string, string>>({})
  let pageCounts = $state<Record<string, number>>({})
  let loadingThumbs = $state<Record<string, boolean>>({})
  let activePath = $state<string | null>(null)
  let stage = $state<FilePreview | null>(null)
  let stageLoading = $state(false)
  let stageError = $state<string | null>(null)

  const singleFilled = $derived(!multiple && paths.length === 1)

  function acceptExtensions(): string[] {
    return accept
      .split(',')
      .map((s) => s.trim().toLowerCase().replace(/^\./, ''))
      .filter(Boolean)
  }

  function matchesAccept(path: string): boolean {
    const ext = path.split('.').pop()?.toLowerCase() ?? ''
    const allowed = acceptExtensions()
    return allowed.length === 0 || allowed.includes(ext)
  }

  function addPaths(incoming: string[]) {
    const filtered = incoming.filter(matchesAccept)
    if (filtered.length === 0) return
    if (multiple) {
      const set = new Set(paths)
      for (const p of filtered) set.add(p)
      paths = Array.from(set)
    } else {
      paths = filtered.slice(0, 1)
    }
    if (showPreview && filtered[0]) void selectPreview(filtered[filtered.length - 1])
  }

  async function pickFiles() {
    const selected = await open({
      multiple,
      filters: [{ name: 'Archivos', extensions: acceptExtensions() }],
    })
    if (!selected) return
    addPaths(Array.isArray(selected) ? selected : [selected])
  }

  function removeAt(index: number) {
    const removed = paths[index]
    paths = paths.filter((_, i) => i !== index)
    if (removed) {
      const { [removed]: _t, ...restThumbs } = thumbs
      const { [removed]: _p, ...restPages } = pageCounts
      thumbs = restThumbs
      pageCounts = restPages
      if (activePath === removed) {
        activePath = paths[0] ?? null
        if (activePath && showPreview) void selectPreview(activePath)
        else {
          stage = null
          stageError = null
        }
      }
    }
  }

  function move(index: number, dir: -1 | 1) {
    const target = index + dir
    if (target < 0 || target >= paths.length) return
    const next = [...paths]
    ;[next[index], next[target]] = [next[target], next[index]]
    paths = next
  }

  async function loadThumb(path: string) {
    if (thumbs[path] || loadingThumbs[path]) return
    if (!isPdfPath(path) && !isImagePath(path)) return
    loadingThumbs = { ...loadingThumbs, [path]: true }
    try {
      const preview = await previewFile(path, 1, 160)
      thumbs = { ...thumbs, [path]: preview.dataUrl }
      pageCounts = { ...pageCounts, [path]: preview.pageCount }
    } catch {
      // leave empty — row still usable
    } finally {
      const { [path]: _, ...rest } = loadingThumbs
      loadingThumbs = rest
    }
  }

  async function selectPreview(path: string, page = 1) {
    if (!showPreview) {
      activePath = path
      return
    }
    activePath = path
    stageLoading = true
    stageError = null
    try {
      const preview = await previewFile(path, page, 720)
      stage = preview
      pageCounts = { ...pageCounts, [path]: preview.pageCount }
      if (page === 1) thumbs = { ...thumbs, [path]: preview.dataUrl }
    } catch (e) {
      stage = null
      stageError = String(e)
    } finally {
      stageLoading = false
    }
  }

  async function stagePage(delta: -1 | 1) {
    if (!activePath || !stage || stage.kind !== 'pdf') return
    const next = stage.page + delta
    if (next < 1 || next > stage.pageCount) return
    await selectPreview(activePath, next)
  }

  $effect(() => {
    for (const path of paths) {
      void loadThumb(path)
    }
    if (paths.length === 1 && activePath !== paths[0]) {
      if (showPreview) void selectPreview(paths[0])
      else activePath = paths[0]
    }
    if (paths.length === 0) {
      activePath = null
      stage = null
      stageError = null
    }
  })

  onMount(() => {
    let unlisten: (() => void) | undefined
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === 'over') dragging = true
        else if (event.payload.type === 'leave') dragging = false
        else if (event.payload.type === 'drop') {
          dragging = false
          addPaths(event.payload.paths)
        }
      })
      .then((fn) => {
        unlisten = fn
      })
      .catch(() => {})

    return () => unlisten?.()
  })
</script>

<div class="space-y-3">
  {#if !singleFilled}
    <button
      type="button"
      onclick={pickFiles}
      class="mp-drop"
      class:is-dragging={dragging}
      class:is-single={!multiple}
    >
      <div
        class="mx-auto mb-3 grid h-11 w-11 place-items-center rounded-[var(--radius-md)] border border-[var(--color-rule)] bg-[var(--color-paper)] text-[var(--color-ink)]"
      >
        <Icon name="upload" size={20} />
      </div>
      <p class="text-[var(--text-sm)] font-semibold text-[var(--color-ink)]">{label}</p>
      <p class="mono mt-1 text-[var(--text-xs)] text-[var(--color-ink-2)]">
        {#if multiple}
          {accept.replaceAll('.', '').toUpperCase()} · varios archivos
        {:else}
          {accept.replaceAll('.', '').toUpperCase()} · un solo archivo
        {/if}
      </p>
    </button>
  {/if}

  {#if paths.length > 0}
    <ul class="space-y-2">
      {#each paths as path, index (path)}
        <li
          class="mp-file-row"
          class:is-preview-active={showPreview && activePath === path}
          class:is-single-chip={singleFilled}
        >
          <button
            type="button"
            class="mp-file-main"
            onclick={() => (showPreview ? selectPreview(path) : undefined)}
            disabled={!showPreview}
          >
            <span class="mp-thumb" aria-hidden="true">
              {#if thumbs[path]}
                <img src={thumbs[path]} alt="" />
              {:else if loadingThumbs[path]}
                <span class="mp-thumb-loading"></span>
              {:else}
                <Icon name={isPdfPath(path) ? 'jpg-to-pdf' : 'pdf-to-jpg'} size={18} />
              {/if}
            </span>
            <span class="min-w-0 flex-1 text-left">
              <span class="mono selectable block truncate text-[var(--text-sm)]" title={path}
                >{fileName(path)}</span
              >
              {#if pageCounts[path]}
                <span class="mt-0.5 block text-[var(--text-xs)] font-semibold text-[var(--color-ink-2)]">
                  {pageCounts[path]} {pageCounts[path] === 1 ? 'página' : 'páginas'}
                </span>
              {/if}
            </span>
          </button>
          {#if singleFilled}
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-9 !px-3"
              onclick={pickFiles}
            >
              Cambiar
            </button>
          {/if}
          {#if multiple && paths.length > 1}
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-9 !px-2"
              onclick={() => move(index, -1)}
              aria-label="Subir"
            >
              <Icon name="arrow-up" size={16} />
            </button>
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-9 !px-2"
              onclick={() => move(index, 1)}
              aria-label="Bajar"
            >
              <Icon name="arrow-down" size={16} />
            </button>
          {/if}
          <button
            type="button"
            class="mp-btn mp-btn-ghost !min-h-9 !px-2 text-[var(--color-danger)]"
            onclick={() => removeAt(index)}
            aria-label="Quitar"
          >
            <Icon name="x" size={16} />
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if showPreview && activePath}
    <div class="mp-preview-stage">
      <div class="mp-preview-meta">
        <span class="mono selectable truncate" title={activePath}>{fileName(activePath)}</span>
        {#if stage && stage.kind === 'pdf' && stage.pageCount > 1}
          <div class="mp-preview-pager">
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              disabled={stageLoading || stage.page <= 1}
              onclick={() => stagePage(-1)}
              aria-label="Página anterior"
            >
              <Icon name="arrow-up" size={14} />
            </button>
            <span class="mono text-[var(--text-xs)]"
              >{stage.page} / {stage.pageCount}</span
            >
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              disabled={stageLoading || stage.page >= stage.pageCount}
              onclick={() => stagePage(1)}
              aria-label="Página siguiente"
            >
              <Icon name="arrow-down" size={14} />
            </button>
          </div>
        {/if}
      </div>

      <div class="mp-preview-frame">
        {#if stageLoading && !stage}
          <p class="text-[var(--text-sm)] text-[var(--color-ink-2)]">Renderizando vista…</p>
        {:else if stageError}
          <p class="text-[var(--text-sm)] text-[var(--color-danger)]">{stageError}</p>
        {:else if stage}
          <img
            class="mp-preview-img"
            class:is-loading={stageLoading}
            src={stage.dataUrl}
            alt="Vista previa de {fileName(activePath)}"
          />
        {/if}
      </div>
    </div>
  {/if}
</div>
