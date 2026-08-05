<script lang="ts">
  import { onMount } from 'svelte'
  import { TOOLS, type ToolId, type ToolMeta } from './lib/api'
  import { initNotifications } from './lib/notify'
  import Icon from './lib/components/Icon.svelte'
  import MergeView from './lib/tools/MergeView.svelte'
  import SplitView from './lib/tools/SplitView.svelte'
  import RotateView from './lib/tools/RotateView.svelte'
  import CompressView from './lib/tools/CompressView.svelte'
  import PdfToJpgView from './lib/tools/PdfToJpgView.svelte'
  import JpgToPdfView from './lib/tools/JpgToPdfView.svelte'

  let activeTool = $state<ToolId | null>(null)

  const activeMeta = $derived(TOOLS.find((t) => t.id === activeTool) ?? null)

  function selectTool(tool: ToolMeta) {
    activeTool = tool.id
  }

  function toolIndex(id: ToolId): string {
    const i = TOOLS.findIndex((t) => t.id === id)
    return String(i + 1).padStart(2, '0')
  }

  onMount(() => {
    void initNotifications()
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
      <span class="mp-stamp-tag">Taller local</span>
    </div>

    <nav class="mp-tool-list" aria-label="Lista de herramientas">
      {#each TOOLS as tool (tool.id)}
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
      {/each}
    </nav>
  </aside>

  <section class="mp-canvas">
    <div class="mp-sheet">
      {#if !activeTool}
        <header class="mp-canvas-head">
          <div>
            <span class="kicker">Hoja de trabajo</span>
            <h1>Escoge el sello</h1>
            <p>Seis herramientas. Un clic. El PDF no sale de tu máquina.</p>
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
                Unir, cortar, girar, aplastar peso o pasar a imagen. Menú a la izquierda — resultado en
                la hoja.
              </p>
              <div class="mp-hint-row">
                <span class="mp-hint">01–06 tools</span>
                <span class="mp-hint">0 uploads</span>
                <span class="mp-hint">banana stamp</span>
              </div>
            </div>
          </div>
        </div>
      {:else}
        <header class="mp-canvas-head">
          <div>
            <span class="kicker">Tool {toolIndex(activeTool)}</span>
            <h1>{activeMeta?.title}</h1>
            <p>{activeMeta?.description}</p>
          </div>
          <button type="button" class="mp-btn mp-btn-ghost" onclick={() => (activeTool = null)}>
            Cerrar
          </button>
        </header>
        <div class="mp-canvas-body">
          <div class="mp-panel">
            {#if activeTool === 'merge'}
              <MergeView />
            {:else if activeTool === 'split'}
              <SplitView />
            {:else if activeTool === 'rotate'}
              <RotateView />
            {:else if activeTool === 'compress'}
              <CompressView />
            {:else if activeTool === 'pdf-to-jpg'}
              <PdfToJpgView />
            {:else if activeTool === 'jpg-to-pdf'}
              <JpgToPdfView />
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </section>
</div>
