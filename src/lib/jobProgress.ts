import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'

export interface JobProgress {
  current: number
  total: number
  label: string
}

export async function cancelJob(): Promise<void> {
  await invoke('cancel_job')
}

export async function clearJob(): Promise<void> {
  await invoke('clear_job')
}

/** Subscribe while a tool run is in flight; returns an unsubscribe fn. */
export async function subscribeJobProgress(
  onProgress: (p: JobProgress) => void,
): Promise<() => void> {
  const unlisten = await listen<JobProgress>('job-progress', (event) => {
    onProgress(event.payload)
  })
  return () => {
    unlisten()
  }
}

/** Listen for progress events for the duration of `fn`. */
export async function runWithProgress<T>(
  setProgress: (p: JobProgress | null) => void,
  fn: () => Promise<T>,
): Promise<T> {
  setProgress(null)
  await clearJob()
  const unsub = await subscribeJobProgress(setProgress)
  try {
    return await fn()
  } finally {
    unsub()
  }
}

/** Dispatch Ctrl+Enter / primary CTA hotkey to the active tool. */
export function dispatchToolRun() {
  window.dispatchEvent(new CustomEvent('mp-run'))
}

export function dispatchToolOpen() {
  window.dispatchEvent(new CustomEvent('mp-open'))
}
