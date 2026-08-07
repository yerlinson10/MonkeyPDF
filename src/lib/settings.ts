import { LazyStore } from '@tauri-apps/plugin-store'
import type { ToolId } from './api'

export type AiProviderId = 'openai' | 'anthropic' | 'openrouter' | 'ollama'

export type OutputToolId = Exclude<ToolId, 'settings'>

export interface AiSettings {
  provider: AiProviderId
  openaiKey: string
  anthropicKey: string
  openrouterKey: string
  ollamaBaseUrl: string
  openaiModel: string
  anthropicModel: string
  openrouterModel: string
  ollamaModel: string
}

export interface OutputPathSettings {
  /** Carpeta por defecto para todas las herramientas. */
  defaultDir: string
  /** Override por herramienta; vacío = usa defaultDir. */
  toolDirs: Partial<Record<OutputToolId, string>>
}

const DEFAULTS: AiSettings = {
  provider: 'openai',
  openaiKey: '',
  anthropicKey: '',
  openrouterKey: '',
  ollamaBaseUrl: 'http://127.0.0.1:11434',
  openaiModel: 'gpt-4o-mini',
  anthropicModel: 'claude-3-5-haiku-latest',
  openrouterModel: 'openai/gpt-4o-mini',
  ollamaModel: 'llama3.2',
}

const OUTPUT_PATH_DEFAULTS: OutputPathSettings = {
  defaultDir: '',
  toolDirs: {},
}

let store: LazyStore | null = null

function getStore(): LazyStore {
  if (!store) store = new LazyStore('monkeypdf-settings.json')
  return store
}

export async function loadAiSettings(): Promise<AiSettings> {
  try {
    const s = getStore()
    const saved = await s.get<Partial<AiSettings>>('ai')
    return { ...DEFAULTS, ...(saved ?? {}) }
  } catch {
    return { ...DEFAULTS }
  }
}

export async function saveAiSettings(settings: AiSettings): Promise<void> {
  const s = getStore()
  await s.set('ai', settings)
  await s.save()
}

export async function loadOutputPathSettings(): Promise<OutputPathSettings> {
  try {
    const s = getStore()
    const saved = await s.get<Partial<OutputPathSettings>>('outputPaths')
    return {
      defaultDir: saved?.defaultDir ?? OUTPUT_PATH_DEFAULTS.defaultDir,
      toolDirs: { ...(saved?.toolDirs ?? {}) },
    }
  } catch {
    return { ...OUTPUT_PATH_DEFAULTS, toolDirs: {} }
  }
}

export async function saveOutputPathSettings(settings: OutputPathSettings): Promise<void> {
  const s = getStore()
  const toolDirs: Partial<Record<OutputToolId, string>> = {}
  for (const [id, dir] of Object.entries(settings.toolDirs)) {
    const trimmed = dir?.trim()
    if (trimmed) toolDirs[id as OutputToolId] = trimmed
  }
  await s.set('outputPaths', {
    defaultDir: settings.defaultDir.trim(),
    toolDirs,
  } satisfies OutputPathSettings)
  await s.save()
}

/** Resuelve la carpeta de salida: override de la herramienta o la global. */
export async function resolveOutputDir(tool?: OutputToolId): Promise<string> {
  const paths = await loadOutputPathSettings()
  if (tool) {
    const override = paths.toolDirs[tool]?.trim()
    if (override) return override
  }
  return paths.defaultDir.trim()
}

/** Une carpeta + nombre de archivo respetando el separador del SO. */
export function joinOutputPath(dir: string, fileName: string): string {
  const cleanDir = dir.replace(/[/\\]+$/, '')
  if (!cleanDir) return fileName
  const sep = /\\/.test(cleanDir) && !/\//.test(cleanDir) ? '\\' : '/'
  return `${cleanDir}${sep}${fileName}`
}

export function apiKeyFor(settings: AiSettings): string {
  switch (settings.provider) {
    case 'openai':
      return settings.openaiKey
    case 'anthropic':
      return settings.anthropicKey
    case 'openrouter':
      return settings.openrouterKey
    case 'ollama':
      return ''
  }
}

export function modelFor(settings: AiSettings): string {
  switch (settings.provider) {
    case 'openai':
      return settings.openaiModel
    case 'anthropic':
      return settings.anthropicModel
    case 'openrouter':
      return settings.openrouterModel
    case 'ollama':
      return settings.ollamaModel
  }
}

export function baseUrlFor(settings: AiSettings): string | null {
  switch (settings.provider) {
    case 'ollama':
      return settings.ollamaBaseUrl
    case 'openrouter':
      return 'https://openrouter.ai/api/v1'
    default:
      return null
  }
}

export const AI_PROVIDERS: { id: AiProviderId; label: string }[] = [
  { id: 'openai', label: 'OpenAI' },
  { id: 'anthropic', label: 'Anthropic' },
  { id: 'openrouter', label: 'OpenRouter' },
  { id: 'ollama', label: 'Ollama' },
]
