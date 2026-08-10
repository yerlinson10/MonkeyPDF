<script lang="ts">
  export type IconName =
    | 'merge'
    | 'split'
    | 'organize'
    | 'rotate'
    | 'compress'
    | 'pdf-to-jpg'
    | 'jpg-to-pdf'
    | 'protect'
    | 'repair'
    | 'metadata'
    | 'page-numbers'
    | 'office'
    | 'pdfa'
    | 'extract'
    | 'ocr'
    | 'redact'
    | 'crop'
    | 'watermark'
    | 'compare'
    | 'sign'
    | 'edit'
    | 'markdown'
    | 'ai'
    | 'settings'
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
  {:else if name === 'organize'}
    <rect x="3" y="3" width="7" height="7" rx="1" />
    <rect x="14" y="3" width="7" height="7" rx="1" />
    <rect x="3" y="14" width="7" height="7" rx="1" />
    <rect x="14" y="14" width="7" height="7" rx="1" />
    <path d="M10 6.5h4M14 17.5h-4" />
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
  {:else if name === 'protect'}
    <rect x="5" y="11" width="14" height="10" rx="1.5" />
    <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    <circle cx="12" cy="16" r="1.2" fill="currentColor" stroke="none" />
  {:else if name === 'repair'}
    <path d="M14.7 6.3a4 4 0 0 0-5.6 5.6L3 18l3 3 6.1-6.1a4 4 0 0 0 5.6-5.6l-2.5 2.5-2.5-2.5 2.5-2.5z" />
  {:else if name === 'metadata'}
    <rect x="5" y="3" width="14" height="18" rx="1.5" />
    <path d="M8 8h8M8 12h8M8 16h5" />
    <circle cx="16.5" cy="16" r="1.2" fill="currentColor" stroke="none" />
  {:else if name === 'page-numbers'}
    <rect x="5" y="3" width="14" height="18" rx="1.5" />
    <path d="M9 8h6M9 12h6M9 16h3" />
  {:else if name === 'office'}
    <rect x="4" y="4" width="7" height="7" rx="1" />
    <rect x="13" y="4" width="7" height="7" rx="1" />
    <rect x="4" y="13" width="7" height="7" rx="1" />
    <rect x="13" y="13" width="7" height="7" rx="1" />
  {:else if name === 'pdfa'}
    <rect x="5" y="3" width="14" height="18" rx="1.5" />
    <path d="M8 8h8M8 12h5" />
    <path d="M15 15l2 2 3-4" />
  {:else if name === 'extract'}
    <rect x="4" y="4" width="10" height="14" rx="1.5" />
    <path d="M14 8h5v11a1.5 1.5 0 0 1-1.5 1.5H14" />
    <path d="M8 10h2M8 13h2" />
    <path d="M17 12v5M15 15l2 2 2-2" />
  {:else if name === 'ocr'}
    <rect x="4" y="5" width="16" height="14" rx="1.5" />
    <path d="M8 9h8M8 12h6M8 15h4" />
    <circle cx="17" cy="15" r="1.5" />
  {:else if name === 'redact'}
    <rect x="4" y="4" width="16" height="16" rx="1.5" />
    <path d="M7 8h10M7 12h10M7 16h6" stroke-width="2.5" />
  {:else if name === 'crop'}
    <path d="M6 3v12a3 3 0 0 0 3 3h12" />
    <path d="M18 21V9a3 3 0 0 0-3-3H3" />
  {:else if name === 'watermark'}
    <rect x="5" y="3" width="12" height="16" rx="1.5" />
    <path d="M14 14c0-2.2 1.8-3.5 1.8-5.2A1.8 1.8 0 0 0 14 7a1.8 1.8 0 0 0-1.8 1.8c0 1.7 1.8 3 1.8 5.2z" />
  {:else if name === 'compare'}
    <rect x="3" y="4" width="8" height="16" rx="1" />
    <rect x="13" y="4" width="8" height="16" rx="1" />
    <path d="M6 9h2M6 12h2M16 9h2M16 13h2" />
  {:else if name === 'sign'}
    <path d="M4 20h16" />
    <path d="M6 16c2-4 4-8 6-8s3 2 5 6" />
    <path d="M14 8l3-3 2 2-3 3" />
  {:else if name === 'edit'}
    <path d="M5 19h3l9.5-9.5-3-3L5 16v3z" />
    <path d="M13.5 7.5l3 3" />
    <path d="M4 21h16" />
  {:else if name === 'markdown'}
    <path d="M4 6h16v12H4z" />
    <path d="M7 15V9l2.5 3L12 9v6M15 12h2m0 0l-1.5-2M17 12l-1.5 2" />
  {:else if name === 'ai'}
    <circle cx="12" cy="12" r="3" />
    <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M5.6 18.4l1.4-1.4M17 7l1.4-1.4" />
  {:else if name === 'settings'}
    <circle cx="12" cy="12" r="3" />
    <path
      d="M12 3.5v2.2M12 18.3v2.2M4.9 7.1l1.6 1.6M17.5 15.3l1.6 1.6M3.5 12h2.2M18.3 12h2.2M4.9 16.9l1.6-1.6M17.5 8.7l1.6-1.6"
    />
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
