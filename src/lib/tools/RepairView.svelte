<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { diagnosePdf, repairPdf, type Diagnosis, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let password = $state('')
  let diagnosis = $state<Diagnosis | null>(null)
  let diagnosing = $state(false)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

  $effect(() => {
    const p = paths[0]
    diagnosis = null
    if (!p) return
    diagnosing = true
    error = null
    void diagnosePdf(p)
      .then((d) => {
        diagnosis = d
      })
      .catch((e) => {
        error = String(e)
      })
      .finally(() => {
        diagnosing = false
      })
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
    if (diagnosis?.encrypted && !password.trim()) {
      error = 'PDF cifrado: indica la contraseña'
      return
    }
    loading = true
    try {
      result = await repairPdf(paths[0], output, password.trim() || null)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }
</script>

<div class="space-y-5">
  <ResultBanner {loading} {error} {result} toolLabel="Reparar PDF" />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF dañado o dudoso" />

  {#if diagnosing}
    <p class="text-[var(--text-xs)] text-[var(--color-ink-2)]">Diagnosticando…</p>
  {/if}

  {#if diagnosis}
    <div class="diag-card">
      <div class="diag-head">
        <strong>Diagnóstico</strong>
        <span class="mp-stamp-tag">PDF {diagnosis.pdfVersion || '?'}</span>
      </div>
      <div class="mp-hint-row" style="margin-top: 0.5rem">
        <span class="mp-chip is-on">{diagnosis.pageCount} pág.</span>
        <span class="mp-chip" class:is-warn={diagnosis.encrypted}>
          {diagnosis.encrypted ? 'Cifrado' : 'Sin cifrado'}
        </span>
        <span class="mp-chip" class:is-warn={!diagnosis.hasEof}>
          {diagnosis.hasEof ? 'EOF OK' : 'Sin EOF'}
        </span>
        <span class="mp-chip" class:is-warn={diagnosis.brokenObjects > 0}>
          {diagnosis.brokenObjects} stream rotos
        </span>
        <span class="mp-chip" class:is-warn={diagnosis.orphanObjects > 0}>
          {diagnosis.orphanObjects} huérfanos
        </span>
        <span class="mp-chip" class:is-warn={diagnosis.missingPages > 0}>
          {diagnosis.missingPages} pág. incompletas
        </span>
        {#if diagnosis.linearized}
          <span class="mp-chip is-warn">Linearizado</span>
        {/if}
      </div>
      <ul class="diag-list">
        {#each diagnosis.warnings as w}
          <li>{w}</li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if diagnosis?.encrypted}
    <div class="mp-field">
      <label for="repair-pwd">Contraseña (quitar cifrado)</label>
      <input
        id="repair-pwd"
        class="mp-input"
        type="password"
        autocomplete="current-password"
        bind:value={password}
        placeholder="Obligatoria si el PDF está cifrado"
      />
    </div>
  {/if}

  <OutputPicker bind:value={output} defaultName="reparado.pdf" label="PDF de salida" />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading || diagnosing} onclick={run}>
    Reparar
  </button>
</div>

<style>
  .diag-card {
    border: 2px solid var(--color-ink);
    box-shadow: 4px 4px 0 var(--color-ink);
    padding: 0.85rem 1rem;
    background: var(--color-paper, #fff);
    color: var(--color-ink);
  }

  .diag-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .diag-list {
    margin: 0.65rem 0 0;
    padding-left: 1.1rem;
    font-size: var(--text-xs);
    color: var(--color-ink-2);
  }

  :global(.mp-chip.is-warn) {
    background: var(--color-banana, #f5d547) !important;
    border-color: var(--color-ink) !important;
  }
</style>
