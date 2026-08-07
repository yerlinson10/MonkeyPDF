import { invoke } from '@tauri-apps/api/core'

export interface OpResult {
  outputPaths: string[]
  pageCount: number
  elapsedMs: number
}

export interface AiResult {
  text: string
  provider: string
  elapsedMs: number
  sourceChars: number
}

export type ToolId =
  | 'merge'
  | 'split'
  | 'organize'
  | 'rotate'
  | 'compress'
  | 'pdf-to-jpg'
  | 'jpg-to-pdf'
  | 'protect'
  | 'repair'
  | 'page-numbers'
  | 'office'
  | 'markdown'
  | 'ai'
  | 'ocr'
  | 'redact'
  | 'crop'
  | 'watermark'
  | 'compare'
  | 'sign'
  | 'settings'

export interface ToolMeta {
  id: ToolId
  title: string
  short: string
  description: string
  accept: string
  group?: 'core' | 'suite' | 'advanced' | 'ai'
}

export const TOOLS: ToolMeta[] = [
  {
    id: 'merge',
    title: 'Unir PDF',
    short: 'Varios → uno',
    description: 'Combina varios PDFs en un solo archivo, en el orden que elijas.',
    accept: '.pdf',
    group: 'core',
  },
  {
    id: 'split',
    title: 'Dividir PDF',
    short: 'Por rangos',
    description: 'Extrae rangos de páginas a nuevos PDFs.',
    accept: '.pdf',
    group: 'core',
  },
  {
    id: 'organize',
    title: 'Ordenar',
    short: 'Páginas / archivos',
    description: 'Reordena, rota o elimina páginas; mezcla varios PDFs en un solo documento.',
    accept: '.pdf',
    group: 'core',
  },
  {
    id: 'rotate',
    title: 'Rotar PDF',
    short: '90° / 180° / 270°',
    description: 'Rota páginas concretas o el documento completo.',
    accept: '.pdf',
    group: 'core',
  },
  {
    id: 'compress',
    title: 'Comprimir PDF',
    short: 'Menos peso',
    description: 'Reduce el tamaño recomprimiendo imágenes o rasterizando páginas a JPEG.',
    accept: '.pdf',
    group: 'core',
  },
  {
    id: 'pdf-to-jpg',
    title: 'PDF a JPG',
    short: 'Páginas → imagen',
    description: 'Renderiza cada página como JPG a la resolución que indiques.',
    accept: '.pdf',
    group: 'core',
  },
  {
    id: 'jpg-to-pdf',
    title: 'JPG a PDF',
    short: 'Imágenes → PDF',
    description: 'Empaqueta imágenes en un PDF con ajuste proporcional.',
    accept: '.jpg,.jpeg,.png,.webp',
    group: 'core',
  },
  {
    id: 'protect',
    title: 'Proteger',
    short: 'Clave / unlock',
    description: 'Añade o quita contraseña de apertura (RC4 128-bit).',
    accept: '.pdf',
    group: 'suite',
  },
  {
    id: 'repair',
    title: 'Reparar',
    short: 'Diagnóstico',
    description: 'Diagnostica y repara PDFs dañados (xref, streams, huérfanos) con re-guardado limpio.',
    accept: '.pdf',
    group: 'suite',
  },
  {
    id: 'page-numbers',
    title: 'Numerar',
    short: 'Página N',
    description: 'Sella números de página en cada hoja.',
    accept: '.pdf',
    group: 'suite',
  },
  {
    id: 'office',
    title: 'Office',
    short: 'DOCX / XLSX / PPTX',
    description: 'Convierte Word, Excel, PowerPoint o HTML ↔ PDF con LibreOffice.',
    accept: '.pdf,.doc,.docx,.xls,.xlsx,.ppt,.pptx,.html,.htm,.odt,.ods,.odp',
    group: 'suite',
  },
  {
    id: 'ocr',
    title: 'OCR',
    short: 'Escaneo → texto',
    description: 'Reconoce texto con Tesseract del sistema (Markdown, TXT o PDF buscable).',
    accept: '.pdf',
    group: 'advanced',
  },
  {
    id: 'redact',
    title: 'Censura',
    short: 'Tachar zonas',
    description: 'Cubre zonas con negro permanente: aplana la página (sin texto ni campos debajo).',
    accept: '.pdf',
    group: 'advanced',
  },
  {
    id: 'crop',
    title: 'Recorte',
    short: 'CropBox',
    description: 'Recorta páginas fijando CropBox y MediaBox al área seleccionada.',
    accept: '.pdf',
    group: 'advanced',
  },
  {
    id: 'watermark',
    title: 'Marca de agua',
    short: 'Texto / imagen',
    description: 'Inserta marca de agua de texto o imagen con posición, mosaico, transparencia y rotación.',
    accept: '.pdf',
    group: 'advanced',
  },
  {
    id: 'compare',
    title: 'Comparar',
    short: 'A vs B',
    description: 'Compara A vs B en paralelo: texto, mapa visual y scroll sincronizado.',
    accept: '.pdf',
    group: 'advanced',
  },
  {
    id: 'sign',
    title: 'Firmar',
    short: 'Firma / formulario',
    description:
      'Crea firma e iniciales, arrástralas al PDF, rellena campos de formulario y hornea el resultado.',
    accept: '.pdf',
    group: 'advanced',
  },
  {
    id: 'markdown',
    title: 'Markdown',
    short: 'PDF → MD',
    description: 'Extrae texto a Markdown con heurísticas de títulos y tablas.',
    accept: '.pdf',
    group: 'ai',
  },
  {
    id: 'ai',
    title: 'IA',
    short: 'Resumir / traducir',
    description:
      'Resumen o traducción con tu API key (OpenAI, Anthropic, OpenRouter u Ollama).',
    accept: '.pdf',
    group: 'ai',
  },
  {
    id: 'settings',
    title: 'Ajustes',
    short: 'Configuración',
    description:
      'Rutas de salida e IA. Las claves se guardan localmente y solo salen al llamar al proveedor.',
    accept: '',
    group: 'ai',
  },
]

/** Tools shown in the side rail (settings lives in the fixed foot icon). */
export const RAIL_TOOLS: ToolMeta[] = TOOLS.filter((t) => t.id !== 'settings')

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

export async function protectPdf(
  path: string,
  userPassword: string,
  ownerPassword: string | null,
  output: string,
): Promise<OpResult> {
  return invoke('protect_pdf', {
    path,
    userPassword,
    ownerPassword,
    output,
  })
}

export async function unlockPdf(
  path: string,
  password: string,
  output: string,
): Promise<OpResult> {
  return invoke('unlock_pdf', { path, password, output })
}

export async function addPageNumbers(
  path: string,
  output: string,
  position: string,
  format: string | null,
  startFrom: number | null,
  fontSize: number | null,
): Promise<OpResult> {
  return invoke('add_page_numbers', {
    path,
    output,
    position,
    format,
    startFrom,
    fontSize,
  })
}

export async function convertOffice(
  path: string,
  target: string,
  outputDir: string,
): Promise<OpResult> {
  return invoke('convert_office', { path, target, outputDir })
}

export async function checkLibreOffice(): Promise<boolean> {
  return invoke('check_libreoffice')
}

export async function checkTesseract(): Promise<boolean> {
  return invoke('check_tesseract')
}

export async function ocrPdf(
  path: string,
  output: string,
  lang: string | null,
  mode: string | null,
): Promise<OpResult> {
  return invoke('ocr_pdf', { path, output, lang, mode })
}

export interface RedactRegion {
  page: number
  x: number
  y: number
  w: number
  h: number
}

export async function redactPdf(
  path: string,
  output: string,
  regions: RedactRegion[],
): Promise<OpResult> {
  return invoke('redact_pdf', { path, output, regions })
}

export interface CropBox {
  x: number
  y: number
  w: number
  h: number
}

export async function cropPdf(
  path: string,
  output: string,
  crop: CropBox,
  pages: number[] | null,
): Promise<OpResult> {
  return invoke('crop_pdf', { path, output, crop, pages })
}

export async function comparePdfs(
  pathA: string,
  pathB: string,
  outputDir: string,
  mode: string | null,
): Promise<OpResult> {
  return invoke('compare_pdfs', { pathA, pathB, outputDir, mode })
}

export interface TextChange {
  page: number
  kind: 'only_a' | 'only_b' | 'changed' | string
  textA: string
  textB: string
}

export interface DiffUnderline {
  /** Normalized 0–1, top-left origin */
  x: number
  y: number
  w: number
  h: number
}

export interface VisualPageDiff {
  page: number
  changedPx: number
  diffDataUrl: string
  underlines: DiffUnderline[]
}

export interface CompareReport {
  pagesA: number
  pagesB: number
  textChanges: TextChange[]
  visualPages: VisualPageDiff[]
  elapsedMs: number
  outputPaths: string[]
}

export async function compareReport(
  pathA: string,
  pathB: string,
  mode: string | null,
  exportDir: string | null = null,
): Promise<CompareReport> {
  return invoke('compare_report', { pathA, pathB, mode, exportDir })
}

export interface PageMediaBox {
  x: number
  y: number
  width: number
  height: number
}

export async function getPageMediabox(path: string, page: number): Promise<PageMediaBox> {
  return invoke('get_page_mediabox', { path, page })
}

export type SignatureKind = 'signature' | 'initials' | 'logo'
export type SignatureMethod = 'type' | 'draw' | 'upload'

export interface SignatureAsset {
  id: string
  kind: SignatureKind
  name: string | null
  method: SignatureMethod
  font: string | null
  color: string | null
  pngDataUrl: string
  source: Record<string, unknown>
}

export interface NewSignatureAsset {
  id?: string | null
  kind: SignatureKind
  name?: string | null
  method: SignatureMethod
  font?: string | null
  color?: string | null
  pngDataUrl: string
  source?: Record<string, unknown>
}

export async function listSignatures(): Promise<SignatureAsset[]> {
  return invoke('list_signatures')
}

export async function saveSignature(asset: NewSignatureAsset): Promise<SignatureAsset> {
  return invoke('save_signature', { asset })
}

export async function deleteSignature(id: string): Promise<void> {
  return invoke('delete_signature', { id })
}

export type FormFieldKind =
  | 'text'
  | 'checkbox'
  | 'radio'
  | 'choice'
  | 'signature'
  | 'unknown'

export interface FormField {
  page: number
  name: string
  kind: FormFieldKind
  x: number
  y: number
  w: number
  h: number
  value: string
  options: string[]
}

export async function getFormFields(path: string): Promise<FormField[]> {
  return invoke('get_form_fields', { path })
}

export interface SignPlacement {
  assetId?: string | null
  pngBytes?: number[] | null
  pngDataUrl?: string | null
  page: number
  x: number
  y: number
  w: number
  h: number
}

export interface FieldFill {
  name: string
  value: string
}

export async function signPdf(
  path: string,
  output: string,
  placements: SignPlacement[],
  formFills: FieldFill[],
): Promise<OpResult> {
  return invoke('sign_pdf', { path, output, placements, formFills })
}

export interface Diagnosis {
  encrypted: boolean
  pdfVersion: string
  pageCount: number
  hasXrefStream: boolean
  hasEof: boolean
  brokenObjects: number
  orphanObjects: number
  missingPages: number
  linearized: boolean
  warnings: string[]
}

export async function diagnosePdf(path: string): Promise<Diagnosis> {
  return invoke('diagnose_pdf', { path })
}

export async function repairPdf(
  path: string,
  output: string,
  password: string | null,
): Promise<OpResult> {
  return invoke('repair_pdf', { path, output, password })
}

export interface WatermarkSpec {
  mode: 'text' | 'image'
  text?: string | null
  font?: string | null
  size?: number | null
  bold?: boolean
  italic?: boolean
  underline?: boolean
  color?: string | null
  imagePath?: string | null
  position: number
  mosaic: boolean
  transparency: number
  rotation: number
  pageFrom?: number | null
  pageTo?: number | null
  layer: 'above' | 'below'
}

export async function watermarkPdf(
  path: string,
  output: string,
  spec: WatermarkSpec,
): Promise<OpResult> {
  return invoke('watermark_pdf', { path, output, spec })
}

export interface OrganizePageRef {
  sourcePath: string
  page: number
  rotate?: number
}

export async function organizePdf(
  pages: OrganizePageRef[],
  output: string,
): Promise<OpResult> {
  return invoke('organize_pdf', { pages, output })
}

/** Normalized rect (0–1, top-left) → PDF points (bottom-left). */
export function normRectToPdf(
  nx: number,
  ny: number,
  nw: number,
  nh: number,
  box: PageMediaBox,
): { x: number; y: number; w: number; h: number } {
  return {
    x: box.x + nx * box.width,
    y: box.y + (1 - ny - nh) * box.height,
    w: nw * box.width,
    h: nh * box.height,
  }
}

export async function normRectsToPdfRegions(
  pdfPath: string,
  rects: Array<{ page: number; nx: number; ny: number; nw: number; nh: number }>,
): Promise<RedactRegion[]> {
  const byPage = new Map<number, typeof rects>()
  for (const r of rects) {
    const list = byPage.get(r.page) ?? []
    list.push(r)
    byPage.set(r.page, list)
  }
  const out: RedactRegion[] = []
  for (const [p, list] of byPage) {
    const box = await getPageMediabox(pdfPath, p)
    for (const r of list) {
      const pts = normRectToPdf(r.nx, r.ny, r.nw, r.nh, box)
      out.push({ page: r.page, ...pts })
    }
  }
  return out
}

export async function pdfToMarkdown(path: string, output: string): Promise<OpResult> {
  return invoke('pdf_to_markdown', { path, output })
}

export async function aiProcessPdf(args: {
  path: string
  action: string
  provider: string
  apiKey: string
  model?: string | null
  targetLang?: string | null
  baseUrl?: string | null
}): Promise<AiResult> {
  return invoke('ai_process_pdf', {
    path: args.path,
    action: args.action,
    provider: args.provider,
    apiKey: args.apiKey,
    model: args.model ?? null,
    targetLang: args.targetLang ?? null,
    baseUrl: args.baseUrl ?? null,
  })
}

export async function writeTextFile(path: string, content: string): Promise<OpResult> {
  return invoke('write_text_file', { path, content })
}

export async function getPdfPageCount(path: string): Promise<number> {
  return invoke('get_pdf_page_count', { path })
}

export interface PreviewTextSpan {
  text: string
  x: number
  y: number
  w: number
  h: number
}

export interface FilePreview {
  dataUrl: string
  pageCount: number
  page: number
  kind: string
  textSpans?: PreviewTextSpan[]
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

export async function revealInExplorer(path: string): Promise<void> {
  await invoke('reveal_in_explorer', { path })
}
