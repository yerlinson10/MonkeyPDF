<script lang="ts">
  import { onMount } from 'svelte'
  import { RAIL_TOOLS, TOOLS, revealInExplorer, type ToolId, type ToolMeta } from './lib/api'
  import { initNotifications } from './lib/notify'
  import { clearHistory, fileName, loadHistory, type HistoryEntry } from './lib/history'
  import { dispatchToolOpen, dispatchToolRun } from './lib/jobProgress'
  import Icon from './lib/components/Icon.svelte'
  import MergeView from './lib/tools/MergeView.svelte'
  import SplitView from './lib/tools/SplitView.svelte'
  import OrganizeView from './lib/tools/OrganizeView.svelte'
  import RotateView from './lib/tools/RotateView.svelte'
  import CompressView from './lib/tools/CompressView.svelte'
  import PdfToJpgView from './lib/tools/PdfToJpgView.svelte'
  import JpgToPdfView from './lib/tools/JpgToPdfView.svelte'
  import ExtractView from './lib/tools/ExtractView.svelte'
  import ProtectView from './lib/tools/ProtectView.svelte'
  import RepairView from './lib/tools/RepairView.svelte'
  import MetadataView from './lib/tools/MetadataView.svelte'
  import PageNumbersView from './lib/tools/PageNumbersView.svelte'
  import OfficeView from './lib/tools/OfficeView.svelte'
  import PdfaView from './lib/tools/PdfaView.svelte'
  import OcrView from './lib/tools/OcrView.svelte'
  import RedactView from './lib/tools/RedactView.svelte'
  import CropView from './lib/tools/CropView.svelte'
  import WatermarkView from './lib/tools/WatermarkView.svelte'
  import CompareView from './lib/tools/CompareView.svelte'
  import SignView from './lib/tools/SignView.svelte'
  import MarkdownView from './lib/tools/MarkdownView.svelte'
  import AiView from './lib/tools/AiView.svelte'
  import SettingsView from './lib/tools/SettingsView.svelte'
  import AppContextMenu from './lib/components/AppContextMenu.svelte'

  let activeTool = $state<ToolId | null>(null)
  let toolQuery = $state('')
  let history = $state<HistoryEntry[]>([])
  let searchEl = $state<HTMLInputElement | null>(null)

  const activeMeta = $derived(TOOLS.find((t) => t.id === activeTool) ?? null)

  const filteredTools = $derived.by(() => {
    const q = toolQuery.trim().toLowerCase()
    if (!q) return RAIL_TOOLS
    return RAIL_TOOLS.filter((t) => {
      const hay = `${t.title} ${t.short} ${t.description} ${t.id}`.toLowerCase()
      return hay.includes(q) || q.split(/\s+/).every((part) => hay.includes(part))
    })
  })

  function selectTool(tool: ToolMeta) {
    activeTool = tool.id
  }

  function openSettings() {
    activeTool = 'settings'
  }

  function toolIndex(id: ToolId): string {
    const i = RAIL_TOOLS.findIndex((t) => t.id === id)
    if (i < 0) return '··'
    return String(i + 1).padStart(2, '0')
  }

  async function refreshHistory() {
    history = await loadHistory()
  }

  async function openHistoryOut(entry: HistoryEntry) {
    const path = entry.outputs[0]
    if (!path) return
    try {
      await revealInExplorer(path)
    } catch (e) {
      console.warn(e)
    }
  }

  function repeatHistory(entry: HistoryEntry) {
    activeTool = entry.toolId
  }

  function isTypingTarget(el: EventTarget | null): boolean {
    if (!(el instanceof HTMLElement)) return false
    const tag = el.tagName
    return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable
  }

  onMount(() => {
    void initNotifications()
    void refreshHistory()
    const onHist = () => void refreshHistory()
    window.addEventListener('mp-history', onHist)

    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey
      if (mod && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        searchEl?.focus()
        searchEl?.select()
        return
      }
      if (e.key === 'Escape') {
        if (toolQuery) {
          toolQuery = ''
          return
        }
        if (activeTool) {
          activeTool = null
        }
        return
      }
      if (mod && e.key === 'Enter' && activeTool && activeTool !== 'settings') {
        e.preventDefault()
        dispatchToolRun()
        return
      }
      if (mod && e.key.toLowerCase() === 'o' && activeTool && activeTool !== 'settings') {
        e.preventDefault()
        dispatchToolOpen()
        return
      }
      if (!mod && !isTypingTarget(e.target) && /^[1-9]$/.test(e.key)) {
        const idx = Number(e.key) - 1
        const tool = RAIL_TOOLS[idx]
        if (tool) {
          e.preventDefault()
          activeTool = tool.id
        }
      }
    }
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mp-history', onHist)
      window.removeEventListener('keydown', onKey)
    }
  })
</script>

<div class="mp-shell">
  <aside class="mp-rail" aria-label="Herramientas">
    <div class="mp-brand">
      <div class="mp-brand-row">
        <div class="mp-mark" aria-hidden="true">
          <img src="/favicon.svg" alt="" width="36" height="36" />
        </div>
        <div class="mp-wordmark" aria-label="MonkeyPDF">
          <span class="w1">Monkey</span>
          <span class="w2">PDF</span>
        </div>
      </div>
      <label class="mp-rail-search">
        <span class="sr-only">Buscar herramientas</span>
        <input
          bind:this={searchEl}
          type="search"
          class="mp-rail-search-input"
          placeholder="Buscar… Ctrl+K"
          bind:value={toolQuery}
          autocomplete="off"
          spellcheck="false"
        />
        {#if toolQuery}
          <button
            type="button"
            class="mp-rail-search-clear"
            aria-label="Limpiar búsqueda"
            onclick={() => (toolQuery = '')}
          >
            <Icon name="x" size={14} />
          </button>
        {/if}
      </label>
    </div>

    <nav class="mp-tool-list" aria-label="Lista de herramientas">
      {#each filteredTools as tool (tool.id)}
        <button
          type="button"
          class="mp-tool-btn"
          class:is-active={activeTool === tool.id}
          aria-current={activeTool === tool.id ? 'page' : undefined}
          onclick={() => selectTool(tool)}
        >
          <span class="mp-tool-num">{toolIndex(tool.id)}</span>
          <span class="icon-wrap">
            <Icon name={tool.id} size={16} />
          </span>
          <span class="label">
            <strong>{tool.title}</strong>
            <small>{tool.short}</small>
          </span>
        </button>
      {:else}
        <p class="mp-rail-empty">Sin resultados para “{toolQuery.trim()}”</p>
      {/each}
    </nav>

    <div class="mp-rail-foot">
      <button
        type="button"
        class="mp-rail-settings"
        class:is-active={activeTool === 'settings'}
        aria-label="Ajustes"
        aria-current={activeTool === 'settings' ? 'page' : undefined}
        title="Ajustes"
        onclick={openSettings}
      >
        <Icon name="settings" size={18} />
      </button>
    </div>
  </aside>

  <section class="mp-canvas">
    <div class="mp-sheet">
      {#if !activeTool}
        <header class="mp-canvas-head">
          <div>
            <span class="kicker">Hoja de trabajo</span>
            <h1>Escoge el sello</h1>
            <p>Veintidós herramientas. Un clic. El PDF no sale de tu máquina.</p>
          </div>
        </header>
        <div class="mp-canvas-body">
          <div class="mp-empty">
            <div class="mp-empty-card">
              <div class="mp-hero-mark">
                <div class="badge" aria-hidden="true">
                  <img src="/favicon.svg" alt="" width="44" height="44" />
                </div>
              </div>
              <div class="giant">Hazlo<br /><em>local.</em></div>
              <p>
                Núcleo, suite, firmas, marcas de agua, OCR/censura y IA. Menú a la izquierda —
                resultado en la hoja.
              </p>
              <div class="mp-hint-row">
                <span class="mp-hint">01–22 tools</span>
                <span class="mp-hint">Ctrl+K busca</span>
                <span class="mp-hint">Esc cierra</span>
              </div>
            </div>

            {#if history.length}
              <div class="mp-recent">
                <div class="mp-recent-head">
                  <span class="kicker">Recientes</span>
                  <button
                    type="button"
                    class="mp-btn mp-btn-ghost !min-h-8 !text-xs"
                    onclick={() => void clearHistory().then(refreshHistory)}
                  >
                    Limpiar
                  </button>
                </div>
                <ul class="mp-recent-list">
                  {#each history.slice(0, 8) as entry (entry.id)}
                    <li class="mp-recent-item">
                      <button type="button" class="mp-recent-main" onclick={() => repeatHistory(entry)}>
                        <span class="mp-recent-tool">{entry.toolLabel}</span>
                        <span class="mp-recent-file mono">{fileName(entry.outputs[0] ?? '')}</span>
                      </button>
                      <button
                        type="button"
                        class="mp-btn mp-btn-ghost !min-h-8 !px-2"
                        title="Abrir en explorador"
                        onclick={() => openHistoryOut(entry)}
                      >
                        <Icon name="folder" size={14} />
                      </button>
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <header class="mp-canvas-head">
          <div>
            <span class="kicker">
              {activeTool === 'settings' ? 'Configuración' : `Tool ${toolIndex(activeTool)}`}
            </span>
            <h1>{activeMeta?.title}</h1>
            <p>{activeMeta?.description}</p>
          </div>
          <button type="button" class="mp-btn mp-btn-ghost" onclick={() => (activeTool = null)}>
            Cerrar
          </button>
        </header>
        <div class="mp-canvas-body">
          <div
            class="mp-panel"
            class:is-wide={activeTool === 'compare' ||
              activeTool === 'redact' ||
              activeTool === 'crop' ||
              activeTool === 'sign' ||
              activeTool === 'watermark' ||
              activeTool === 'organize'}
          >
            {#if activeTool === 'merge'}
              <MergeView />
            {:else if activeTool === 'split'}
              <SplitView />
            {:else if activeTool === 'organize'}
              <OrganizeView />
            {:else if activeTool === 'rotate'}
              <RotateView />
            {:else if activeTool === 'compress'}
              <CompressView />
            {:else if activeTool === 'pdf-to-jpg'}
              <PdfToJpgView />
            {:else if activeTool === 'jpg-to-pdf'}
              <JpgToPdfView />
            {:else if activeTool === 'extract'}
              <ExtractView />
            {:else if activeTool === 'protect'}
              <ProtectView />
            {:else if activeTool === 'repair'}
              <RepairView />
            {:else if activeTool === 'metadata'}
              <MetadataView />
            {:else if activeTool === 'page-numbers'}
              <PageNumbersView />
            {:else if activeTool === 'office'}
              <OfficeView />
            {:else if activeTool === 'pdfa'}
              <PdfaView />
            {:else if activeTool === 'ocr'}
              <OcrView />
            {:else if activeTool === 'redact'}
              <RedactView />
            {:else if activeTool === 'crop'}
              <CropView />
            {:else if activeTool === 'watermark'}
              <WatermarkView />
            {:else if activeTool === 'compare'}
              <CompareView />
            {:else if activeTool === 'sign'}
              <SignView />
            {:else if activeTool === 'markdown'}
              <MarkdownView />
            {:else if activeTool === 'ai'}
              <AiView />
            {:else if activeTool === 'settings'}
              <SettingsView />
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </section>
</div>

<AppContextMenu />
