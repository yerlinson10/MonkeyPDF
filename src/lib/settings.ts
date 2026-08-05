import { LazyStore } from '@tauri-apps/plugin-store'

export type AiProviderId = 'openai' | 'anthropic' | 'openrouter' | 'ollama'

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
