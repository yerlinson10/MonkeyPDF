<script lang="ts">
  import { onMount } from 'svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import {
    AI_PROVIDERS,
    loadAiSettings,
    saveAiSettings,
    type AiSettings,
  } from '../settings'

  let settings = $state<AiSettings | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let saved = $state(false)

  onMount(() => {
    loadAiSettings().then((s) => (settings = s))
  })

  async function save() {
    if (!settings) return
    error = null
    saved = false
    loading = true
    try {
      await saveAiSettings(settings)
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
    result={saved
      ? { outputPaths: ['Ajustes guardados en disco local'], pageCount: 0, elapsedMs: 0 }
      : null}
    toolLabel="Ajustes"
  />

  {#if settings}
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

      <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={save}>
        Guardar IA
      </button>
    </section>
  {:else}
    <p>Cargando…</p>
  {/if}
</div>
