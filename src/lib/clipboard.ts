/** System clipboard via Tauri (WebView clipboard is unreliable). */
import {
  readText as tauriReadText,
  writeText as tauriWriteText,
} from '@tauri-apps/plugin-clipboard-manager'

export async function copyText(text: string): Promise<boolean> {
  const value = text ?? ''
  if (!value) return false

  try {
    await tauriWriteText(value)
    return true
  } catch (err) {
    console.warn('Tauri clipboard write failed, trying browser API', err)
  }

  try {
    await navigator.clipboard.writeText(value)
    return true
  } catch {
    /* fall through */
  }

  try {
    const ta = document.createElement('textarea')
    ta.value = value
    ta.setAttribute('readonly', '')
    ta.style.position = 'fixed'
    ta.style.left = '-9999px'
    document.body.appendChild(ta)
    ta.focus()
    ta.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(ta)
    return ok
  } catch {
    return false
  }
}

export async function readClipboardText(): Promise<string> {
  try {
    return (await tauriReadText()) ?? ''
  } catch {
    try {
      return (await navigator.clipboard.readText()) ?? ''
    } catch {
      return ''
    }
  }
}

export function selectedText(): string {
  return window.getSelection()?.toString() ?? ''
}
