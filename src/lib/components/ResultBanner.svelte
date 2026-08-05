<script lang="ts">
  /* Hallmark · component: toast · genre: editorial · theme: press-shop banana
   * states: default · hover · focus · active · disabled · loading · error · success
   * contrast: pass
   */
  import { fileName, revealInExplorer, type OpResult } from '../api'
  import { notifyError, notifySuccess } from '../notify'
  import Icon from './Icon.svelte'

  interface Props {
    result?: OpResult | null
    error?: string | null
    loading?: boolean
    /** Short label for the OS notification (e.g. "Unir PDF"). */
    toolLabel?: string
  }

  let { result = null, error = null, loading = false, toolLabel = 'Operación' }: Props =
    $props()

  let lastNotifiedResult = $state<OpResult | null>(null)
  let lastNotifiedError = $state<string | null>(null)
  let dismissed = $state(false)
  let dismissTimer: ReturnType<typeof setTimeout> | null = null

  const visible = $derived(!dismissed && (loading || !!error || !!result))
  const tone = $derived(loading ? 'loading' : error ? 'error' : result ? 'success' : 'idle')

  $effect(() => {
    if (loading || error || result) {
      dismissed = false
    }
  })

  $effect(() => {
    if (result && result !== lastNotifiedResult) {
      lastNotifiedResult = result
      void notifySuccess(result, toolLabel)
      scheduleAutoDismiss(4500)
    }
  })

  $effect(() => {
    if (error && error !== lastNotifiedError) {
      lastNotifiedError = error
      void notifyError(error)
      scheduleAutoDismiss(7000)
    }
  })

  $effect(() => {
    if (loading) {
      clearDismissTimer()
    }
  })

  function clearDismissTimer() {
    if (dismissTimer) {
      clearTimeout(dismissTimer)
      dismissTimer = null
    }
  }

  function scheduleAutoDismiss(ms: number) {
    clearDismissTimer()
    dismissTimer = setTimeout(() => {
      dismissed = true
      dismissTimer = null
    }, ms)
  }

  function dismiss() {
    clearDismissTimer()
    dismissed = true
  }

  async function openPath(path: string) {
    try {
      await revealInExplorer(path)
    } catch (e) {
      console.warn('No se pudo abrir en el explorador', e)
    }
  }
</script>

{#if visible}
  <div class="mp-toast-host" aria-live="polite">
    <div
      class="mp-toast"
      class:is-loading={tone === 'loading'}
      class:is-error={tone === 'error'}
      class:is-success={tone === 'success'}
      role={tone === 'error' ? 'alert' : 'status'}
      data-state={tone}
    >
      <div class="mp-toast-mark" aria-hidden="true">
        {#if tone === 'loading'}
          <span class="mp-toast-pulse"></span>
        {:else if tone === 'error'}
          <Icon name="x" size={16} stroke={2.25} />
        {:else}
          <Icon name="check" size={16} stroke={2.25} />
        {/if}
      </div>

      <div class="mp-toast-body">
        {#if loading}
          <span class="mp-toast-kicker">Taller</span>
          <p class="mp-toast-title">Procesando</p>
          <p class="mp-toast-meta">Un momento — el sello aún gira.</p>
        {:else if error}
          <span class="mp-toast-kicker">Fallo</span>
          <p class="mp-toast-title">No se pudo terminar</p>
          <p class="mp-toast-meta mono">{error}</p>
        {:else if result}
          <span class="mp-toast-kicker">Listo</span>
          <p class="mp-toast-title">
            {toolLabel}
            <span class="mp-toast-stats mono"
              >{result.elapsedMs} ms · {result.pageCount} pág.</span
            >
          </p>
          <ul class="mp-toast-paths">
            {#each result.outputPaths as path}
              <li>
                <button
                  type="button"
                  class="mp-path-link mono"
                  title="Abrir en el explorador"
                  onclick={() => openPath(path)}
                >
                  {fileName(path)}
                  <span class="mp-path-full">{path}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      {#if !loading}
        <button type="button" class="mp-toast-close" aria-label="Cerrar" onclick={dismiss}>
          <Icon name="x" size={14} stroke={2} />
        </button>
      {/if}
    </div>
  </div>
{/if}
