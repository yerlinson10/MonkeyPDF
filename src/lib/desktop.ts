/**
 * Makes the WebView behave like a desktop shell, not a browser tab.
 * Call once at app boot.
 */
import { invoke } from '@tauri-apps/api/core'

export function installDesktopShell() {
  // Hide Chromium/WebView right-click menu (Inspect, Reload, Save as…)
  // Keep OS menu on editable fields for copy/paste.
  document.addEventListener(
    'contextmenu',
    (event) => {
      const target = event.target
      if (!(target instanceof Element)) {
        event.preventDefault()
        return
      }
      if (target.closest('input, textarea, [contenteditable="true"]')) return
      event.preventDefault()
    },
    { capture: true },
  )

  document.addEventListener(
    'keydown',
    (event) => {
      const key = event.key.toLowerCase()
      const mod = event.ctrlKey || event.metaKey

      // Reload
      if (key === 'f5' || (mod && key === 'r')) {
        event.preventDefault()
        return
      }

      // Zoom (also disabled via tauri zoomHotkeysEnabled)
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
