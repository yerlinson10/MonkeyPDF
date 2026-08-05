import { invoke } from '@tauri-apps/api/core'

export interface OpResult {
  outputPaths: string[]
  pageCount: number
  elapsedMs: number
}

export type ToolId =
  | 'merge'
  | 'split'
  | 'rotate'
  | 'compress'
  | 'pdf-to-jpg'
  | 'jpg-to-pdf'

export interface ToolMeta {
  id: ToolId
  title: string
  short: string
  description: string
  accept: string
}

export const TOOLS: ToolMeta[] = [
  {
    id: 'merge',
    title: 'Unir PDF',
    short: 'Varios → uno',
    description: 'Combina varios PDFs en un solo archivo, en el orden que elijas.',
    accept: '.pdf',
  },
  {
    id: 'split',
    title: 'Dividir PDF',
    short: 'Por rangos',
    description: 'Extrae rangos de páginas a nuevos PDFs.',
    accept: '.pdf',
  },
  {
    id: 'rotate',
    title: 'Rotar PDF',
    short: '90° / 180° / 270°',
    description: 'Rota páginas concretas o el documento completo.',
    accept: '.pdf',
  },
  {
    id: 'compress',
    title: 'Comprimir PDF',
    short: 'Menos peso',
    description: 'Reduce el tamaño recomprimiendo imágenes o rasterizando páginas a JPEG.',
    accept: '.pdf',
  },
  {
    id: 'pdf-to-jpg',
    title: 'PDF a JPG',
    short: 'Páginas → imagen',
    description: 'Renderiza cada página como JPG a la resolución que indiques.',
    accept: '.pdf',
  },
  {
    id: 'jpg-to-pdf',
    title: 'JPG a PDF',
    short: 'Imágenes → PDF',
    description: 'Empaqueta imágenes en un PDF A4 con ajuste proporcional.',
    accept: '.jpg,.jpeg,.png,.webp',
  },
]

export async function mergePdfs(paths: string[], output: string): Promise<OpResult> {
  return invoke('merge_pdfs', { paths, output })
}

export async function splitPdf(
  path: string,
  ranges: Array<[number, number]>,
  outputDir: string,
): Promise<OpResult> {
  return invoke('split_pdf', { path, ranges, outputDir })
}

export async function rotatePdf(
  path: string,
  angle: number,
  pages: number[] | null,
  output: string,
): Promise<OpResult> {
  return invoke('rotate_pdf', { path, angle, pages, output })
}

export async function compressPdf(
  path: string,
  quality: number,
  output: string,
): Promise<OpResult> {
  return invoke('compress_pdf', { path, quality, output })
}

export async function pdfToJpg(
  path: string,
  dpi: number,
  outputDir: string,
): Promise<OpResult> {
  return invoke('pdf_to_jpg', { path, dpi, outputDir })
}

export async function jpgToPdf(paths: string[], output: string): Promise<OpResult> {
  return invoke('jpg_to_pdf', { paths, output })
}

export async function getPdfPageCount(path: string): Promise<number> {
  return invoke('get_pdf_page_count', { path })
}

export interface FilePreview {
  dataUrl: string
  pageCount: number
  page: number
  kind: string
}

export async function previewPdf(
  path: string,
  page = 1,
  maxWidth = 480,
): Promise<FilePreview> {
  return invoke('preview_pdf', { path, page, maxWidth })
}

export async function previewImage(path: string, maxWidth = 480): Promise<FilePreview> {
  return invoke('preview_image', { path, maxWidth })
}

export function isPdfPath(path: string): boolean {
  return path.toLowerCase().endsWith('.pdf')
}

export function isImagePath(path: string): boolean {
  return /\.(jpe?g|png|webp)$/i.test(path)
}

export async function previewFile(
  path: string,
  page = 1,
  maxWidth = 480,
): Promise<FilePreview> {
  if (isPdfPath(path)) return previewPdf(path, page, maxWidth)
  if (isImagePath(path)) return previewImage(path, maxWidth)
  throw new Error('Unsupported preview type')
}

export function fileName(path: string): string {
  const parts = path.replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || path
}

/** Open a file (selected) or folder in the system file explorer. */
export async function revealInExplorer(path: string): Promise<void> {
  await invoke('reveal_in_explorer', { path })
}
