<script lang="ts">
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import { protectPdf, unlockPdf, type OpResult } from '../api'

  let paths = $state<string[]>([])
  let output = $state('')
  let mode = $state<'protect' | 'unlock'>('protect')
  let password = $state('')
  let ownerPassword = $state('')
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)

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
    if (!password) {
      error = 'Escribe la contraseña'
      return
    }
    loading = true
    try {
      if (mode === 'protect') {
        result = await protectPdf(
          paths[0],
          password,
          ownerPassword.trim() ? ownerPassword : null,
          output,
        )
      } else {
        result = await unlockPdf(paths[0], password, output)
      }
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
    toolLabel={mode === 'protect' ? 'Proteger PDF' : 'Desbloquear PDF'}
  />
  <FileDropZone bind:paths accept=".pdf" multiple={false} label="Arrastra un PDF" />

  <div class="mp-field">
    <span>Modo</span>
    <div class="mp-hint-row" style="margin-top: 0">
      <button
        type="button"
        class="mp-chip"
        class:is-on={mode === 'protect'}
        onclick={() => (mode = 'protect')}>Proteger</button
      >
      <button
        type="button"
        class="mp-chip"
        class:is-on={mode === 'unlock'}
        onclick={() => (mode = 'unlock')}>Desbloquear</button
      >
    </div>
  </div>

  <div class="mp-field">
    <label for="pwd">Contraseña de usuario *</label>
    <input
      id="pwd"
      class="mp-input"
      type="password"
      autocomplete="new-password"
      bind:value={password}
      placeholder="Obligatoria"
    />
  </div>

  {#if mode === 'protect'}
    <div class="mp-field">
      <label for="owner">Contraseña de propietario (opcional)</label>
      <input
        id="owner"
        class="mp-input"
        type="password"
        autocomplete="new-password"
        bind:value={ownerPassword}
        placeholder="Igual a la de usuario si se omite"
      />
    </div>
  {/if}

  <OutputPicker
    bind:value={output}
    tool="protect"
    defaultName={mode === 'protect' ? 'protected.pdf' : 'unlocked.pdf'}
    label="PDF de salida"
  />
  <button type="button" class="mp-btn mp-btn-primary" disabled={loading} onclick={run}>
    {mode === 'protect' ? 'Proteger PDF' : 'Desbloquear PDF'}
  </button>
</div>
