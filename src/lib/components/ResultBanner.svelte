<script lang="ts">
  /* Hallmark · component: toast · genre: editorial · theme: press-shop banana
   * states: default · hover · focus · active · disabled · loading · error · success
   * contrast: pass
   */
  import { fileName, revealInExplorer, type OpResult, type ToolId } from '../api'
  import { notifyError, notifySuccess } from '../notify'
  import { pushHistory } from '../history'
  import { cancelJob, type JobProgress } from '../jobProgress'
  import Icon from './Icon.svelte'

  interface Props {
    result?: OpResult | null
    error?: string | null
    loading?: boolean
    /** Short label for the OS notification (e.g. "Unir PDF"). */
    toolLabel?: string
    /** When set with inputs, successful runs are added to history. */
    toolId?: Exclude<ToolId, 'settings'>
    inputs?: string[]
    progress?: JobProgress | null
    /** Show cancel while loading (long jobs). */
    cancellable?: boolean
  }

  let {
    result = null,
    error = null,
    loading = false,
    toolLabel = 'Operación',
    toolId,
    inputs = [],
    progress = null,
    cancellable = false,
  }: Props = $props()

  let lastNotifiedResult = $state<OpResult | null>(null)
  let lastNotifiedError = $state<string | null>(null)
  let dismissed = $state(false)
  let dismissTimer: ReturnType<typeof setTimeout> | null = null
  let cancelling = $state(false)

  const visible = $derived(!dismissed && (loading || !!error || !!result))
  const tone = $derived(loading ? 'loading' : error ? 'error' : result ? 'success' : 'idle')
  const pct = $derived.by(() => {
    if (!progress || progress.total <= 0) return null
    return Math.min(100, Math.round((progress.current / progress.total) * 100))
  })

  $effect(() => {
    if (loading || error || result) {
      dismissed = false
    }
  })

  $effect(() => {
    if (result && result !== lastNotifiedResult) {
      lastNotifiedResult = result
      void notifySuccess(result, toolLabel)
      if (toolId && result.outputPaths.length) {
        void pushHistory({
          toolId,
          toolLabel,
          inputs,
          outputs: result.outputPaths,
          pageCount: result.pageCount,
          elapsedMs: result.elapsedMs,
        }).then(() => {
          window.dispatchEvent(new CustomEvent('mp-history'))
        })
      }
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
      cancelling = false
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

  async function onCancel() {
    cancelling = true
    try {
      await cancelJob()
    } catch (e) {
      console.warn('cancel failed', e)
      cancelling = false
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
          <p class="mp-toast-title">
            {cancelling ? 'Cancelando…' : 'Procesando'}
            {#if pct !== null}
              <span class="mp-toast-stats mono">{pct}%</span>
            {/if}
          </p>
          <p class="mp-toast-meta">
            {progress?.label || (cancelling ? 'Esperando al motor…' : 'Un momento — el sello aún gira.')}
          </p>
          {#if pct !== null}
            <div class="mp-toast-bar" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
              <span style={`width:${pct}%`}></span>
            </div>
          {/if}
        {:else if error}
          <span class="mp-toast-kicker">Fallo</span>
          <p class="mp-toast-title">No se pudo terminar</p>
          <p class="mp-toast-meta mono">{error}</p>
        {:else if result}
          <span class="mp-toast-kicker">{result.partial ? 'Parcial' : 'Listo'}</span>
          <p class="mp-toast-title">
            {result.partial ? 'Rescate parcial' : toolLabel}
            <span class="mp-toast-stats mono"
              >{result.elapsedMs} ms · {result.pageCount} pág.</span
            >
          </p>
          {#if result.partial && result.warnings?.length}
            <p class="mp-toast-meta">{result.warnings[0]}</p>
          {/if}
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

      {#if loading && cancellable}
        <button
          type="button"
          class="mp-toast-close"
          aria-label="Cancelar"
          disabled={cancelling}
          onclick={onCancel}
        >
          <Icon name="x" size={14} stroke={2} />
        </button>
      {:else if !loading}
        <button type="button" class="mp-toast-close" aria-label="Cerrar" onclick={dismiss}>
          <Icon name="x" size={14} stroke={2} />
        </button>
      {/if}
    </div>
  </div>
{/if}
