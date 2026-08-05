import { invoke } from '@tauri-apps/api/core'
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import type { OpResult } from './api'
import { fileName } from './api'

let permissionReady: Promise<boolean> | null = null

async function ensurePermission(): Promise<boolean> {
  if (!permissionReady) {
    permissionReady = (async () => {
      try {
        let granted = await isPermissionGranted()
        if (!granted) {
          granted = (await requestPermission()) === 'granted'
        }
        return granted
      } catch {
        return false
      }
    })()
  }
  return permissionReady
}

/** Native OS notification; clicking it opens Explorer at the output path. */
export async function notifySuccess(result: OpResult, toolLabel = 'Operación'): Promise<void> {
  const path = result.outputPaths[0] ?? ''
  const files = result.outputPaths.length
  const name = path ? fileName(path) : ''
  const body =
    files <= 1
      ? `${toolLabel} lista · ${result.pageCount} pág.${name ? ` · ${name}` : ''}`
      : `${toolLabel} lista · ${files} archivos · ${result.pageCount} pág.`

  try {
    await invoke('notify_done', {
      title: 'MonkeyPDF',
      body: path ? `${body}\n${path}` : body,
      path: path || null,
    })
  } catch (err) {
    console.warn('notify_done failed, falling back', err)
    if (!(await ensurePermission())) return
    sendNotification({
      title: 'MonkeyPDF',
      body,
      largeBody: path || body,
      autoCancel: true,
    })
  }
}

export async function notifyError(message: string): Promise<void> {
  const body = message.length > 120 ? `${message.slice(0, 117)}…` : message
  try {
    await invoke('notify_done', {
      title: 'MonkeyPDF',
      body,
      path: null,
    })
  } catch {
    if (!(await ensurePermission())) return
    sendNotification({
      title: 'MonkeyPDF',
      body,
      autoCancel: true,
    })
  }
}

/** Kept for App.svelte startup; desktop needs no action registration. */
export async function initNotifications(): Promise<void> {
  await ensurePermission()
}
