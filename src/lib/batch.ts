import { readDir } from '@tauri-apps/plugin-fs'
import { joinOutputPath } from './settings'
import { fileName } from './api'

/** List PDF files directly inside a directory (non-recursive). */
export async function listPdfsInDir(dir: string): Promise<string[]> {
  const entries = await readDir(dir)
  const out: string[] = []
  for (const e of entries) {
    if (e.isDirectory) continue
    const name = e.name ?? ''
    if (!/\.pdf$/i.test(name)) continue
    out.push(joinOutputPath(dir, name))
  }
  out.sort((a, b) => fileName(a).localeCompare(fileName(b), undefined, { sensitivity: 'base' }))
  return out
}

export function stemFromPath(path: string): string {
  const name = fileName(path)
  return name.replace(/\.pdf$/i, '') || 'out'
}

export function batchSiblingOutput(inputPath: string, outDir: string, suffix: string): string {
  return joinOutputPath(outDir, `${stemFromPath(inputPath)}${suffix}.pdf`)
}
