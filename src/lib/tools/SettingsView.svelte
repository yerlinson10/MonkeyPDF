<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import ResultBanner from '../components/ResultBanner.svelte'
  import Icon from '../components/Icon.svelte'
  import { RAIL_TOOLS } from '../api'
  import {
    AI_PROVIDERS,
    loadAiSettings,
    saveAiSettings,
    loadOutputPathSettings,
    saveOutputPathSettings,
    type AiSettings,
    type OutputPathSettings,
    type OutputToolId,
  } from '../settings'

  let settings = $state<AiSettings | null>(null)
  let paths = $state<OutputPathSettings | null>(null)
  let advancedOpen = $state(false)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let saved = $state(false)
  let savedLabel = $state('Ajustes guardados en disco local')

  onMount(() => {
    void Promise.all([loadAiSettings(), loadOutputPathSettings()]).then(([ai, out]) => {
      settings = ai
      paths = out
    })
  })

  async function pickDir(current: string): Promise<string | null> {
    const dir = await open({
      directory: true,
      multiple: false,
      defaultPath: current.trim() || undefined,
    })
    return typeof dir === 'string' ? dir : null
  }

  async function pickDefaultDir() {
    if (!paths) return
    const dir = await pickDir(paths.defaultDir)
    if (dir) paths.defaultDir = dir
  }

  async function pickToolDir(id: OutputToolId) {
    if (!paths) return
    const dir = await pickDir(paths.toolDirs[id] ?? paths.defaultDir)
    if (dir) paths.toolDirs[id] = dir
  }

  function clearToolDir(id: OutputToolId) {
    if (!paths) return
    const next = { ...paths.toolDirs }
    delete next[id]
    paths.toolDirs = next
  }

  async function saveAi() {
    if (!settings) return
    error = null
    saved = false
    loading = true
    try {
      await saveAiSettings(settings)
      savedLabel = 'Ajustes de IA guardados en disco local'
      saved = true
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  async function savePaths() {
    if (!paths) return
    error = null
    saved = false
    loading = true
    try {
      await saveOutputPathSettings(paths)
      savedLabel = 'Rutas de salida guardadas en disco local'
      saved = true
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
    result={saved ? { outputPaths: [savedLabel], pageCount: 0, elapsedMs: 0 } : null}
    toolLabel="Ajustes"
  />

  {#if settings && paths}
    <section class="mp-settings-section" aria-labelledby="settings-paths-title">
      <header class="mp-settings-section-head">
        <h2 id="settings-paths-title">Rutas de salida</h2>
        <p>
          Carpeta por defecto donde se guardan los resultados. En Avanzado puedes fijar una ruta
          distinta por herramienta.
        </p>
      </header>

      <div class="mp-field">
        <label for="default-out">Carpeta por defecto</label>
        <div class="flex gap-2">
          <input
            id="default-out"
            class="mp-input mono"
            bind:value={paths.defaultDir}
            placeholder="Sin ruta — eliges al exportar"
            readonly
          />
          <button type="button" class="mp-btn mp-btn-ghost shrink-0" onclick={pickDefaultDir}>
            <Icon name="folder" size={16} />
            Examinar
          </button>
          {#if paths.defaultDir}
            <button
              type="button"
              class="mp-btn mp-btn-ghost shrink-0"
              onclick={() => paths && (paths.defaultDir = '')}
              title="Quitar"
            >
              Limpiar
            </button>
          {/if}
        </div>
      </div>

      <div class="mp-settings-advanced">
        <button
          type="button"
          class="mp-settings-advanced-toggle"
          aria-expanded={advancedOpen}
          onclick={() => (advancedOpen = !advancedOpen)}
        >
          <span>Avanzado</span>
          <span class="mp-settings-advanced-hint">Ruta por función</span>
          <Icon name={advancedOpen ? 'arrow-up' : 'arrow-down'} size={16} />
        </button>

        {#if advancedOpen}
          <div class="mp-settings-advanced-body">
            <p class="mp-settings-advanced-note">
              Vacío = usa la carpeta por defecto. Solo las herramientas con ruta propia la
              sobrescriben.
            </p>
            {#each RAIL_TOOLS as tool}
              {@const toolId = tool.id as OutputToolId}
              <div class="mp-settings-path-row">
                <label for={`tool-out-${toolId}`}>{tool.title}</label>
                <div class="flex gap-2">
                  <input
                    id={`tool-out-${toolId}`}
                    class="mp-input mono"
                    value={paths.toolDirs[toolId] ?? ''}
                    placeholder="Usar carpeta por defecto"
                    readonly
                  />
                  <button
                    type="button"
                    class="mp-btn mp-btn-ghost shrink-0"
                    onclick={() => pickToolDir(toolId)}
                  >
                    <Icon name="folder" size={16} />
                    Examinar
                  </button>
                  {#if paths.toolDirs[toolId]}
                    <button
                      type="button"
                      class="mp-btn mp-btn-ghost shrink-0"
                      onclick={() => clearToolDir(toolId)}
                      title="Usar carpeta por defecto"
                    >
                      Limpiar
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={savePaths}>
        Guardar rutas
      </button>
    </section>

    <section class="mp-settings-section" aria-labelledby="settings-ai-title">
      <header class="mp-settings-section-head">
        <h2 id="settings-ai-title">Inteligencia artificial</h2>
        <p>Claves y modelos para resumir o traducir. Se guardan solo en este equipo.</p>
      </header>

      <div class="mp-field">
        <span>Proveedor</span>
        <div class="mp-hint-row" style="margin-top: 0">
          {#each AI_PROVIDERS as p}
            <button
              type="button"
              class="mp-chip"
              class:is-on={settings.provider === p.id}
              onclick={() => settings && (settings.provider = p.id)}>{p.label}</button
            >
          {/each}
        </div>
      </div>

      {#if settings.provider === 'openai'}
        <div class="mp-field">
          <label for="oai">API key</label>
          <input
            id="oai"
            class="mp-input"
            type="password"
            autocomplete="off"
            bind:value={settings.openaiKey}
            placeholder="sk-…"
          />
        </div>
        <div class="mp-field">
          <label for="oai-m">Modelo</label>
          <input id="oai-m" class="mp-input" bind:value={settings.openaiModel} />
        </div>
      {:else if settings.provider === 'anthropic'}
        <div class="mp-field">
          <label for="ant">API key</label>
          <input
            id="ant"
            class="mp-input"
            type="password"
            autocomplete="off"
            bind:value={settings.anthropicKey}
            placeholder="sk-ant-…"
          />
        </div>
        <div class="mp-field">
          <label for="ant-m">Modelo</label>
          <input id="ant-m" class="mp-input" bind:value={settings.anthropicModel} />
        </div>
      {:else if settings.provider === 'openrouter'}
        <div class="mp-field">
          <label for="or">API key</label>
          <input
            id="or"
            class="mp-input"
            type="password"
            autocomplete="off"
            bind:value={settings.openrouterKey}
            placeholder="sk-or-…"
          />
        </div>
        <div class="mp-field">
          <label for="or-m">Modelo</label>
          <input
            id="or-m"
            class="mp-input"
            bind:value={settings.openrouterModel}
            placeholder="openai/gpt-4o-mini"
          />
        </div>
        <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
          Usa el id de modelo de openrouter.ai (ej. <span class="mono">anthropic/claude-3.5-sonnet</span>).
        </p>
      {:else if settings.provider === 'ollama'}
        <div class="mp-field">
          <label for="ollama">Base URL</label>
          <input id="ollama" class="mp-input" bind:value={settings.ollamaBaseUrl} />
        </div>
        <div class="mp-field">
          <label for="ollama-m">Modelo</label>
          <input id="ollama-m" class="mp-input" bind:value={settings.ollamaModel} />
        </div>
        <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
          Local — no necesita API key. Ollama debe estar en marcha.
        </p>
      {/if}

      <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={saveAi}>
        Guardar IA
      </button>
    </section>
  {:else}
    <p>Cargando…</p>
  {/if}
</div>
