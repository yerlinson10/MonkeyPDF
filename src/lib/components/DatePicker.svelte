<script lang="ts">
  /**
   * Hallmark date picker — hard borders, banana stamp, Syne.
   * value is ISO day `YYYY-MM-DD` or empty string.
   */
  import { onMount } from 'svelte'
  import { formatDisplayDate, toIsoDay } from '../pdfDate'

  interface Props {
    value?: string
    label?: string
    /** Visual size: form fields on PDF use compact. */
    size?: 'default' | 'compact'
    placeholder?: string
    displayFormat?: string
    disabled?: boolean
    clearable?: boolean
    /** Fired when ISO day changes (also bind:value). */
    onchange?: (iso: string) => void
  }

  let {
    value = $bindable(''),
    label = '',
    size = 'default',
    placeholder = 'Elegir fecha',
    displayFormat = 'es-short',
    disabled = false,
    clearable = true,
    onchange,
  }: Props = $props()

  let open = $state(false)
  let rootEl = $state<HTMLDivElement | null>(null)
  let viewYear = $state(new Date().getFullYear())
  let viewMonth = $state(new Date().getMonth())

  const WEEKDAYS = ['L', 'M', 'X', 'J', 'V', 'S', 'D']
  const MONTHS = [
    'Enero',
    'Febrero',
    'Marzo',
    'Abril',
    'Mayo',
    'Junio',
    'Julio',
    'Agosto',
    'Septiembre',
    'Octubre',
    'Noviembre',
    'Diciembre',
  ]

  const display = $derived(
    value ? formatDisplayDate(value, displayFormat) : '',
  )

  $effect(() => {
    if (!open) return
    if (value && /^\d{4}-\d{2}-\d{2}$/.test(value)) {
      viewYear = Number(value.slice(0, 4))
      viewMonth = Number(value.slice(5, 7)) - 1
    } else {
      const now = new Date()
      viewYear = now.getFullYear()
      viewMonth = now.getMonth()
    }
  })

  onMount(() => {
    const onDoc = (e: PointerEvent) => {
      if (!open || !rootEl) return
      if (e.target instanceof Node && !rootEl.contains(e.target)) {
        open = false
      }
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) {
        open = false
      }
    }
    document.addEventListener('pointerdown', onDoc, true)
    window.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('pointerdown', onDoc, true)
      window.removeEventListener('keydown', onKey)
    }
  })

  function daysInMonth(y: number, m: number) {
    return new Date(y, m + 1, 0).getDate()
  }

  /** Monday-first offset 0..6 */
  function startOffset(y: number, m: number) {
    const dow = new Date(y, m, 1).getDay() // 0 Sun
    return dow === 0 ? 6 : dow - 1
  }

  const cells = $derived.by(() => {
    const total = daysInMonth(viewYear, viewMonth)
    const offset = startOffset(viewYear, viewMonth)
    const out: Array<{ day: number | null; iso: string | null }> = []
    for (let i = 0; i < offset; i++) out.push({ day: null, iso: null })
    for (let d = 1; d <= total; d++) {
      const iso = `${viewYear}-${String(viewMonth + 1).padStart(2, '0')}-${String(d).padStart(2, '0')}`
      out.push({ day: d, iso })
    }
    while (out.length % 7 !== 0) out.push({ day: null, iso: null })
    return out
  })

  function toggle() {
    if (disabled) return
    open = !open
  }

  function pick(iso: string) {
    value = iso
    onchange?.(iso)
    open = false
  }

  function clear() {
    value = ''
    onchange?.('')
    open = false
  }

  function today() {
    pick(toIsoDay(new Date()))
  }

  function shiftMonth(delta: number) {
    let m = viewMonth + delta
    let y = viewYear
    if (m < 0) {
      m = 11
      y -= 1
    } else if (m > 11) {
      m = 0
      y += 1
    }
    viewMonth = m
    viewYear = y
  }

  const todayIso = toIsoDay(new Date())
</script>

<div
  class="mp-date"
  class:is-compact={size === 'compact'}
  class:is-open={open}
  class:is-disabled={disabled}
  bind:this={rootEl}
>
  {#if label}
    <span class="mp-date-label">{label}</span>
  {/if}

  <button
    type="button"
    class="mp-date-trigger"
    {disabled}
    aria-haspopup="dialog"
    aria-expanded={open}
    onclick={toggle}
  >
    <span class="mp-date-value" class:is-empty={!display}>
      {display || placeholder}
    </span>
    <span class="mp-date-ico" aria-hidden="true">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="5" width="18" height="16" rx="1.5" />
        <path d="M8 3v4M16 3v4M3 11h18" />
      </svg>
    </span>
  </button>

  {#if open}
    <div class="mp-date-pop" role="dialog" aria-label="Calendario">
      <div class="mp-date-nav">
        <button type="button" class="mp-date-nav-btn" aria-label="Mes anterior" onclick={() => shiftMonth(-1)}>
          ‹
        </button>
        <strong class="mp-date-month">{MONTHS[viewMonth]} {viewYear}</strong>
        <button type="button" class="mp-date-nav-btn" aria-label="Mes siguiente" onclick={() => shiftMonth(1)}>
          ›
        </button>
      </div>

      <div class="mp-date-week" aria-hidden="true">
        {#each WEEKDAYS as w}
          <span>{w}</span>
        {/each}
      </div>

      <div class="mp-date-grid">
        {#each cells as cell, i (i)}
          {#if cell.day == null || !cell.iso}
            <span class="mp-date-cell is-empty"></span>
          {:else}
            <button
              type="button"
              class="mp-date-cell"
              class:is-selected={cell.iso === value}
              class:is-today={cell.iso === todayIso}
              onclick={() => pick(cell.iso!)}
            >
              {cell.day}
            </button>
          {/if}
        {/each}
      </div>

      <div class="mp-date-foot">
        <button type="button" class="mp-chip" onclick={today}>Hoy</button>
        {#if clearable}
          <button type="button" class="mp-chip" onclick={clear} disabled={!value}>Quitar</button>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .mp-date {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    width: 100%;
  }

  .mp-date-label {
    font-size: var(--text-xs);
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--color-ink-2);
  }

  .mp-date-trigger {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    min-height: 44px;
    padding: 0 var(--space-3);
    border: 1.5px solid var(--color-rule-strong);
    border-radius: var(--radius-sm);
    background: var(--color-paper);
    color: var(--color-ink);
    font: inherit;
    font-size: var(--text-sm);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    text-align: left;
    box-sizing: border-box;
  }

  .mp-date-trigger:hover:not(:disabled) {
    border-color: var(--color-ink);
  }

  .mp-date-trigger:focus-visible {
    outline: none;
    border-color: var(--color-ink);
    box-shadow: 0 0 0 3px var(--color-accent-soft);
  }

  .mp-date.is-disabled .mp-date-trigger {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .mp-date.is-open .mp-date-trigger {
    border-color: var(--color-ink);
    box-shadow: 0 0 0 3px var(--color-accent-soft);
  }

  .mp-date-value {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mp-date-value.is-empty {
    color: var(--color-ink-2);
    font-weight: 600;
  }

  .mp-date-ico {
    display: grid;
    place-items: center;
    color: var(--color-ink-2);
    flex-shrink: 0;
  }

  .mp-date-pop {
    position: absolute;
    z-index: 80;
    top: calc(100% + 6px);
    left: 0;
    width: min(100%, 17.5rem);
    min-width: 16.5rem;
    padding: 0.65rem;
    background: var(--color-paper);
    border: 2px solid var(--color-ink);
    border-radius: var(--radius-sm);
    box-shadow: 4px 4px 0 var(--color-ink);
  }

  .mp-date-nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.35rem;
    margin-bottom: 0.5rem;
  }

  .mp-date-month {
    font-size: var(--text-sm);
    font-weight: 800;
    letter-spacing: 0.01em;
  }

  .mp-date-nav-btn {
    display: grid;
    place-items: center;
    width: 1.75rem;
    height: 1.75rem;
    border: 1.5px solid var(--color-ink);
    background: var(--color-paper);
    color: var(--color-ink);
    box-shadow: 2px 2px 0 var(--color-ink);
    cursor: pointer;
    padding: 0;
    font: inherit;
    font-size: 1.1rem;
    font-weight: 800;
    line-height: 1;
  }

  .mp-date-nav-btn:hover {
    background: var(--color-accent);
  }

  .mp-date-week {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
    margin-bottom: 0.25rem;
  }

  .mp-date-week span {
    text-align: center;
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.06em;
    color: var(--color-ink-2);
    text-transform: uppercase;
  }

  .mp-date-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
  }

  .mp-date-cell {
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    border: 1.5px solid transparent;
    background: transparent;
    color: var(--color-ink);
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    padding: 0;
  }

  .mp-date-cell:not(.is-empty):hover {
    border-color: var(--color-ink);
    background: var(--color-paper-2);
  }

  .mp-date-cell.is-today:not(.is-selected) {
    border-color: var(--color-rule-strong);
  }

  .mp-date-cell.is-selected {
    background: var(--color-accent);
    border-color: var(--color-ink);
    box-shadow: 2px 2px 0 var(--color-ink);
    font-weight: 800;
  }

  .mp-date-cell.is-empty {
    pointer-events: none;
  }

  .mp-date-foot {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
    margin-top: 0.55rem;
    padding-top: 0.45rem;
    border-top: 1.5px solid var(--color-rule);
  }

  .mp-date-foot :global(.mp-chip) {
    min-height: 28px;
    padding: 0 0.65rem;
    font-size: 11px;
  }

  /* Compact — overlay form fields on PDF pages */
  .mp-date.is-compact {
    height: 100%;
  }

  .mp-date.is-compact .mp-date-trigger {
    min-height: 100%;
    height: 100%;
    padding: 0 0.35rem;
    font-size: 11px;
    border-width: 0;
    background: color-mix(in srgb, var(--color-paper) 92%, transparent);
    box-shadow: none;
  }

  .mp-date.is-compact .mp-date-pop {
    top: calc(100% + 4px);
    left: 0;
    min-width: 15.5rem;
  }

  .mp-date.is-compact.is-open .mp-date-trigger {
    box-shadow: none;
    outline: 1.5px solid var(--color-ink);
  }
</style>
