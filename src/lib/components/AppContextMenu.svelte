<script lang="ts">
  import { onMount } from 'svelte'
  import {
    closeContextMenu,
    subscribeContextMenu,
    type CtxState,
  } from '../contextMenu'

  let menu = $state<CtxState>(null)
  let rootEl = $state<HTMLDivElement | null>(null)

  onMount(() => {
    const unsub = subscribeContextMenu((s) => {
      menu = s
    })
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeContextMenu()
    }
    const onDown = (e: MouseEvent) => {
      if (!menu) return
      const t = e.target
      if (t instanceof Node && rootEl?.contains(t)) return
      closeContextMenu()
    }
    const onScroll = () => closeContextMenu()
    window.addEventListener('keydown', onKey)
    window.addEventListener('mousedown', onDown, true)
    window.addEventListener('scroll', onScroll, true)
    return () => {
      unsub()
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('mousedown', onDown, true)
      window.removeEventListener('scroll', onScroll, true)
    }
  })

  async function run(item: Extract<NonNullable<CtxState>['items'][number], { run: unknown }>) {
    if (item.disabled) return
    closeContextMenu()
    await item.run()
  }
</script>

{#if menu}
  <div
    class="mp-ctx"
    bind:this={rootEl}
    style="left:{menu.x}px;top:{menu.y}px;"
    role="menu"
    aria-label="Menú contextual"
  >
    {#each menu.items as item (item.id)}
      {#if 'separator' in item && item.separator}
        <div class="mp-ctx-sep" role="separator"></div>
      {:else if 'run' in item}
        <button
          type="button"
          class="mp-ctx-item"
          class:is-danger={item.danger}
          class:is-disabled={item.disabled}
          role="menuitem"
          disabled={item.disabled}
          onclick={() => run(item)}
        >
          <span>{item.label}</span>
          {#if item.hint}
            <kbd>{item.hint}</kbd>
          {/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .mp-ctx {
    position: fixed;
    z-index: 9999;
    min-width: 12.5rem;
    max-width: 18rem;
    padding: 0.35rem;
    border: 1.5px solid var(--color-ink);
    border-radius: var(--radius-sm, 4px);
    background: var(--color-paper, #fffdf6);
    box-shadow: 4px 4px 0 var(--color-ink);
    font-family: var(--font-ui, inherit);
  }

  .mp-ctx-sep {
    height: 1px;
    margin: 0.3rem 0.35rem;
    background: var(--color-rule, #d5ccb8);
  }

  .mp-ctx-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    width: 100%;
    min-height: 2rem;
    padding: 0.35rem 0.55rem;
    border: 0;
    border-radius: 2px;
    background: transparent;
    color: var(--color-ink, #1a1a1a);
    font: inherit;
    font-size: var(--text-sm, 0.875rem);
    font-weight: 700;
    text-align: left;
    cursor: pointer;
  }

  .mp-ctx-item:hover:not(:disabled),
  .mp-ctx-item:focus-visible {
    background: var(--color-accent, #f5d76e);
    outline: none;
  }

  .mp-ctx-item.is-danger {
    color: var(--color-danger, #c0392b);
  }

  .mp-ctx-item:disabled,
  .mp-ctx-item.is-disabled {
    opacity: 0.4;
    cursor: default;
  }

  .mp-ctx-item kbd {
    font-family: inherit;
    font-size: 0.65rem;
    font-weight: 800;
    letter-spacing: 0.04em;
    color: var(--color-ink-2, #666);
  }
</style>
