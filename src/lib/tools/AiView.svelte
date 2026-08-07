<script lang="ts">
  import { onMount } from 'svelte'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { aiProcessPdf, writeTextFile, type OpResult } from '../api'
  import {
    apiKeyFor,
    baseUrlFor,
    loadAiSettings,
    modelFor,
    type AiSettings,
  } from '../settings'

  let paths = $state<string[]>([])
  let output = $state('')
  let action = $state<'summarize' | 'translate'>('summarize')
  let targetLang = $state('español')
  let settings = $state<AiSettings | null>(null)
  let preview = $state('')
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  onMount(() => {
    loadAiSettings().then((s) => (settings = s))
  })

  async function run() {
    error = null
    result = null
    preview = ''
    if (!paths[0]) {
      error = 'Selecciona un PDF'
      return
    }
    if (!settings) {
      error = 'Cargando ajustes…'
      return
    }
    const key = apiKeyFor(settings)
    if (settings.provider !== 'ollama' && !key) {
      error = 'Configura tu API key en Ajustes (icono del menú)'
      return
    }
    loading = true
    try {
      const ai = await aiProcessPdf({
        path: paths[0],
        action,
        provider: settings.provider,
        apiKey: key,
        model: modelFor(settings),
        targetLang: action === 'translate' ? targetLang : null,
        baseUrl: baseUrlFor(settings),
      })
      preview = ai.text
      if (output) {
        result = await writeTextFile(output, ai.text)
      } else {
        result = {
          outputPaths: [],
          pageCount: 1,
          elapsedMs: ai.elapsedMs,
        }
      }
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="IA" />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF con texto" />

  {#if settings}
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">
      Proveedor: <strong>{settings.provider}</strong> · modelo <span class="mono">{modelFor(settings)}</span>
      — cámbialo en Ajustes.
    </p>
  {/if}

  <div class="mp-field">
    <span>Acción</span>
    <div class="mp-hint-row" style="margin-top: 0">
      <button
        type="button"
        class="mp-chip"
        class:is-on={action === 'summarize'}
        onclick={() => (action = 'summarize')}>Resumir</button
      >
      <button
        type="button"
        class="mp-chip"
        class:is-on={action === 'translate'}
        onclick={() => (action = 'translate')}>Traducir</button
      >
    </div>
  </div>

  {#if action === 'translate'}
    <div class="mp-field">
      <label for="lang">Idioma destino</label>
      <input id="lang" class="mp-input" bind:value={targetLang} />
    </div>
  {/if}

  <OutputPicker
    bind:value={output}
    tool="ai"
    autofill={false}
    defaultName={action === 'summarize' ? 'resumen.md' : 'traduccion.md'}
    label="Guardar resultado (opcional)"
    filters={[{ name: 'Markdown / texto', extensions: ['md', 'txt'] }]}
  />

  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    {action === 'summarize' ? 'Resumir' : 'Traducir'}
  </button>

  {#if preview}
    <div class="mp-field">
      <span>Resultado</span>
      <textarea class="mp-input" rows="12" readonly value={preview}></textarea>
    </div>
  {/if}
</div>
