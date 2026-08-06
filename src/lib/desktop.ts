/**
 * Makes the WebView behave like a desktop shell, not a browser tab.
 * Call once at app boot.
 */
import { invoke } from '@tauri-apps/api/core'
import { copyText, selectedText } from './clipboard'

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false
  return !!target.closest('input, textarea, [contenteditable="true"]')
}

export function installDesktopShell() {
  // Block Chromium inspect/reload menu; editable fields keep native menu.
  // Elsewhere components open our AppContextMenu via oncontextmenu.
  document.addEventListener(
    'contextmenu',
    (event) => {
      if (isEditableTarget(event.target)) return
      event.preventDefault()
    },
    { capture: true },
  )

  document.addEventListener(
    'keydown',
    (event) => {
      const key = event.key.toLowerCase()
      const mod = event.ctrlKey || event.metaKey
      const editable = isEditableTarget(event.target)

      // Always allow clipboard + select-all shortcuts
      if (mod && (key === 'c' || key === 'x')) {
        const sel = selectedText()
        if (sel) {
          event.preventDefault()
          void copyText(sel)
        }
        // If no selection in editable, let the field handle cut/copy
        return
      }
      if (mod && key === 'v') {
        // Never block paste — inputs/textarea need it
        return
      }
      if (mod && key === 'a') {
        if (editable) return
        // Select all in focused selectable region if any
        const active = document.activeElement
        if (active instanceof HTMLElement && active.classList.contains('selectable')) {
          const range = document.createRange()
          range.selectNodeContents(active)
          const sel = window.getSelection()
          sel?.removeAllRanges()
          sel?.addRange(range)
          event.preventDefault()
        }
        return
      }

      // Reload
      if (key === 'f5' || (mod && key === 'r')) {
        event.preventDefault()
        return
      }

      // Browser zoom hotkeys (app has its own zoom)
      if (mod && (key === '+' || key === '=' || key === '-' || key === '0')) {
        event.preventDefault()
        return
      }

      // Browser page actions that don't belong in a desktop app
      if (mod && ['u', 'p', 's', 'n', 't', 'j'].includes(key)) {
        event.preventDefault()
        return
      }

      if (mod && event.shiftKey && ['i', 'c', 'j', 'k'].includes(key) && !import.meta.env.DEV) {
        event.preventDefault()
      }
    },
    { capture: true },
  )

  document.addEventListener(
    'dragstart',
    (event) => {
      const target = event.target
      if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return
      if (target instanceof Element && target.closest('img, a, svg')) {
        event.preventDefault()
      }
    },
    { capture: true },
  )

  document.addEventListener(
    'auxclick',
    (event) => {
      if (event.button === 1) event.preventDefault()
    },
    { capture: true },
  )

  // External links: open in the OS browser (webview ignores target=_blank)
  document.addEventListener(
    'click',
    (event) => {
      const target = event.target
      if (!(target instanceof Element)) return
      const anchor = target.closest('a[href]')
      if (!(anchor instanceof HTMLAnchorElement)) return
      const href = anchor.getAttribute('href')?.trim() ?? ''
      if (!(href.startsWith('https://') || href.startsWith('http://'))) return
      event.preventDefault()
      void invoke('open_url', { url: href }).catch((err) => {
        console.warn('No se pudo abrir la URL', err)
      })
    },
    { capture: true },
  )
}
