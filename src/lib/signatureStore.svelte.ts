import {
  deleteSignature as apiDelete,
  listSignatures,
  saveSignature as apiSave,
  type NewSignatureAsset,
  type SignatureAsset,
} from './api'

let assets = $state<SignatureAsset[]>([])
let loaded = $state(false)
let loading = $state(false)
let error = $state<string | null>(null)

export function getSignatureAssets(): SignatureAsset[] {
  return assets
}

export function signatureAssetsLoaded(): boolean {
  return loaded
}

export function signatureStoreError(): string | null {
  return error
}

export async function refreshSignatures(): Promise<SignatureAsset[]> {
  loading = true
  error = null
  try {
    assets = await listSignatures()
    loaded = true
    return assets
  } catch (e) {
    error = String(e)
    throw e
  } finally {
    loading = false
  }
}

export async function saveSignatureAsset(input: NewSignatureAsset): Promise<SignatureAsset> {
  const saved = await apiSave(input)
  const idx = assets.findIndex((a) => a.id === saved.id)
  if (idx >= 0) {
    assets = [...assets.slice(0, idx), saved, ...assets.slice(idx + 1)]
  } else {
    assets = [...assets, saved]
  }
  return saved
}

export async function removeSignatureAsset(id: string): Promise<void> {
  await apiDelete(id)
  assets = assets.filter((a) => a.id !== id)
}

export function assetById(id: string): SignatureAsset | undefined {
  return assets.find((a) => a.id === id)
}

export function assetsOfKind(kind: SignatureAsset['kind']): SignatureAsset[] {
  return assets.filter((a) => a.kind === kind)
}

export function isSignatureStoreLoading(): boolean {
  return loading
}

/** Derive initials from a full name (first letter of each word). */
export function initialsFromName(name: string): string {
  return name
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .map((w) => w[0]!.toUpperCase())
    .join('')
    .slice(0, 4)
}
