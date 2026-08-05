<script lang="ts">
  export type IconName =
    | 'merge'
    | 'split'
    | 'rotate'
    | 'compress'
    | 'pdf-to-jpg'
    | 'jpg-to-pdf'
    | 'upload'
    | 'folder'
    | 'arrow-up'
    | 'arrow-down'
    | 'x'
    | 'check'
    | 'monkey'

  interface Props {
    name: IconName
    size?: number
    stroke?: number
    class?: string
  }

  let { name, size = 20, stroke = 1.75, class: className = '' }: Props = $props()
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === 'merge'}
    <path d="M8 6v4a4 4 0 0 0 4 4h0a4 4 0 0 0 4-4V6" />
    <path d="M8 18v-4a4 4 0 0 1 4-4h0a4 4 0 0 1 4 4v4" />
    <path d="M12 10v4" />
  {:else if name === 'split'}
    <path d="M12 3v18" />
    <path d="M5 8l7 4 7-4" />
    <path d="M5 16l7-4 7 4" />
  {:else if name === 'rotate'}
    <path d="M21 12a9 9 0 1 1-3-6.7" />
    <path d="M21 3v6h-6" />
  {:else if name === 'compress'}
    <path d="M4 14h6v6" />
    <path d="M20 10h-6V4" />
    <path d="M14 10l7-7" />
    <path d="M3 21l7-7" />
  {:else if name === 'pdf-to-jpg'}
    <rect x="4" y="3" width="12" height="16" rx="1.5" />
    <path d="M8 7h4M8 11h4M8 15h2" />
    <path d="M14 14l4 2v5l-4 2-4-2v-5l4-2z" />
  {:else if name === 'jpg-to-pdf'}
    <rect x="8" y="3" width="12" height="16" rx="1.5" />
    <path d="M4 7h4v12a1.5 1.5 0 0 1-1.5 1.5H5.5A1.5 1.5 0 0 1 4 19V7z" />
    <path d="M12 8h4M12 12h4" />
  {:else if name === 'upload'}
    <path d="M12 16V6" />
    <path d="M8 10l4-4 4 4" />
    <path d="M4 18h16" />
  {:else if name === 'folder'}
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" />
  {:else if name === 'arrow-up'}
    <path d="M12 19V5" />
    <path d="M6 11l6-6 6 6" />
  {:else if name === 'arrow-down'}
    <path d="M12 5v14" />
    <path d="M6 13l6 6 6-6" />
  {:else if name === 'x'}
    <path d="M6 6l12 12M18 6L6 18" />
  {:else if name === 'check'}
    <path d="M5 12l5 5L20 7" />
  {:else if name === 'monkey'}
    <circle cx="12" cy="13" r="6.5" />
    <circle cx="7.5" cy="8" r="2.2" />
    <circle cx="16.5" cy="8" r="2.2" />
    <circle cx="10" cy="12.5" r="0.8" fill="currentColor" stroke="none" />
    <circle cx="14" cy="12.5" r="0.8" fill="currentColor" stroke="none" />
    <path d="M10 16c.7.7 1.4 1 2 1s1.3-.3 2-1" />
  {/if}
</svg>
