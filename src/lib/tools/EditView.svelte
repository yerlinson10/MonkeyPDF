<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { readFile } from '@tauri-apps/plugin-fs'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import Icon from '../components/Icon.svelte'
  import {
    editListText,
    editPdf,
    getFormFields,
    getPageMediabox,
    getPdfPageCount,
    normRectToPdf,
    previewPdf,
    type EditOp,
    type FormField,
    type OpResult,
    type PageMediaBox,
    type TextRun,
  } from '../api'
  import { needsRerender, previewRenderWidth, renderWidthForCss } from '../previewScale'
  import { attachMiddlePan } from '../panScroll'
  import { runWithProgress, type JobProgress } from '../jobProgress'
  import { fileName } from '../history'

  /* Hallmark · component: pdf-editor · genre: playful-técnico · theme: press-shop banana
   * states: default · hover · focus · active · disabled · loading · error · success
   * contrast: pass
   */

  type Mode =
    | 'edit'
    | 'text'
    | 'annotate'
    | 'shapes'
    | 'draw'
    | 'image'
    | 'stamp'
    | 'whiteout'
    | 'form'

  type AnnotSub = 'highlight' | 'underline' | 'strikeout' | 'note'
  type ShapeSub = 'rect' | 'ellipse' | 'line'
  type Handle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w'

  type Kind =
    | 'addText'
    | 'replaceText'
    | 'highlight'
    | 'underline'
    | 'strikeout'
    | 'note'
    | 'rect'
    | 'ellipse'
    | 'line'
    | 'freeDraw'
    | 'whiteout'
    | 'image'
    | 'stamp'

  interface EditObject {
    id: string
    kind: Kind
    page: number
    label: string
    /** Normalized 0–1 against the page bitmap, origin top-left */
    nx?: number
    ny?: number
    nw?: number
    nh?: number
    text?: string
    runId?: number
    /** Sibling operators of an edited line, blanked on save. */
    clearRunIds?: number[]
    /** Points of horizontal room the replacement may use. */
    fitWidth?: number
    font?: string
    size?: number
    bold?: boolean
    italic?: boolean
    color?: string
    align?: string
    /** Rich paragraph: each inner array is one visual line of styled spans. */
    richLines?: RichSpan[][]
    /** Per-line plain text for surgical in-place replacement (preserves PDF font). */
    lineTexts?: string[]
    /** Absolute PDF-space fragments to bake on save (one addText each). */
    bakeSpans?: BakeSpan[]
    opacity?: number
    stroke?: string
    fill?: string | null
    strokeWidth?: number
    from?: [number, number]
    to?: [number, number]
    arrow?: string
    paths?: Array<Array<[number, number]>>
    imagePath?: string
    rotation?: number
    stamp?: string
    customText?: string
  }

  interface RichSpan {
    text: string
    font: string
    size: number
    bold: boolean
    italic: boolean
    color: string
  }

  interface BakeSpan {
    x: number
    y: number
    w: number
    h: number
    text: string
    font: string
    size: number
    bold: boolean
    italic: boolean
    color: string
  }

  /** Anything that can be laid out as a stretch of text on the page. */
  interface TextAnchor {
    x: number
    y: number
    w: number
    h: number
    fontName: string
    text: string
  }

  /**
   * A visual line of the document: several PDF text-showing operators that sit
   * on the same baseline. Editing works on these rather than on raw runs, so a
   * click gives you the whole sentence instead of a stray fragment.
   */
  interface TextLine extends TextAnchor {
    id: number
    runIds: number[]
    color: string
    editable: boolean
  }

  type Drag =
    | { kind: 'create'; ox: number; oy: number }
    | { kind: 'draw'; points: Array<[number, number]> }
    | { kind: 'move'; id: string; ox: number; oy: number; snx: number; sny: number }
    | {
        kind: 'moveLine'
        id: string
        ox: number
        oy: number
        sfrom: [number, number]
        sto: [number, number]
      }
    | {
        kind: 'resize'
        id: string
        handle: Handle
        ox: number
        oy: number
        snx: number
        sny: number
        snw: number
        snh: number
      }
    | { kind: 'endpoint'; id: string; which: 'from' | 'to' }

  const MODES: { id: Mode; label: string; hint: string }[] = [
    {
      id: 'edit',
      label: 'Editar texto',
      hint: 'Clic en un párrafo. Selecciona palabras y formatea solo esa selección (B, color, tamaño, fuente).',
    },
    { id: 'text', label: 'Añadir texto', hint: 'Clic donde quieras escribir. Arrastra para fijar el ancho.' },
    { id: 'annotate', label: 'Anotar', hint: 'Arrastra sobre el texto, o haz clic directamente en una línea.' },
    { id: 'shapes', label: 'Formas', hint: 'Arrastra para dibujar la forma.' },
    { id: 'draw', label: 'Dibujar', hint: 'Arrastra a mano alzada.' },
    { id: 'image', label: 'Imagen', hint: 'Clic para elegir una imagen y colocarla.' },
    { id: 'stamp', label: 'Sello', hint: 'Clic para estampar.' },
    { id: 'whiteout', label: 'Borrar', hint: 'Arrastra sobre lo que quieras tapar en blanco.' },
    { id: 'form', label: 'Formulario', hint: 'Rellena los campos del panel izquierdo.' },
  ]

  const STAMPS = [
    'aprobado',
    'rechazado',
    'confidencial',
    'borrador',
    'firmado',
    'urgente',
    'copia',
    'original',
  ]

  const COLORS = ['#1a1a1a', '#e11d48', '#2563eb', '#16a34a', '#f59e0b', '#7c3aed', '#ffffff']
  const HIGHLIGHTS = ['#ffe066', '#a7f3d0', '#bfdbfe', '#fbcfe8', '#fed7aa']
  const HANDLES: Handle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']

  const MIN_SIZE = 0.008
  const CLICK_SLOP = 0.012

  let paths = $state<string[]>([])
  let output = $state('')
  let pageCount = $state(1)
  let page = $state(1)
  let previewUrl = $state('')
  let previewLoading = $state(false)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let progress = $state<JobProgress | null>(null)
  let zoom = $state(1)
  // Not reactive on purpose: it feeds the render request, never the markup.
  let renderWidth = previewRenderWidth(1)
  let rerenderTimer: ReturnType<typeof setTimeout> | null = null

  let mode = $state<Mode>('edit')
  let annotSub = $state<AnnotSub>('highlight')
  let shapeSub = $state<ShapeSub>('rect')
  let objects = $state<EditObject[]>([])
  let selectedId = $state<string | null>(null)
  let undoStack = $state<EditObject[][]>([])
  let redoStack = $state<EditObject[][]>([])

  let textRuns = $state<TextRun[]>([])
  let formFields = $state<FormField[]>([])
  let formValues = $state<Record<string, string>>({})
  let mediabox = $state<PageMediaBox | null>(null)
  let flatten = $state(false)
  let previewFresh = $state(false)
  let previewTmpPath = $state('')
  let previewRefreshTimer: ReturnType<typeof setTimeout> | null = null

  let font = $state('Helvetica')
  let fontSize = $state(12)
  let bold = $state(false)
  let italic = $state(false)
  let textColor = $state('#1a1a1a')
  let align = $state('left')
  let strokeColor = $state('#e11d48')
  let fillColor = $state<string | null>(null)
  let strokeWidth = $state(1.5)
  let opacity = $state(1)
  let stampKind = $state('aprobado')
  let stampCustom = $state('')
  let highlightColor = $state('#ffe066')

  let viewportEl = $state<HTMLDivElement | null>(null)
  let surfaceEl = $state<HTMLDivElement | null>(null)
  let surfaceW = $state(0)
  let drag = $state<Drag | null>(null)
  let draftBox = $state<{ nx: number; ny: number; nw: number; nh: number } | null>(null)
  let draftPath = $state<Array<[number, number]> | null>(null)
  let editingRunId = $state<number | null>(null)
  let editingObjId = $state<string | null>(null)
  let editDraft = $state('')
  let editEl = $state<HTMLElement | null>(null)
  let editRichSeed = $state('')
  let savedEditRange: Range | null = null
  let imageUrls = $state<Record<string, string>>({})

  const path = $derived(paths[0] ?? '')
  const selected = $derived(objects.find((o) => o.id === selectedId) ?? null)
  const pageObjects = $derived(objects.filter((o) => o.page === page))
  const surfaceH = $derived(
    mediabox && surfaceW ? (surfaceW * mediabox.height) / mediabox.width : 0,
  )
  const pageReplacements = $derived(
    objects.filter((o) => o.kind === 'replaceText' && o.page === page),
  )
  const pageFields = $derived(formFields.filter((f) => f.page === page))
  const textLines = $derived(buildLines(textRuns))
  const paragraphs = $derived(buildParagraphs(textLines))
  const activeHint = $derived(MODES.find((m) => m.id === mode)?.hint ?? '')
  const replacedRunIds = $derived(
    new Set(
      objects
        .filter((o) => o.kind === 'replaceText' && o.page === page && o.runId != null)
        .map((o) => o.runId as number),
    ),
  )
  const dirty = $derived(
    objects.length > 0 ||
      formFields.some((f) => formValues[f.name] != null && formValues[f.name] !== f.value),
  )

  $effect(() => {
    const el = viewportEl
    if (!el) return
    return attachMiddlePan(el)
  })

  onMount(() => {
    const onRun = () => void run()
    window.addEventListener('mp-run', onRun)
    return () => window.removeEventListener('mp-run', onRun)
  })

  $effect(() => {
    if (path) void loadDocument(path)
    else resetDocument()
  })

  function resetDocument() {
    pageCount = 1
    page = 1
    previewUrl = ''
    textRuns = []
    formFields = []
    formValues = {}
    objects = []
    undoStack = []
    redoStack = []
    selectedId = null
    editingRunId = null
    editingObjId = null
  }

  async function loadDocument(p: string) {
    previewLoading = true
    error = null
    zoom = 1
    renderWidth = previewRenderWidth(1)
    try {
      pageCount = await getPdfPageCount(p)
      page = 1
      await loadPage(p, 1)
      const fields = await getFormFields(p)
      formFields = fields
      const vals: Record<string, string> = {}
      for (const f of fields) {
        if (f.kind !== 'signature') vals[f.name] = f.value
      }
      formValues = vals
    } catch (e) {
      error = String(e)
    } finally {
      previewLoading = false
    }
  }

  async function loadPage(p: string, pg: number) {
    const [prev, box] = await Promise.all([previewPdf(p, pg, renderWidth), getPageMediabox(p, pg)])
    previewUrl = prev.dataUrl
    mediabox = box
    try {
      textRuns = await editListText(p, pg)
    } catch (e) {
      textRuns = []
      console.error('editListText falló:', e)
    }
  }

  /**
   * Re-render the current page with all pending edits applied to a temp file,
   * so the preview shows the exact font from the PDF (surgical_replace keeps
   * the original font). The on-screen overlay can't match embedded fonts, so
   * we let the real render speak for itself.
   */
  async function refreshPreview() {
    if (!path) return
    const tmp = path.replace(/\.pdf$/i, '') + '.preview.tmp.pdf'
    try {
      const ops = await buildAllOps()
      if (ops.length) {
        await editPdf(path, tmp, ops, false)
        previewTmpPath = tmp
        const prev = await previewPdf(tmp, page, renderWidth)
        previewUrl = prev.dataUrl
        previewFresh = true
      } else {
        previewTmpPath = ''
        const prev = await previewPdf(path, page, renderWidth)
        previewUrl = prev.dataUrl
        previewFresh = true
      }
    } catch (e) {
      console.error('refreshPreview falló:', e)
    }
  }

  function schedulePreviewRefresh() {
    if (previewRefreshTimer) clearTimeout(previewRefreshTimer)
    previewRefreshTimer = setTimeout(() => {
      previewRefreshTimer = null
      void refreshPreview()
    }, 350)
  }

  async function buildAllOps(): Promise<EditOp[]> {
    const ops: EditOp[] = []
    const boxCache = new Map<number, PageMediaBox>()
    async function boxFor(pg: number) {
      const hit = boxCache.get(pg)
      if (hit) return hit
      const b = await getPageMediabox(path, pg)
      boxCache.set(pg, b)
      return b
    }
    for (const [name, value] of Object.entries(formValues)) {
      const f = formFields.find((x) => x.name === name)
      if (f && value !== f.value) ops.push({ op: 'formFill', field: name, value })
    }
    for (const o of objects) {
      const box = await boxFor(o.page)
      if (o.kind === 'replaceText' && o.runId != null && o.text != null) {
        if (o.lineTexts?.length) {
          const para = paragraphs.find((p) => p.id === o.runId)
          const lines = para?.lines ?? []
          for (let i = 0; i < lines.length; i++) {
            const line = lines[i]
            const text = o.lineTexts[i] ?? ''
            ops.push({ op: 'replaceText', page: o.page, runId: line.runIds[0], newText: text, fitWidth: line.w })
            for (const id of line.runIds.slice(1)) {
              ops.push({ op: 'replaceText', page: o.page, runId: id, newText: '' })
            }
          }
        } else if (o.bakeSpans?.length) {
          for (const id of o.clearRunIds ?? []) ops.push({ op: 'replaceText', page: o.page, runId: id, newText: '' })
          const r = normRectToPdf(o.nx ?? 0, o.ny ?? 0, o.nw ?? 0.1, o.nh ?? 0.02, box)
          ops.push({ op: 'whiteout', page: o.page, x: r.x - 0.5, y: r.y - 0.5, w: r.w + 1, h: r.h + 1, color: '#ffffff' })
          for (const s of o.bakeSpans) {
            ops.push({ op: 'addText', page: o.page, x: s.x, y: s.y, w: Math.max(s.w, 8), h: s.h, text: s.text, font: s.font, size: s.size, bold: s.bold, italic: s.italic, color: s.color, align: 'left', opacity: 1 })
          }
        } else if (o.font == null && o.bold == null && o.italic == null && !o.text.includes('\n')) {
          ops.push({ op: 'replaceText', page: o.page, runId: o.runId, newText: o.text, fitWidth: o.fitWidth })
          for (const id of o.clearRunIds ?? []) {
            if (id === o.runId) continue
            ops.push({ op: 'replaceText', page: o.page, runId: id, newText: '' })
          }
        } else {
          for (const id of o.clearRunIds ?? []) ops.push({ op: 'replaceText', page: o.page, runId: id, newText: '' })
          const r = normRectToPdf(o.nx ?? 0, o.ny ?? 0, o.nw ?? 0.1, o.nh ?? 0.02, box)
          ops.push({ op: 'whiteout', page: o.page, x: r.x, y: r.y, w: r.w, h: r.h, color: '#ffffff' })
          ops.push({ op: 'addText', page: o.page, x: r.x, y: r.y, w: r.w, h: r.h, text: o.text, font: o.font ?? 'Helvetica', size: o.size, bold: o.bold, italic: o.italic, color: o.color, align: o.align, opacity: 1 })
        }
      } else if (o.kind === 'addText' && o.nx != null) {
        const nx = o.nx, ny = o.ny ?? 0, nw = o.nw ?? 0.1, nh = o.nh ?? 0.02
        const r = normRectToPdf(nx, ny, nw, nh, box)
        ops.push({ op: 'addText', page: o.page, x: r.x, y: r.y, w: r.w, h: r.h, text: o.text ?? '', font: o.font ?? 'Helvetica', size: o.size ?? 12, bold: o.bold ?? false, italic: o.italic ?? false, color: o.color ?? '#1a1a1a', align: o.align ?? 'left', opacity: o.opacity ?? 1 })
      }
    }
    return ops
  }

  async function setPage(pg: number) {
    if (!path || pg < 1 || pg > pageCount || pg === page) return
    commitObjEdit()
    commitRunEdit()
    page = pg
    selectedId = null
    previewFresh = false
    previewTmpPath = ''
    previewLoading = true
    try {
      await loadPage(path, pg)
    } catch (e) {
      error = String(e)
    } finally {
      previewLoading = false
    }
  }

  function setZoom(next: number) {
    zoom = Math.min(4, Math.max(0.5, Math.round(next * 20) / 20))
  }

  // Keep the bitmap at the canvas's true device resolution; anything else is
  // resampled by the browser and looks soft.
  $effect(() => {
    const target = renderWidthForCss(surfaceW)
    if (!path || !surfaceW) return
    if (!needsRerender(renderWidth, target)) return
    if (rerenderTimer) clearTimeout(rerenderTimer)
    rerenderTimer = setTimeout(() => {
      renderWidth = target
      previewLoading = true
      void loadPage(path, page).finally(() => {
        previewLoading = false
      })
    }, 180)
  })

  function onWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return
    e.preventDefault()
    setZoom(zoom + (e.deltaY > 0 ? -0.1 : 0.1))
  }

  /* ---------- geometry ---------- */

  function clamp01(v: number) {
    return Math.min(1, Math.max(0, v))
  }

  function normFromEvent(e: PointerEvent) {
    if (!surfaceEl) return null
    const r = surfaceEl.getBoundingClientRect()
    if (r.width <= 0 || r.height <= 0) return null
    return { x: clamp01((e.clientX - r.left) / r.width), y: clamp01((e.clientY - r.top) / r.height) }
  }

  /** Points → screen pixels, so on-canvas text matches the printed result. */
  function ptToPx(pt: number) {
    if (!mediabox || !surfaceW) return pt
    return (pt / mediabox.width) * surfaceW
  }

  /* ---------- typography ----------
   * The overlay has to sit exactly on top of the rasterised glyphs, so we
   * reproduce the PDF font as closely as the browser allows and align by
   * baseline rather than by box.
   */

  let measureCtx: CanvasRenderingContext2D | null = null
  const metricsCache = new Map<string, { asc: number; desc: number }>()

  function measurer() {
    if (!measureCtx) measureCtx = document.createElement('canvas').getContext('2d')
    return measureCtx
  }

  function cssFamily(pdfFont: string) {
    const n = (pdfFont || '').toLowerCase()
    if (/times|roman|serif|georgia|garamond|book/.test(n)) return '"Times New Roman", Times, serif'
    if (/courier|mono|consol/.test(n)) return '"Courier New", Courier, monospace'
    return 'Helvetica, Arial, sans-serif'
  }

  function cssWeight(pdfFont: string) {
    return /bold|black|heavy|semibold|demi/i.test(pdfFont) ? 700 : 400
  }

  function cssStyle(pdfFont: string) {
    return /italic|oblique/i.test(pdfFont) ? 'italic' : 'normal'
  }

  /** Ascent/descent as a fraction of the font size, matching CSS line-box baseline. */
  function fontMetrics(family: string, weight: number, style: string) {
    const key = `${style}|${weight}|${family}`
    const hit = metricsCache.get(key)
    if (hit) return hit
    const c = measurer()
    // CSS places the line-box baseline at: half-leading + (font ascent).
    // The font ascent is the typographic ascent (hhea/sTypo), NOT the font
    // bounding box ascent. fontBoundingBoxAscent (~0.95 for Helvetica) would
    // push the editor ~3px below the original at 12pt; we need ~0.72.
    let m = { asc: 0.75, desc: 0.25 }
    if (c) {
      c.font = `${style} ${weight} 100px ${family}`
      // actualBoundingBoxAscent of a tall glyph ("Á") ≈ font ascent.
      const t = c.measureText('ÁÉÍ')
      if (t.actualBoundingBoxAscent) {
        const asc = t.actualBoundingBoxAscent / 100
        const desc = (t.actualBoundingBoxDescent || 22) / 100
        m = { asc: Math.min(0.95, Math.max(0.6, asc)), desc: Math.min(0.4, Math.max(0.15, desc)) }
      }
    }
    metricsCache.set(key, m)
    return m
  }

  function textWidthPx(text: string, cssFont: string) {
    const c = measurer()
    if (!c || !text) return 0
    c.font = cssFont
    return c.measureText(text).width
  }

  /**
   * Screen geometry for one run. `displayText` (when given) widens the cover so
   * longer replacements don't spill over the neighbouring words.
   */
  function runLayout(tr: TextAnchor, displayText?: string) {
    const box = mediabox
    if (!box || !surfaceW || !surfaceH) {
      return {
        left: 0,
        top: 0,
        fontPx: 12,
        family: 'Helvetica, Arial, sans-serif',
        weight: 400,
        style: 'normal',
        scaleX: 1,
        coverTop: 0,
        coverLeft: 0,
        coverW: 0,
        coverH: 0,
      }
    }
    const fontPx = Math.max((tr.h / box.height) * surfaceH, 1)
    const family = cssFamily(tr.fontName)
    const weight = cssWeight(tr.fontName)
    const style = cssStyle(tr.fontName)
    const cssFont = `${style} ${weight} ${fontPx}px ${family}`
    const { asc, desc } = fontMetrics(family, weight, style)

    const left = ((tr.x - box.x) / box.width) * surfaceW
    const baseline = ((box.y + box.height - tr.y) / box.height) * surfaceH
    const origW = (tr.w / box.width) * surfaceW

    // The substitute font never matches the embedded one exactly; squeeze it so
    // the original string covers the same span it does in the bitmap.
    const measured = textWidthPx(tr.text, cssFont)
    const scaleX = measured > 0 ? Math.min(1.35, Math.max(0.7, origW / measured)) : 1
    const shownW =
      displayText != null ? textWidthPx(displayText, cssFont) * scaleX : origW

    // With line-height:1 the baseline sits half-leading + ascent below the top.
    const baselineFromTop = ((1 - (asc + desc)) / 2 + asc) * fontPx

    return {
      left,
      top: baseline - baselineFromTop,
      fontPx,
      family,
      weight,
      style,
      scaleX,
      coverLeft: left - fontPx * 0.06,
      coverTop: baseline - asc * fontPx,
      coverW: Math.max(origW, shownW) + fontPx * 0.16,
      coverH: (asc + desc) * fontPx,
    }
  }

  /* ---------- lines + paragraphs ----------
   * Robust PDF layout analysis tuned for Word-like editing:
   * 1) Cluster runs that share a baseline into lines (generous Y tolerance
   *    so subpixel jitter doesn't split "purchased" into "purchase"+"d").
   * 2) Always insert a space between runs when there's a positive gap.
   * 3) Group lines into paragraphs by vertical proximity + same font size,
   *    no column check — just like Word.
   */

  function median(vals: number[]): number {
    if (!vals.length) return 0
    const s = [...vals].sort((a, b) => a - b)
    const mid = Math.floor(s.length / 2)
    return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2
  }

  function buildLines(runs: TextRun[]): TextLine[] {
    if (!runs.length) return []
    const avgH = Math.max(
      median(runs.map((r) => r.h).filter((h) => h > 0)) || 12,
      1,
    )
    // Generous: same line if baselines are within half a line height.
    const yTol = Math.max(avgH * 0.5, 3)

    const sorted = [...runs].sort((a, b) => b.y - a.y || a.x - b.x)
    const clusters: TextRun[][] = []
    for (const r of sorted) {
      const cluster = clusters.find((c) => {
        const seed = c[0]
        return Math.abs(seed.y - r.y) <= yTol
      })
      if (cluster) cluster.push(r)
      else clusters.push([r])
    }

    const lines: TextLine[] = []
    for (const c of clusters) {
      lines.push(makeLine(c.sort((a, b) => a.x - b.x)))
    }
    return lines.sort((a, b) => b.y - a.y || a.x - b.x)
  }

  function makeLine(runs: TextRun[]): TextLine {
    const first = runs[0]
    const last = runs[runs.length - 1]
    const h = Math.max(...runs.map((r) => r.h), 1)
    const y = median(runs.map((r) => r.y))

    let text = ''
    runs.forEach((r, i) => {
      const prev = runs[i - 1]
      if (prev) {
        const gap = r.x - (prev.x + prev.w)
        // Any positive gap = a space in the original text.
        if (gap > 0.1 && !text.endsWith(' ') && !r.text.startsWith(' ')) text += ' '
      }
      text += r.text
    })

    return {
      id: first.runId,
      runIds: runs.map((r) => r.runId),
      text,
      x: first.x,
      y,
      w: Math.max(last.x + last.w - first.x, first.w),
      h,
      fontName: first.fontName,
      color: first.color,
      editable: runs.every((r) => r.editable),
    }
  }

  interface TextParagraph {
    id: number
    lines: TextLine[]
    runIds: number[]
    text: string
    x: number
    y: number
    w: number
    h: number
    fontName: string
    fontSize: number
    color: string
    editable: boolean
    /** Points between consecutive baselines. */
    leading: number
  }

  function sizeKey(h: number) {
    // Snap to 1pt so minor matrix noise doesn't split a paragraph.
    return Math.round(h)
  }

  function buildParagraphs(lines: TextLine[]): TextParagraph[] {
    if (!lines.length) return []
    const sorted = [...lines].sort((a, b) => b.y - a.y || a.x - b.x)

    const out: TextParagraph[] = []
    let group: TextLine[] = []

    const flush = () => {
      if (group.length) out.push(makeParagraph(group))
      group = []
    }

    for (const line of sorted) {
      const prev = group[group.length - 1]
      if (!prev) {
        group = [line]
        continue
      }
      // Different font size = different block (heading vs body). Exact 0.5pt.
      const sameSize = Math.abs(prev.h - line.h) <= 0.5
      const vGap = prev.y - line.y
      // Same paragraph: same size + lines stacked within normal leading.
      const samePara = sameSize && vGap > 0 && vGap <= prev.h * 2.0
      if (samePara) group.push(line)
      else {
        flush()
        group = [line]
      }
    }
    flush()
    return out
  }

  function makeParagraph(lines: TextLine[]): TextParagraph {
    const first = lines[0]
    const last = lines[lines.length - 1]
    const fontSize = Math.max(...lines.map((l) => l.h), 1)
    const leading =
      lines.length > 1 ? (first.y - last.y) / (lines.length - 1) : fontSize * 1.2
    const left = Math.min(...lines.map((l) => l.x))
    const right = Math.max(...lines.map((l) => l.x + l.w))
    return {
      id: first.id,
      lines,
      runIds: lines.flatMap((l) => l.runIds),
      text: lines.map((l) => l.text).join('\n'),
      x: left,
      y: first.y,
      w: right - left,
      h: first.y - last.y + last.h,
      fontName: first.fontName,
      fontSize,
      color: first.color,
      editable: lines.every((l) => l.editable),
      leading,
    }
  }

  /** Screen geometry for a paragraph block — baseline-aligned like the PDF. */
  function paragraphLayout(para: TextParagraph, displayText?: string) {
    const box = mediabox
    if (!box || !surfaceW || !surfaceH) {
      return {
        left: 0,
        top: 0,
        fontPx: 12,
        leadingPx: 14,
        family: 'Helvetica, Arial, sans-serif',
        weight: 400,
        style: 'normal',
        coverTop: 0,
        coverLeft: 0,
        coverW: 0,
        coverH: 0,
      }
    }
    const fontPx = Math.max(ptToPx(para.fontSize), 1)
    const leadingPx = Math.max(ptToPx(para.leading), fontPx)
    const family = cssFamily(para.fontName)
    const weight = cssWeight(para.fontName)
    const style = cssStyle(para.fontName)
    const { asc, desc } = fontMetrics(family, weight, style)

    const left = ((para.x - box.x) / box.width) * surfaceW
    // Baseline of the first line, in screen pixels.
    const baseline = ((box.y + box.height - para.y) / box.height) * surfaceH
    const wPx = (para.w / box.width) * surfaceW

    // Contenteditable / pre-wrap with line-height:leadingPx: first baseline is
    // roughly (leading - font)/2 + ascent — same model as a normal CSS block.
    const firstBaselineFromTop = (leadingPx - fontPx) / 2 + asc * fontPx

    const lineCount = displayText != null ? Math.max(displayText.split('\n').length, 1) : para.lines.length
    const coverH = Math.max(
      (para.lines[0].y - para.lines[para.lines.length - 1].y) / box.height * surfaceH +
        (asc + desc) * fontPx,
      lineCount * leadingPx,
    )

    return {
      left,
      top: baseline - firstBaselineFromTop,
      fontPx,
      leadingPx,
      family,
      weight,
      style,
      coverLeft: left - fontPx * 0.08,
      coverTop: baseline - asc * fontPx,
      coverW: wPx + fontPx * 0.2,
      coverH: coverH + fontPx * 0.1,
    }
  }

  /**
   * A run's `y` is its baseline; glyphs span roughly 0.80 above and 0.24 below,
   * so the clickable box has to be built around it rather than from it.
   */
  function runToNorm(run: TextAnchor) {
    const box = mediabox
    if (!box) return { nx: 0, ny: 0, nw: 0.1, nh: 0.02 }
    const top = run.y + run.h * 0.8
    const bottom = run.y - run.h * 0.24
    return {
      nx: (run.x - box.x) / box.width,
      ny: (box.y + box.height - top) / box.height,
      nw: Math.max(run.w / box.width, 0.004),
      nh: Math.max((top - bottom) / box.height, 0.004),
    }
  }

  /** Full paragraph glyph box (first ascent → last descent), for whiteout. */
  function paraToNorm(para: TextParagraph) {
    const box = mediabox
    if (!box) return { nx: 0, ny: 0, nw: 0.1, nh: 0.02 }
    const first = para.lines[0]
    const last = para.lines[para.lines.length - 1]
    const top = first.y + first.h * 0.85
    const bottom = last.y - last.h * 0.28
    return {
      nx: (para.x - box.x) / box.width,
      ny: (box.y + box.height - top) / box.height,
      nw: Math.max(para.w / box.width, 0.004),
      nh: Math.max((top - bottom) / box.height, 0.004),
    }
  }

  /** Screen box for an AcroForm widget, so it can be filled where it lives. */
  function fieldLayout(f: FormField) {
    const box = mediabox
    if (!box || !surfaceW || !surfaceH) return { left: 0, top: 0, w: 0, h: 0, fontPx: 11 }
    const h = (f.h / box.height) * surfaceH
    return {
      left: ((f.x - box.x) / box.width) * surfaceW,
      top: ((box.y + box.height - (f.y + f.h)) / box.height) * surfaceH,
      w: (f.w / box.width) * surfaceW,
      h,
      fontPx: Math.max(Math.min(h * 0.66, 22), 8),
    }
  }

  function isChecked(v: string | undefined) {
    return v === 'Yes' || v === 'On' || v === 'true' || v === '1'
  }

  function setField(name: string, value: string) {
    formValues = { ...formValues, [name]: value }
  }

  function focusField(f: FormField) {
    if (f.page !== page) void setPage(f.page)
    queueMicrotask(() => {
      const el = surfaceEl?.querySelector<HTMLElement>(`[data-field="${CSS.escape(f.name)}"]`)
      el?.focus()
      el?.scrollIntoView({ block: 'center', behavior: 'smooth' })
    })
  }

  function clampBox(nx: number, ny: number, nw: number, nh: number) {
    let w = Math.max(MIN_SIZE, nw)
    let h = Math.max(MIN_SIZE, nh)
    let x = clamp01(nx)
    let y = clamp01(ny)
    if (x + w > 1) x = Math.max(0, 1 - w)
    if (y + h > 1) y = Math.max(0, 1 - h)
    return { nx: x, ny: y, nw: Math.min(w, 1 - x), nh: Math.min(h, 1 - y) }
  }

  function applyResize(
    handle: Handle,
    snx: number,
    sny: number,
    snw: number,
    snh: number,
    dx: number,
    dy: number,
  ) {
    let nx = snx
    let ny = sny
    let nw = snw
    let nh = snh
    if (handle.includes('e')) nw = snw + dx
    if (handle.includes('w')) {
      nx = snx + dx
      nw = snw - dx
    }
    if (handle.includes('s')) nh = snh + dy
    if (handle.includes('n')) {
      ny = sny + dy
      nh = snh - dy
    }
    if (nw < MIN_SIZE) {
      if (handle.includes('w')) nx = snx + snw - MIN_SIZE
      nw = MIN_SIZE
    }
    if (nh < MIN_SIZE) {
      if (handle.includes('n')) ny = sny + snh - MIN_SIZE
      nh = MIN_SIZE
    }
    return clampBox(nx, ny, nw, nh)
  }

  /* ---------- history ---------- */

  function pushUndo() {
    undoStack = [...undoStack.slice(-49), $state.snapshot(objects) as EditObject[]]
    redoStack = []
  }

  function undo() {
    if (!undoStack.length) return
    redoStack = [...redoStack, $state.snapshot(objects) as EditObject[]]
    objects = undoStack[undoStack.length - 1]
    undoStack = undoStack.slice(0, -1)
    selectedId = null
    editingObjId = null
    schedulePreviewRefresh()
  }

  function redo() {
    if (!redoStack.length) return
    undoStack = [...undoStack, $state.snapshot(objects) as EditObject[]]
    objects = redoStack[redoStack.length - 1]
    redoStack = redoStack.slice(0, -1)
    selectedId = null
    editingObjId = null
    schedulePreviewRefresh()
  }

  function uid() {
    return `e${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`
  }

  function addObject(obj: EditObject) {
    pushUndo()
    objects = [...objects, obj]
    selectedId = obj.id
    return obj
  }

  function updateObject(id: string, patch: Partial<EditObject>) {
    objects = objects.map((o) => (o.id === id ? { ...o, ...patch } : o))
  }

  function updateSelected(patch: Partial<EditObject>) {
    if (selectedId) updateObject(selectedId, patch)
  }

  function deleteSelected() {
    if (!selectedId) return
    pushUndo()
    objects = objects.filter((o) => o.id !== selectedId)
    selectedId = null
    editingObjId = null
  }

  function removeObject(id: string) {
    pushUndo()
    objects = objects.filter((o) => o.id !== id)
    if (selectedId === id) selectedId = null
  }

  function clearPage() {
    if (!pageObjects.length) return
    pushUndo()
    objects = objects.filter((o) => o.page !== page)
    selectedId = null
  }

  /* ---------- object creation ---------- */

  function makeText(box: { nx: number; ny: number; nw: number; nh: number }, startEditing: boolean) {
    const obj = addObject({
      id: uid(),
      kind: 'addText',
      page,
      label: 'Texto',
      ...box,
      text: '',
      font,
      size: fontSize,
      bold,
      italic,
      color: textColor,
      align,
      opacity,
    })
    if (startEditing) {
      editingObjId = obj.id
      editDraft = ''
    }
  }

  function makeAnnot(box: { nx: number; ny: number; nw: number; nh: number }, label: string) {
    const kind: Kind =
      annotSub === 'underline' ? 'underline' : annotSub === 'strikeout' ? 'strikeout' : 'highlight'
    addObject({
      id: uid(),
      kind,
      page,
      label,
      ...box,
      color: annotSub === 'highlight' ? highlightColor : strokeColor,
      opacity: annotSub === 'highlight' ? 0.45 : 1,
    })
  }

  async function makeImage(box: { nx: number; ny: number; nw: number; nh: number }) {
    const picked = await open({
      multiple: false,
      filters: [{ name: 'Imagen', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    })
    if (typeof picked !== 'string') return
    const url = await imageUrl(picked)
    const ratio = url ? await naturalRatio(url) : 1
    const nh = ratio > 0 && mediabox ? (box.nw * ratio * mediabox.width) / mediabox.height : box.nh
    addObject({
      id: uid(),
      kind: 'image',
      page,
      label: fileName(picked),
      ...clampBox(box.nx, box.ny, box.nw, Math.min(nh || box.nh, 0.9)),
      imagePath: picked,
      opacity,
      rotation: 0,
    })
  }

  async function imageUrl(p: string) {
    const cached = imageUrls[p]
    if (cached) return cached
    try {
      const bytes = await readFile(p)
      const url = URL.createObjectURL(new Blob([bytes as BlobPart]))
      imageUrls = { ...imageUrls, [p]: url }
      return url
    } catch {
      return ''
    }
  }

  function naturalRatio(url: string): Promise<number> {
    return new Promise((resolve) => {
      const img = new Image()
      img.onload = () => resolve(img.naturalWidth ? img.naturalHeight / img.naturalWidth : 1)
      img.onerror = () => resolve(1)
      img.src = url
    })
  }

  /* ---------- pointer interaction ---------- */

  function onSurfacePointerDown(e: PointerEvent) {
    if (!path || e.button !== 0) return
    const target = e.target as HTMLElement
    // Anything that owns its own interaction handles the event itself.
    if (target.closest('.ed-obj, .ed-handle, .ed-run, .ed-inline, .ed-linehit, .ed-field, .ed-textarea, .ed-rich')) return

    commitObjEdit()
    commitRunEdit()
    selectedId = null

    const n = normFromEvent(e)
    if (!n) return

    if (mode === 'edit' || mode === 'form') return

    surfaceEl?.setPointerCapture(e.pointerId)

    if (mode === 'draw') {
      drag = { kind: 'draw', points: [[n.x, n.y]] }
      draftPath = [[n.x, n.y]]
      return
    }

    drag = { kind: 'create', ox: n.x, oy: n.y }
    draftBox = { nx: n.x, ny: n.y, nw: 0, nh: 0 }
  }

  function onSurfacePointerMove(e: PointerEvent) {
    const d = drag
    if (!d) return
    const n = normFromEvent(e)
    if (!n) return

    if (d.kind === 'draw') {
      d.points.push([n.x, n.y])
      draftPath = [...d.points]
      return
    }

    if (d.kind === 'create') {
      draftBox = {
        nx: Math.min(d.ox, n.x),
        ny: Math.min(d.oy, n.y),
        nw: Math.abs(n.x - d.ox),
        nh: Math.abs(n.y - d.oy),
      }
      return
    }

    if (d.kind === 'move') {
      const o = objects.find((x) => x.id === d.id)
      if (!o) return
      updateObject(
        d.id,
        clampBox(d.snx + (n.x - d.ox), d.sny + (n.y - d.oy), o.nw ?? MIN_SIZE, o.nh ?? MIN_SIZE),
      )
      return
    }

    if (d.kind === 'moveLine') {
      const dx = n.x - d.ox
      const dy = n.y - d.oy
      updateObject(d.id, {
        from: [clamp01(d.sfrom[0] + dx), clamp01(d.sfrom[1] + dy)],
        to: [clamp01(d.sto[0] + dx), clamp01(d.sto[1] + dy)],
      })
      return
    }

    if (d.kind === 'resize') {
      updateObject(d.id, applyResize(d.handle, d.snx, d.sny, d.snw, d.snh, n.x - d.ox, n.y - d.oy))
      return
    }

    if (d.kind === 'endpoint') {
      updateObject(d.id, { [d.which]: [n.x, n.y] } as Partial<EditObject>)
    }
  }

  async function onSurfacePointerUp(e: PointerEvent) {
    const d = drag
    if (!d) return
    drag = null
    try {
      surfaceEl?.releasePointerCapture(e.pointerId)
    } catch {
      /* pointer already released */
    }

    if (d.kind === 'draw') {
      const pts = draftPath
      draftPath = null
      if (!pts || pts.length < 2) return
      addObject({
        id: uid(),
        kind: 'freeDraw',
        page,
        label: 'Trazo',
        paths: [pts],
        color: strokeColor,
        strokeWidth,
        opacity,
      })
      return
    }

    if (d.kind !== 'create') return

    const raw = draftBox
    draftBox = null
    if (!raw) return

    const isClick = raw.nw < CLICK_SLOP && raw.nh < CLICK_SLOP
    const box = isClick ? defaultBoxAt(d.ox, d.oy) : clampBox(raw.nx, raw.ny, raw.nw, raw.nh)

    if (mode === 'text') {
      makeText(box, true)
      return
    }
    if (mode === 'stamp') {
      addObject({
        id: uid(),
        kind: 'stamp',
        page,
        label: stampCustom || stampKind.toUpperCase(),
        ...box,
        stamp: stampKind,
        customText: stampCustom || undefined,
        color: strokeColor,
      })
      return
    }
    if (mode === 'whiteout') {
      addObject({ id: uid(), kind: 'whiteout', page, label: 'Borrado', ...box, color: '#ffffff' })
      return
    }
    if (mode === 'annotate') {
      if (annotSub === 'note') {
        addObject({
          id: uid(),
          kind: 'note',
          page,
          label: 'Nota',
          nx: box.nx,
          ny: box.ny,
          nw: 0.028,
          nh: 0.028,
          text: 'Nota',
          color: '#f5c542',
        })
      } else {
        makeAnnot(box, annotSub)
      }
      return
    }
    if (mode === 'shapes') {
      if (shapeSub === 'line') {
        addObject({
          id: uid(),
          kind: 'line',
          page,
          label: 'Línea',
          from: [box.nx, box.ny],
          to: [box.nx + box.nw, box.ny + box.nh],
          color: strokeColor,
          strokeWidth,
          arrow: 'end',
        })
      } else {
        addObject({
          id: uid(),
          kind: shapeSub,
          page,
          label: shapeSub === 'rect' ? 'Rectángulo' : 'Elipse',
          ...box,
          stroke: strokeColor,
          fill: fillColor,
          strokeWidth,
          opacity,
        })
      }
      return
    }
    if (mode === 'image') await makeImage(box)
  }

  function defaultBoxAt(x: number, y: number) {
    if (mode === 'text') return clampBox(x, y - 0.012, 0.3, 0.045)
    if (mode === 'annotate' && annotSub !== 'note') return clampBox(x - 0.1, y - 0.012, 0.2, 0.024)
    if (mode === 'annotate') return clampBox(x, y, 0.028, 0.028)
    if (mode === 'stamp') return clampBox(x - 0.11, y - 0.035, 0.22, 0.07)
    if (mode === 'image') return clampBox(x - 0.12, y - 0.09, 0.24, 0.18)
    return clampBox(x - 0.1, y - 0.035, 0.2, 0.07)
  }

  function startMove(e: PointerEvent, obj: EditObject) {
    if (e.button !== 0) return
    e.stopPropagation()
    commitObjEdit()
    selectedId = obj.id
    const n = normFromEvent(e)
    if (!n) return
    pushUndo()
    if (obj.kind === 'line' && obj.from && obj.to) {
      drag = {
        kind: 'moveLine',
        id: obj.id,
        ox: n.x,
        oy: n.y,
        sfrom: [obj.from[0], obj.from[1]],
        sto: [obj.to[0], obj.to[1]],
      }
    } else {
      drag = { kind: 'move', id: obj.id, ox: n.x, oy: n.y, snx: obj.nx ?? 0, sny: obj.ny ?? 0 }
    }
    surfaceEl?.setPointerCapture(e.pointerId)
  }

  function startResize(e: PointerEvent, obj: EditObject, handle: Handle) {
    if (e.button !== 0) return
    e.stopPropagation()
    selectedId = obj.id
    const n = normFromEvent(e)
    if (!n) return
    pushUndo()
    drag = {
      kind: 'resize',
      id: obj.id,
      handle,
      ox: n.x,
      oy: n.y,
      snx: obj.nx ?? 0,
      sny: obj.ny ?? 0,
      snw: obj.nw ?? MIN_SIZE,
      snh: obj.nh ?? MIN_SIZE,
    }
    surfaceEl?.setPointerCapture(e.pointerId)
  }

  function startEndpoint(e: PointerEvent, obj: EditObject, which: 'from' | 'to') {
    if (e.button !== 0) return
    e.stopPropagation()
    selectedId = obj.id
    pushUndo()
    drag = { kind: 'endpoint', id: obj.id, which }
    surfaceEl?.setPointerCapture(e.pointerId)
  }

  /* ---------- text editing ---------- */

  function mapPdfFont(name: string) {
    const n = (name || '').toLowerCase()
    if (/times|roman|serif|georgia|garamond|book/.test(n)) return 'Times'
    if (/courier|mono|consol/.test(n)) return 'Courier'
    return 'Helvetica'
  }

  function defaultSpan(text: string, para: TextParagraph): RichSpan {
    return {
      text,
      font: mapPdfFont(para.fontName),
      size: Math.round(para.fontSize * 10) / 10,
      bold: /bold|black|heavy|semibold|demi/i.test(para.fontName),
      italic: /italic|oblique/i.test(para.fontName),
      color: (para.color || '#1a1a1a').toLowerCase(),
    }
  }

  function plainToRich(text: string, para: TextParagraph): RichSpan[][] {
    return text.split('\n').map((line) => [defaultSpan(line, para)])
  }

  function richToPlain(lines: RichSpan[][]) {
    return lines.map((line) => line.map((s) => s.text).join('')).join('\n')
  }

  function spanStyleClose(a: RichSpan, b: RichSpan) {
    return (
      a.font === b.font &&
      Math.abs(a.size - b.size) <= 0.6 &&
      a.bold === b.bold &&
      a.italic === b.italic &&
      (a.color || '').toLowerCase() === (b.color || '').toLowerCase()
    )
  }

  function richLooksOriginal(rich: RichSpan[][], para: TextParagraph) {
    const base = defaultSpan('', para)
    const plain = richToPlain(rich)
    if (plain !== para.text) {
      // Text may differ; still check that every span is the base style.
    }
    return rich.every((line) => line.every((s) => spanStyleClose(s, { ...base, text: s.text })))
  }

  function richToHtml(lines: RichSpan[][], para?: TextParagraph) {
    const base = para ? defaultSpan('', para) : null
    return lines
      .map((line) => {
        if (!line.length) return '<div><br></div>'
        return (
          '<div>' +
          line
            .map((s) => {
              const esc = s.text
                .replace(/&/g, '&amp;')
                .replace(/</g, '&lt;')
                .replace(/>/g, '&gt;')
              // Same as paragraph default → no inline style (inherits exact overlay metrics).
              if (base && spanStyleClose(s, base)) {
                return esc || '<br>'
              }
              const style = [
                `font-family:${cssFamily(s.font)}`,
                `font-size:${Math.max(ptToPx(s.size), 8)}px`,
                `font-weight:${s.bold ? 700 : 400}`,
                `font-style:${s.italic ? 'italic' : 'normal'}`,
                `color:${s.color}`,
              ].join(';')
              return `<span style="${style}">${esc || '<br>'}</span>`
            })
            .join('') +
          '</div>'
        )
      })
      .join('')
  }

  function styleFromEl(el: Element, base: RichSpan): RichSpan {
    const cs = getComputedStyle(el)
    const ff = (cs.fontFamily || '').toLowerCase()
    let font = base.font
    if (ff.includes('times') || ff.includes('georgia') || ff.includes('serif')) font = 'Times'
    else if (ff.includes('courier') || ff.includes('mono')) font = 'Courier'
    else if (ff.includes('helvetica') || ff.includes('arial') || ff.includes('sans')) font = 'Helvetica'
    const weight = parseInt(cs.fontWeight, 10)
    const px = parseFloat(cs.fontSize) || ptToPx(base.size)
    let size =
      mediabox && surfaceW
        ? Math.round(((px / surfaceW) * mediabox.width) * 10) / 10
        : base.size
    // Snap noise from getComputedStyle back to the paragraph size.
    if (Math.abs(size - base.size) <= 0.6) size = base.size
    const color = (rgbToHex(cs.color) || base.color).toLowerCase()
    return {
      text: '',
      font,
      size: size || base.size,
      bold: weight >= 600 || cs.fontWeight === 'bold',
      italic: cs.fontStyle === 'italic' || cs.fontStyle === 'oblique',
      color,
    }
  }

  function rgbToHex(c: string) {
    const m = c.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/)
    if (!m) return c.startsWith('#') ? c : ''
    const h = (n: string) => Number(n).toString(16).padStart(2, '0')
    return `#${h(m[1])}${h(m[2])}${h(m[3])}`
  }

  function parseRichFromDom(root: HTMLElement, base: RichSpan): RichSpan[][] {
    const lines: RichSpan[][] = []
    let cur: RichSpan[] = []

    const pushLine = () => {
      if (!cur.length) cur = [{ ...base, text: '' }]
      lines.push(cur)
      cur = []
    }

    const addText = (text: string, style: RichSpan) => {
      if (!text) return
      const parts = text.split('\n')
      parts.forEach((p, i) => {
        if (i > 0) pushLine()
        if (!p && i < parts.length - 1) return
        const last = cur[cur.length - 1]
        if (
          last &&
          last.font === style.font &&
          last.size === style.size &&
          last.bold === style.bold &&
          last.italic === style.italic &&
          last.color === style.color
        ) {
          last.text += p
        } else {
          cur.push({ ...style, text: p })
        }
      })
    }

    const walk = (node: Node, style: RichSpan) => {
      if (node.nodeType === Node.TEXT_NODE) {
        addText(node.textContent ?? '', style)
        return
      }
      if (node.nodeType !== Node.ELEMENT_NODE) return
      const el = node as HTMLElement
      const tag = el.tagName
      if (tag === 'BR') {
        pushLine()
        return
      }
      const next = styleFromEl(el, style)
      if (tag === 'DIV' || tag === 'P' || tag === 'LI') {
        if (cur.length || lines.length) pushLine()
        for (const child of Array.from(el.childNodes)) walk(child, next)
        return
      }
      for (const child of Array.from(el.childNodes)) walk(child, next)
    }

    for (const child of Array.from(root.childNodes)) walk(child, base)
    if (cur.length || !lines.length) pushLine()
    // Drop a trailing empty line produced by a final <div>
    while (lines.length > 1 && lines[lines.length - 1].every((s) => !s.text)) lines.pop()
    return lines
  }

  function textWidthPt(sp: RichSpan) {
    const css = `${sp.italic ? 'italic' : 'normal'} ${sp.bold ? 700 : 400} ${Math.max(ptToPx(sp.size), 8)}px ${cssFamily(sp.font)}`
    const px = textWidthPx(sp.text, css)
    if (!mediabox || !surfaceW) return sp.text.length * sp.size * 0.5
    return (px / surfaceW) * mediabox.width
  }

  function buildBakeSpans(
    para: TextParagraph,
    richLines: RichSpan[][],
    textAlign: string,
  ): BakeSpan[] {
    const out: BakeSpan[] = []
    for (let i = 0; i < richLines.length; i++) {
      const spans = richLines[i].filter((s) => s.text.length > 0)
      if (!spans.length) continue
      const orig = para.lines[Math.min(i, para.lines.length - 1)]
      // Exact PDF baseline of that visual line.
      const baselineY = orig?.y ?? para.y - i * para.leading
      const lineW = spans.reduce((acc, s) => acc + textWidthPt(s), 0)
      let x = orig?.x ?? para.x
      if (textAlign === 'center') x = para.x + (para.w - lineW) * 0.5
      else if (textAlign === 'right') x = para.x + para.w - lineW
      for (const sp of spans) {
        const measured = Math.max(textWidthPt(sp), 1)
        out.push({
          x,
          // h=0 → bake_text treats `y` as the baseline (no box offset).
          y: baselineY,
          // Huge width so wrap_text never splits a fragment (we place x ourselves).
          w: 10_000,
          h: 0,
          text: sp.text,
          font: sp.font,
          size: sp.size,
          bold: sp.bold,
          italic: sp.italic,
          color: sp.color,
        })
        x += measured
      }
    }
    return out
  }

  function captureEditSelection() {
    const root = editEl
    const sel = window.getSelection()
    if (!root || !sel || sel.rangeCount === 0) return
    const range = sel.getRangeAt(0)
    if (!root.contains(range.commonAncestorContainer)) return
    savedEditRange = range.cloneRange()
  }

  function activeEditRange(): Range | null {
    const root = editEl
    if (!root) return null
    const sel = window.getSelection()
    if (sel && sel.rangeCount > 0) {
      const range = sel.getRangeAt(0)
      if (root.contains(range.commonAncestorContainer) && !range.collapsed) return range
    }
    if (
      savedEditRange &&
      root.contains(savedEditRange.commonAncestorContainer) &&
      !savedEditRange.collapsed
    ) {
      return savedEditRange
    }
    return null
  }

  function applySelectionFormat(patch: Partial<RichSpan>) {
    const root = editEl
    if (!root || editingRunId == null) {
      if (patch.font != null) font = patch.font
      if (patch.size != null) fontSize = patch.size
      if (patch.bold != null) bold = patch.bold
      if (patch.italic != null) italic = patch.italic
      if (patch.color != null) textColor = patch.color
      return
    }

    const range = activeEditRange()
    if (!range) {
      // No selection → defaults for newly typed text.
      if (patch.font != null) font = patch.font
      if (patch.size != null) fontSize = patch.size
      if (patch.bold != null) bold = patch.bold
      if (patch.italic != null) italic = patch.italic
      if (patch.color != null) textColor = patch.color
      root.style.fontFamily = cssFamily(font)
      root.style.fontSize = `${Math.max(ptToPx(fontSize), 8)}px`
      root.style.fontWeight = bold ? '700' : '400'
      root.style.fontStyle = italic ? 'italic' : 'normal'
      root.style.color = textColor
      root.focus({ preventScroll: true })
      return
    }

    const f = patch.font ?? font
    const s = patch.size ?? fontSize
    const b = patch.bold != null ? patch.bold : bold
    const it = patch.italic != null ? patch.italic : italic
    const c = patch.color ?? textColor

    const span = document.createElement('span')
    span.style.fontFamily = cssFamily(f)
    span.style.fontSize = `${Math.max(ptToPx(s), 8)}px`
    span.style.fontWeight = b ? '700' : '400'
    span.style.fontStyle = it ? 'italic' : 'normal'
    span.style.color = c

    try {
      range.surroundContents(span)
    } catch {
      const frag = range.extractContents()
      span.appendChild(frag)
      range.insertNode(span)
    }

    if (patch.font != null) font = patch.font
    if (patch.size != null) fontSize = patch.size
    if (patch.bold != null) bold = patch.bold
    if (patch.italic != null) italic = patch.italic
    if (patch.color != null) textColor = patch.color

    const sel = window.getSelection()
    if (sel) {
      sel.removeAllRanges()
      const after = document.createRange()
      after.selectNodeContents(span)
      sel.addRange(after)
      savedEditRange = after.cloneRange()
    }
    root.focus({ preventScroll: true })
    syncDraftFromEditor()
  }

  function syncDraftFromEditor() {
    if (!editEl) return
    editDraft = editEl.innerText.replace(/\n$/, '')
  }

  function loadParaStyle(para: TextParagraph, existing?: EditObject | null) {
    font = existing?.font ?? mapPdfFont(para.fontName)
    fontSize = Math.round((existing?.size ?? para.fontSize) * 10) / 10
    bold = existing?.bold ?? /bold|black|heavy|semibold|demi/i.test(para.fontName)
    italic = existing?.italic ?? /italic|oblique/i.test(para.fontName)
    textColor = (existing?.color ?? para.color ?? '#1a1a1a').toLowerCase()
    align = existing?.align ?? 'left'
  }

  function mountRichEditor(node: HTMLElement, html: string) {
    node.innerHTML = html
    editEl = node
    requestAnimationFrame(() => {
      node.focus({ preventScroll: true })
      const sel = window.getSelection()
      if (!sel) return
      const r = document.createRange()
      r.selectNodeContents(node)
      // Collapse to end so the user sees caret on the text, not a full selection flash.
      r.collapse(false)
      sel.removeAllRanges()
      sel.addRange(r)
      savedEditRange = null
    })
    return {
      update(next: string) {
        if (editEl !== node) editEl = node
        if (node.innerHTML !== next && document.activeElement !== node) {
          node.innerHTML = next
        }
      },
      destroy() {
        if (editEl === node) editEl = null
        savedEditRange = null
      },
    }
  }

  function onRunPointerDown(e: PointerEvent, para: TextParagraph) {
    if (e.button !== 0) return
    e.stopPropagation()
    if (mode === 'edit') {
      if (editingRunId != null && editingRunId !== para.id) {
        commitRunEdit()
      }
      const existing = objects.find(
        (o) => o.kind === 'replaceText' && o.page === page && o.runId === para.id,
      )
      editingRunId = para.id
      const rich =
        existing?.richLines ??
        plainToRich(existing?.text ?? para.text, para)
      editDraft = richToPlain(rich)
      editRichSeed = richToHtml(rich, para)
      loadParaStyle(para, existing)
      selectedId = null
      return
    }
    if (mode === 'annotate' && annotSub !== 'note') {
      makeAnnot(runToNorm(para), `${annotSub}: ${para.text.slice(0, 24)}`)
      return
    }
    if (mode === 'whiteout') {
      addObject({
        id: uid(),
        kind: 'whiteout',
        page,
        label: `Borrado: ${para.text.slice(0, 20)}`,
        ...paraToNorm(para),
        color: '#ffffff',
      })
    }
  }

  /**
   * `expectedRunId` guards the blur handler: clicking straight onto another
   * paragraph opens that one before the old editor's blur lands, and without
   * this the stale event would close the new editor.
   */
  function commitRunEdit(expectedRunId?: number) {
    const paraId = editingRunId
    if (paraId == null) return
    if (expectedRunId != null && expectedRunId !== paraId) return
    editingRunId = null
    const para = paragraphs.find((p) => p.id === paraId)
    if (!para) return

    const base = defaultSpan('', para)
    const richLines = editEl
      ? parseRichFromDom(editEl, base)
      : plainToRich(editDraft, para)
    const next = richToPlain(richLines)
    editEl = null
    savedEditRange = null

    const others = objects.filter(
      (o) => !(o.kind === 'replaceText' && o.page === page && o.runId === paraId),
    )
    const styleChanged = !richLooksOriginal(richLines, para)

    if (next === para.text && !styleChanged) {
      objects = others
      return
    }

    pushUndo()

    // No style change → surgical per line: replace each line's text in place,
    // keeping the PDF's own font/size/position. This produces text identical
    // to the document. The backend now tries surgical for ALL fonts.
    if (!styleChanged) {
      const newLines = next.split('\n')
      objects = [
        ...others,
        {
          id: uid(),
          kind: 'replaceText',
          page,
          label: `«${para.text.slice(0, 20)}» → «${next.slice(0, 20)}»`,
          runId: paraId,
          clearRunIds: para.runIds.filter((id) => id !== paraId),
          fitWidth: para.w,
          text: next,
          lineTexts: newLines,
          ...paraToNorm(para),
          size: para.fontSize,
          color: para.color,
          align: 'left',
        },
      ]
      schedulePreviewRefresh()
      return
    }

    // Style changed → must bake with substitute font (not exact).
    objects = [
      ...others,
      {
        id: uid(),
        kind: 'replaceText',
        page,
        label: `«${para.text.slice(0, 20)}» → «${next.slice(0, 20)}»`,
        runId: paraId,
        clearRunIds: para.runIds,
        fitWidth: para.w,
        text: next,
        richLines,
        bakeSpans: buildBakeSpans(para, richLines, align),
        ...paraToNorm(para),
        font,
        size: fontSize,
        bold,
        italic,
        color: textColor,
        align,
      },
    ]
    schedulePreviewRefresh()
  }

  function startObjEdit(obj: EditObject) {
    if (obj.kind !== 'addText') return
    selectedId = obj.id
    editingObjId = obj.id
    editDraft = obj.text ?? ''
  }

  function commitObjEdit() {
    const id = editingObjId
    if (!id) return
    editingObjId = null
    const obj = objects.find((o) => o.id === id)
    if (!obj) return
    if (!editDraft.trim()) {
      objects = objects.filter((o) => o.id !== id)
      if (selectedId === id) selectedId = null
      return
    }
    updateObject(id, { text: editDraft, label: editDraft.slice(0, 28) })
  }

  function autofocus(node: HTMLElement) {
    queueMicrotask(() => {
      node.focus()
      if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) node.select()
    })
  }

  /** Focus + select for the line editor, after the pointerdown has settled. */
  function focusOnMount(node: HTMLTextAreaElement) {
    requestAnimationFrame(() => {
      node.focus({ preventScroll: true })
      node.select()
    })
  }

  /* ---------- save ---------- */

  async function run() {
    if (!path) {
      error = 'Selecciona un PDF'
      return
    }
    commitObjEdit()
    commitRunEdit()
    if (!dirty) {
      error = 'No hay cambios para guardar'
      return
    }
    loading = true
    error = null
    result = null
    try {
      const boxCache = new Map<number, PageMediaBox>()
      async function boxFor(pg: number) {
        const hit = boxCache.get(pg)
        if (hit) return hit
        const b = await getPageMediabox(path, pg)
        boxCache.set(pg, b)
        return b
      }

      const ops: EditOp[] = []

      for (const [name, value] of Object.entries(formValues)) {
        const f = formFields.find((x) => x.name === name)
        if (f && value !== f.value) ops.push({ op: 'formFill', field: name, value })
      }

      for (const o of objects) {
        const box = await boxFor(o.page)

        if (o.kind === 'replaceText' && o.runId != null && o.text != null) {
          const spans = o.bakeSpans
          if (spans?.length) {
            for (const id of o.clearRunIds ?? []) {
              ops.push({ op: 'replaceText', page: o.page, runId: id, newText: '' })
            }
            const r = normRectToPdf(o.nx ?? 0, o.ny ?? 0, o.nw ?? 0.1, o.nh ?? 0.02, box)
            ops.push({
              op: 'whiteout',
              page: o.page,
              x: r.x - 0.5,
              y: r.y - 0.5,
              w: r.w + 1,
              h: r.h + 1,
              color: '#ffffff',
            })
            for (const s of spans) {
              ops.push({
                op: 'addText',
                page: o.page,
                x: s.x,
                y: s.y,
                w: Math.max(s.w, 8),
                h: s.h,
                text: s.text,
                font: s.font,
                size: s.size,
                bold: s.bold,
                italic: s.italic,
                color: s.color,
                align: 'left',
                opacity: 1,
              })
            }
          } else if (o.lineTexts?.length) {
            // Surgical per line: replace each line's first run in place,
            // blank the rest. Keeps the PDF's exact font / position.
            const para = paragraphs.find((p) => p.id === o.runId)
            const lines = para?.lines ?? []
            for (let i = 0; i < lines.length; i++) {
              const line = lines[i]
              const text = o.lineTexts[i] ?? ''
              ops.push({
                op: 'replaceText',
                page: o.page,
                runId: line.runIds[0],
                newText: text,
                fitWidth: line.w,
              })
              for (const id of line.runIds.slice(1)) {
                ops.push({ op: 'replaceText', page: o.page, runId: id, newText: '' })
              }
            }
            // Extra typed lines beyond the original: bake below the last line.
            for (let i = lines.length; i < o.lineTexts.length; i++) {
              const last = lines[lines.length - 1]
              const y = (last?.y ?? 0) - (i - lines.length + 1) * (para?.leading ?? 14)
              ops.push({
                op: 'addText',
                page: o.page,
                x: para?.x ?? 72,
                y,
                w: 10000,
                h: 0,
                text: o.lineTexts[i],
                font: mapPdfFont(para?.fontName ?? 'Helvetica'),
                size: para?.fontSize ?? 12,
                bold: /bold|black|heavy|semibold|demi/i.test(para?.fontName ?? ''),
                italic: /italic|oblique/i.test(para?.fontName ?? ''),
                color: para?.color ?? '#1a1a1a',
                align: 'left',
                opacity: 1,
              })
            }
          } else if (
            o.font == null &&
            o.bold == null &&
            o.italic == null &&
            !o.text.includes('\n')
          ) {
            // Surgical single line: keep the PDF's own font matrix / baseline.
            ops.push({
              op: 'replaceText',
              page: o.page,
              runId: o.runId,
              newText: o.text,
              fitWidth: o.fitWidth,
            })
            for (const id of o.clearRunIds ?? []) {
              if (id === o.runId) continue
              ops.push({ op: 'replaceText', page: o.page, runId: id, newText: '' })
            }
          } else {
            for (const id of o.clearRunIds ?? []) {
              ops.push({ op: 'replaceText', page: o.page, runId: id, newText: '' })
            }
            const r = normRectToPdf(o.nx ?? 0, o.ny ?? 0, o.nw ?? 0.1, o.nh ?? 0.02, box)
            ops.push({
              op: 'whiteout',
              page: o.page,
              x: r.x,
              y: r.y,
              w: r.w,
              h: r.h,
              color: '#ffffff',
            })
            ops.push({
              op: 'addText',
              page: o.page,
              x: r.x,
              y: r.y,
              w: r.w,
              h: r.h,
              text: o.text,
              font: o.font ?? 'Helvetica',
              size: o.size,
              bold: o.bold,
              italic: o.italic,
              color: o.color,
              align: o.align,
              opacity: 1,
            })
          }
          continue
        }
        if (o.kind === 'line' && o.from && o.to) {
          const f = normRectToPdf(o.from[0], o.from[1], 0.001, 0.001, box)
          const t = normRectToPdf(o.to[0], o.to[1], 0.001, 0.001, box)
          ops.push({
            op: 'line',
            page: o.page,
            from: [f.x, f.y],
            to: [t.x, t.y],
            color: o.color,
            width: o.strokeWidth,
            arrow: o.arrow ?? 'none',
          })
          continue
        }
        if (o.kind === 'freeDraw' && o.paths) {
          ops.push({
            op: 'freeDraw',
            page: o.page,
            paths: o.paths.map((pts) =>
              pts.map(([nx, ny]) => {
                const p = normRectToPdf(nx, ny, 0.001, 0.001, box)
                return [p.x, p.y] as [number, number]
              }),
            ),
            color: o.color,
            width: o.strokeWidth,
            opacity: o.opacity,
          })
          continue
        }

        if (o.nx == null || o.ny == null || o.nw == null || o.nh == null) continue
        const r = normRectToPdf(o.nx, o.ny, o.nw, o.nh, box)

        switch (o.kind) {
          case 'addText':
            ops.push({
              op: 'addText',
              page: o.page,
              x: r.x,
              y: r.y,
              w: r.w,
              h: r.h,
              text: o.text ?? '',
              font: o.font,
              size: o.size,
              bold: o.bold,
              italic: o.italic,
              color: o.color,
              align: o.align,
              opacity: o.opacity,
            })
            break
          case 'highlight':
            ops.push({
              op: 'highlight',
              page: o.page,
              quads: [[r.x, r.y, r.w, r.h]],
              color: o.color,
              opacity: o.opacity,
            })
            break
          case 'underline':
            ops.push({
              op: 'underline',
              page: o.page,
              quads: [[r.x, r.y, r.w, r.h]],
              color: o.color,
            })
            break
          case 'strikeout':
            ops.push({
              op: 'strikeout',
              page: o.page,
              quads: [[r.x, r.y, r.w, r.h]],
              color: o.color,
            })
            break
          case 'note':
            ops.push({
              op: 'note',
              page: o.page,
              x: r.x,
              y: r.y,
              text: o.text ?? 'Nota',
              color: o.color,
            })
            break
          case 'whiteout':
            ops.push({
              op: 'whiteout',
              page: o.page,
              x: r.x,
              y: r.y,
              w: r.w,
              h: r.h,
              color: o.color ?? '#ffffff',
            })
            break
          case 'rect':
          case 'ellipse':
            ops.push({
              op: o.kind,
              page: o.page,
              x: r.x,
              y: r.y,
              w: r.w,
              h: r.h,
              stroke: o.stroke,
              fill: o.fill ?? null,
              strokeWidth: o.strokeWidth,
              opacity: o.opacity,
            })
            break
          case 'image':
            if (o.imagePath)
              ops.push({
                op: 'image',
                page: o.page,
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                imagePath: o.imagePath,
                rotation: o.rotation,
                opacity: o.opacity,
              })
            break
          case 'stamp':
            ops.push({
              op: 'stamp',
              page: o.page,
              x: r.x,
              y: r.y,
              w: r.w,
              h: r.h,
              stamp: o.stamp ?? 'aprobado',
              customText: o.customText ?? null,
              color: o.color,
            })
            break
        }
      }

      const out = output || path.replace(/\.pdf$/i, '') + '_editado.pdf'
      result = await runWithProgress(
        (p) => {
          progress = p
        },
        () => editPdf(path, out, ops, flatten),
      )
      output = out
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
      progress = null
    }
  }

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null
    const typing = !!t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)

    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z' && !typing) {
      e.preventDefault()
      if (e.shiftKey) redo()
      else undo()
      return
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y' && !typing) {
      e.preventDefault()
      redo()
      return
    }
    if (e.key === 'Escape') {
      if (editingRunId != null) editingRunId = null
      else if (editingObjId) {
        editingObjId = null
      } else selectedId = null
      return
    }
    if (typing) return
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault()
      deleteSelected()
      return
    }
    if (selected && e.key.startsWith('Arrow')) {
      e.preventDefault()
      const step = e.shiftKey ? 0.02 : 0.002
      const dx = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0
      const dy = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0
      if (selected.kind === 'line' && selected.from && selected.to) {
        updateSelected({
          from: [clamp01(selected.from[0] + dx), clamp01(selected.from[1] + dy)],
          to: [clamp01(selected.to[0] + dx), clamp01(selected.to[1] + dy)],
        })
      } else if (selected.nx != null && selected.ny != null) {
        updateSelected(
          clampBox(selected.nx + dx, selected.ny + dy, selected.nw ?? MIN_SIZE, selected.nh ?? MIN_SIZE),
        )
      }
    }
  }

  function objBackground(o: EditObject) {
    if (o.kind === 'highlight') return `${o.color ?? '#ffe066'}80`
    if (o.kind === 'whiteout') return '#ffffff'
    if (o.kind === 'rect' || o.kind === 'ellipse') return o.fill ?? 'transparent'
    return 'transparent'
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="ed-layout">
  <ResultBanner
    {loading}
    {error}
    {result}
    {progress}
    toolLabel="Editar"
    toolId="edit"
    inputs={paths}
    cancellable={false}
  />

  {#if !path}
    <FileDropZone
      bind:paths
      accept=".pdf"
      multiple={false}
      showPreview={false}
      label="Arrastra un PDF para editar"
    />
  {:else}
    <div class="ed-topbar">
      <div class="ed-modes">
        {#each MODES as m (m.id)}
          <button
            type="button"
            class="mp-chip"
            class:is-on={mode === m.id}
            onclick={() => {
              commitObjEdit()
              commitRunEdit()
              mode = m.id
            }}
          >
            {m.label}
          </button>
        {/each}
      </div>

      <div class="ed-actions">
        <button
          type="button"
          class="mp-btn mp-btn-ghost !min-h-8 !px-2"
          onclick={undo}
          disabled={!undoStack.length}
          title="Deshacer (Ctrl+Z)"
          aria-label="Deshacer"
        >
          ↶
        </button>
        <button
          type="button"
          class="mp-btn mp-btn-ghost !min-h-8 !px-2"
          onclick={redo}
          disabled={!redoStack.length}
          title="Rehacer (Ctrl+Y)"
          aria-label="Rehacer"
        >
          ↷
        </button>
        <button
          type="button"
          class="mp-btn mp-btn-ghost !min-h-8 !px-2"
          onclick={() => (paths = [])}
          title="Cerrar documento"
        >
          Cerrar
        </button>
      </div>
    </div>

    <p class="ed-hint">{activeHint}</p>

    <div class="ed-body">
      <aside class="ed-panel">
        {#if mode === 'annotate'}
          <div class="mp-field">
            <span>Marca</span>
            <div class="ed-chiprow">
              {#each [['highlight', 'Resaltar'], ['underline', 'Subrayar'], ['strikeout', 'Tachar'], ['note', 'Nota']] as [id, label] (id)}
                <button
                  type="button"
                  class="mp-chip"
                  class:is-on={annotSub === id}
                  onclick={() => (annotSub = id as AnnotSub)}
                >
                  {label}
                </button>
              {/each}
            </div>
          </div>
          {#if annotSub === 'highlight'}
            <div class="mp-field">
              <span>Color</span>
              <div class="ed-swatches">
                {#each HIGHLIGHTS as c (c)}
                  <button
                    type="button"
                    class="ed-swatch"
                    class:is-on={highlightColor === c}
                    style="background:{c}"
                    title="Resaltado {c}"
                    aria-label="Resaltado {c}"
                    onclick={() => {
                      highlightColor = c
                      if (selected?.kind === 'highlight') updateSelected({ color: c })
                    }}
                  ></button>
                {/each}
              </div>
            </div>
          {/if}
        {/if}

        {#if mode === 'shapes'}
          <div class="mp-field">
            <span>Forma</span>
            <div class="ed-chiprow">
              {#each [['rect', 'Rectángulo'], ['ellipse', 'Elipse'], ['line', 'Línea']] as [id, label] (id)}
                <button
                  type="button"
                  class="mp-chip"
                  class:is-on={shapeSub === id}
                  onclick={() => (shapeSub = id as ShapeSub)}
                >
                  {label}
                </button>
              {/each}
            </div>
          </div>
          <div class="mp-field">
            <span>Relleno</span>
            <div class="ed-swatches">
              <button
                type="button"
                class="ed-swatch is-none"
                class:is-on={fillColor === null}
                title="Sin relleno"
                aria-label="Sin relleno"
                onclick={() => {
                  fillColor = null
                  if (selected) updateSelected({ fill: null })
                }}
              ></button>
              {#each COLORS as c (c)}
                <button
                  type="button"
                  class="ed-swatch"
                  class:is-on={fillColor === c}
                  style="background:{c}"
                  title="Relleno {c}"
                  aria-label="Relleno {c}"
                  onclick={() => {
                    fillColor = c
                    if (selected) updateSelected({ fill: c })
                  }}
                ></button>
              {/each}
            </div>
          </div>
        {/if}

        {#if mode === 'text' || mode === 'edit' || selected?.kind === 'addText' || editingRunId != null}
          <h3 class="ed-title">{editingRunId != null ? 'Formato del párrafo' : 'Estilo'}</h3>
          <div
            class="ed-format"
            role="group"
            onmousedown={(e) => {
              if (editingRunId == null) return
              captureEditSelection()
              // Keep selection when clicking chips; inputs still need focus.
              if (
                !(e.target instanceof HTMLInputElement) &&
                !(e.target instanceof HTMLSelectElement)
              ) {
                e.preventDefault()
              }
            }}
          >
            <div class="mp-field">
              <span>Fuente</span>
              <select
                class="mp-input"
                bind:value={font}
                onchange={() => {
                  if (editingRunId != null) applySelectionFormat({ font })
                  else if (selected?.kind === 'addText') updateSelected({ font })
                }}
              >
                <option>Helvetica</option>
                <option>Times</option>
                <option>Courier</option>
              </select>
            </div>
            <div class="ed-row">
              <label class="mp-field ed-grow">
                <span>Tamaño</span>
                <input
                  class="mp-input"
                  type="number"
                  min="6"
                  max="96"
                  bind:value={fontSize}
                  onchange={() => {
                    if (editingRunId != null) applySelectionFormat({ size: fontSize })
                    else if (selected?.kind === 'addText') updateSelected({ size: fontSize })
                  }}
                />
              </label>
              <div class="ed-chiprow ed-shrink">
                <button
                  type="button"
                  class="mp-chip ed-bold"
                  class:is-on={bold}
                  title="Negrita (selección)"
                  onclick={() => {
                    if (editingRunId != null) applySelectionFormat({ bold: !bold })
                    else {
                      bold = !bold
                      if (selected?.kind === 'addText') updateSelected({ bold })
                    }
                  }}
                >
                  B
                </button>
                <button
                  type="button"
                  class="mp-chip ed-italic"
                  class:is-on={italic}
                  title="Cursiva (selección)"
                  onclick={() => {
                    if (editingRunId != null) applySelectionFormat({ italic: !italic })
                    else {
                      italic = !italic
                      if (selected?.kind === 'addText') updateSelected({ italic })
                    }
                  }}
                >
                  I
                </button>
              </div>
            </div>
            <div class="mp-field">
              <span>Alineación</span>
              <div class="ed-chiprow">
                {#each [['left', 'Izq.'], ['center', 'Centro'], ['right', 'Der.']] as [id, label] (id)}
                  <button
                    type="button"
                    class="mp-chip"
                    class:is-on={align === id}
                    onclick={() => {
                      align = id
                      if (selected?.kind === 'addText') updateSelected({ align: id })
                    }}
                  >
                    {label}
                  </button>
                {/each}
              </div>
            </div>
            <div class="mp-field">
              <span>Color de texto</span>
              <div class="ed-swatches">
                {#each COLORS as c (c)}
                  <button
                    type="button"
                    class="ed-swatch"
                    class:is-on={textColor === c}
                    style="background:{c}"
                    title="Texto {c}"
                    aria-label="Texto {c}"
                    onclick={() => {
                      textColor = c
                      if (editingRunId != null) applySelectionFormat({ color: c })
                      else if (selected?.kind === 'addText') updateSelected({ color: c })
                    }}
                  ></button>
                {/each}
              </div>
            </div>
          </div>
        {/if}

        {#if mode === 'shapes' || mode === 'draw' || mode === 'stamp'}
          <div class="mp-field">
            <span>Color de trazo</span>
            <div class="ed-swatches">
              {#each COLORS as c (c)}
                <button
                  type="button"
                  class="ed-swatch"
                  class:is-on={strokeColor === c}
                  style="background:{c}"
                  title="Trazo {c}"
                  aria-label="Trazo {c}"
                  onclick={() => {
                    strokeColor = c
                    if (selected) updateSelected({ stroke: c, color: c })
                  }}
                ></button>
              {/each}
            </div>
          </div>
        {/if}

        {#if mode === 'shapes' || mode === 'draw'}
          <label class="mp-field">
            <span>Grosor · {strokeWidth}pt</span>
            <input
              class="mp-range"
              type="range"
              min="0.5"
              max="8"
              step="0.5"
              bind:value={strokeWidth}
              onchange={() => selected && updateSelected({ strokeWidth })}
            />
          </label>
        {/if}

        {#if mode === 'shapes' || mode === 'draw' || mode === 'image'}
          <label class="mp-field">
            <span>Opacidad · {Math.round(opacity * 100)}%</span>
            <input
              class="mp-range"
              type="range"
              min="0.1"
              max="1"
              step="0.05"
              bind:value={opacity}
              onchange={() => selected && updateSelected({ opacity })}
            />
          </label>
        {/if}

        {#if mode === 'stamp'}
          <div class="mp-field">
            <span>Sello</span>
            <div class="ed-chiprow">
              {#each STAMPS as s (s)}
                <button
                  type="button"
                  class="mp-chip"
                  class:is-on={stampKind === s}
                  onclick={() => (stampKind = s)}
                >
                  {s}
                </button>
              {/each}
            </div>
          </div>
          <label class="mp-field">
            <span>Texto propio</span>
            <input class="mp-input" placeholder="Opcional" bind:value={stampCustom} />
          </label>
        {/if}

        {#if mode === 'form'}
          <h3 class="ed-title">Campos ({formFields.length})</h3>
          {#if !formFields.length}
            <p class="ed-note">Este PDF no tiene campos de formulario.</p>
          {:else}
            <p class="ed-note">Escribe directamente en los campos resaltados de la página.</p>
            <ul class="ed-items">
              {#each formFields as f (f.name + f.page + f.x)}
                <li class:is-on={f.page === page}>
                  <button type="button" class="ed-itembtn" onclick={() => focusField(f)}>
                    <span class="mono ed-badge">p{f.page}</span>
                    <span class="truncate" title={f.name}>{f.name}</span>
                  </button>
                  {#if formValues[f.name] && formValues[f.name] !== f.value}
                    <span class="ed-dot" title="Modificado"></span>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        {/if}

        {#if mode === 'edit'}
          <h3 class="ed-title">Texto de la página</h3>
          <p class="ed-note">
            {textLines.length} línea{textLines.length === 1 ? '' : 's'} de texto. Haz clic en
            cualquiera y escribe: se edita la línea entera, como en un procesador de textos.
          </p>
          <p class="ed-note">
            Los que se marcan en rojo usan una fuente incrustada que no se puede reutilizar: esos se
            taparán y reescribirán en Helvetica.
          </p>
        {/if}

        {#if selected}
          <div class="ed-selected">
            <h3 class="ed-title">Seleccionado</h3>
            <p class="ed-note truncate">{selected.label}</p>
            {#if selected.kind === 'addText'}
              <button
                type="button"
                class="mp-btn mp-btn-ghost w-full !min-h-8"
                onclick={() => startObjEdit(selected)}
              >
                Editar texto
              </button>
            {/if}
            <button
              type="button"
              class="mp-btn mp-btn-ghost w-full !min-h-8"
              onclick={deleteSelected}
            >
              Eliminar
            </button>
          </div>
        {/if}
      </aside>

      <main class="ed-main">
        <div class="ed-canvasbar">
          <span class="mono truncate ed-fname" title={path}>{fileName(path)}</span>
          <div class="ed-canvastools">
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              disabled={page <= 1}
              onclick={() => setPage(page - 1)}
              aria-label="Página anterior"
            >
              <Icon name="arrow-up" size={14} />
            </button>
            <span class="mono ed-small">{page} / {pageCount}</span>
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              disabled={page >= pageCount}
              onclick={() => setPage(page + 1)}
              aria-label="Página siguiente"
            >
              <Icon name="arrow-down" size={14} />
            </button>
            <span class="ed-sep"></span>
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              onclick={() => setZoom(zoom - 0.1)}
              aria-label="Alejar">−</button
            >
            <span class="mono ed-small">{Math.round(zoom * 100)}%</span>
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              onclick={() => setZoom(zoom + 0.1)}
              aria-label="Acercar">+</button
            >
          </div>
        </div>

        <div class="ed-viewport" bind:this={viewportEl} onwheel={onWheel}>
          <div
            class="ed-surface"
            class:is-crosshair={mode !== 'edit' && mode !== 'form'}
            bind:this={surfaceEl}
            bind:clientWidth={surfaceW}
            style="width: {zoom * 100}%;"
            role="application"
            aria-label="Lienzo de edición del PDF"
            onpointerdown={onSurfacePointerDown}
            onpointermove={onSurfacePointerMove}
            onpointerup={onSurfacePointerUp}
            onpointercancel={onSurfacePointerUp}
          >
            {#if previewUrl}
              <img src={previewUrl} alt="Página {page}" draggable="false" />
            {:else}
              <div class="ed-placeholder">Cargando…</div>
            {/if}

            {#if previewLoading}
              <div class="ed-loading">Cargando…</div>
            {/if}

            <!-- Existing PDF text: clickable in the modes that act on it -->
            {#if mode === 'edit' || (mode === 'annotate' && annotSub !== 'note') || mode === 'whiteout'}
              {#each paragraphs as para (para.id)}
                {#if editingRunId !== para.id && !replacedRunIds.has(para.id)}
                  {@const L = paragraphLayout(para)}
                  <button
                    type="button"
                    class="ed-run"
                    class:is-locked={!para.editable}
                    style="left:{L.coverLeft}px;top:{L.coverTop}px;width:{L.coverW}px;height:{L.coverH}px"
                    title={para.editable
                      ? para.text.split('\n')[0] + (para.lines.length > 1 ? '…' : '')
                      : `${para.text.split('\n')[0]} — fuente no editable: se tapará y reescribirá`}
                    aria-label="Editar párrafo"
                    onpointerdown={(e) => onRunPointerDown(e, para)}
                  ></button>
                {/if}
              {/each}
            {/if}

            <!-- Committed replacements, drawn as if they were part of the page -->
            {#each pageReplacements as o (o.id)}
              {@const para = paragraphs.find((p) => p.id === o.runId)}
              {#if para && editingRunId !== o.runId}
                {@const L = paragraphLayout(para, o.text ?? '')}
                {#if !previewFresh || o.bakeSpans?.length}
                  <div
                    class="ed-cover"
                    style="left:{L.coverLeft}px;top:{L.coverTop}px;width:{L.coverW}px;height:{L.coverH}px"
                  ></div>
                  <div
                    class="ed-inline ed-inline-done"
                    role="button"
                    tabindex="0"
                    style="left:{L.left}px;top:{L.top}px;width:{Math.max(L.coverW, 40)}px;font:{L.style} {L.weight} {L.fontPx}px/{L.leadingPx}px {L.family};color:{o.color ?? para.color};text-align:{o.align ?? 'left'};white-space:pre-wrap"
                    title="Clic para seguir editando"
                    onpointerdown={(e) => onRunPointerDown(e, para)}
                    onkeydown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault()
                        onRunPointerDown(e as unknown as PointerEvent, para)
                      }
                    }}
                  >
                    {#if o.richLines?.length}
                      {@html richToHtml(o.richLines, para)}
                    {:else}
                      {o.text}
                    {/if}
                  </div>
                {:else}
                  <!-- Preview shows the real edited text; just keep a transparent hit area. -->
                  <button
                    type="button"
                    class="ed-run"
                    style="left:{L.coverLeft}px;top:{L.coverTop}px;width:{L.coverW}px;height:{L.coverH}px"
                    title="Clic para seguir editando"
                    aria-label="Editar párrafo"
                    onpointerdown={(e) => onRunPointerDown(e, para)}
                  ></button>
                {/if}
              {/if}
            {/each}

            <!-- Edit straight on the page: match PDF metrics; format only selection via panel -->
            {#each paragraphs as para (para.id)}
              {#if editingRunId === para.id}
                {@const L = paragraphLayout(para)}
                {@const Lc = paragraphLayout(para, editDraft)}
                <div
                  class="ed-cover"
                  style="left:{Lc.coverLeft}px;top:{Lc.coverTop}px;width:{Lc.coverW}px;height:{Lc.coverH}px"
                ></div>
                <div
                  class="ed-textarea ed-textarea-para ed-rich"
                  contenteditable="true"
                  role="textbox"
                  tabindex="0"
                  aria-multiline="true"
                  aria-label="Editar párrafo del documento"
                  spellcheck="false"
                  style="left:{L.left}px;top:{L.top}px;width:{Math.max(L.coverW, 80)}px;height:{Math.max(Lc.coverH, L.leadingPx)}px;font:{L.style} {L.weight} {L.fontPx}px/{L.leadingPx}px {L.family};color:{para.color};text-align:left"
                  use:mountRichEditor={editRichSeed}
                  onmouseup={captureEditSelection}
                  onkeyup={captureEditSelection}
                  oninput={() => syncDraftFromEditor()}
                  onblur={(e) => {
                    const next = e.relatedTarget as HTMLElement | null
                    if (next?.closest?.('.ed-panel, .ed-textarea, .ed-rich')) return
                    commitRunEdit(para.id)
                  }}
                  onkeydown={(e) => {
                    if (e.key === 'Escape') {
                      e.preventDefault()
                      editingRunId = null
                      editEl = null
                    }
                  }}
                ></div>
              {/if}
            {/each}

            <!-- Vector objects -->
            <svg class="ed-vector" viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true">
              {#each pageObjects as o (o.id)}
                {#if o.kind === 'freeDraw' && o.paths}
                  {#each o.paths as pts, i (i)}
                    <polyline
                      fill="none"
                      stroke={o.color ?? '#e11d48'}
                      stroke-width={(o.strokeWidth ?? 1.5) / 600}
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      opacity={o.opacity ?? 1}
                      points={pts.map((p) => `${p[0]},${p[1]}`).join(' ')}
                    />
                  {/each}
                {:else if o.kind === 'line' && o.from && o.to}
                  <line
                    x1={o.from[0]}
                    y1={o.from[1]}
                    x2={o.to[0]}
                    y2={o.to[1]}
                    stroke={o.color ?? '#e11d48'}
                    stroke-width={(o.strokeWidth ?? 1.5) / 600}
                    stroke-linecap="round"
                  />
                {/if}
              {/each}
              {#if draftPath && draftPath.length > 1}
                <polyline
                  fill="none"
                  stroke={strokeColor}
                  stroke-width={strokeWidth / 600}
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  points={draftPath.map((p) => `${p[0]},${p[1]}`).join(' ')}
                />
              {/if}
            </svg>

            <!-- Line hit areas + endpoints -->
            {#each pageObjects.filter((o) => o.kind === 'line' && o.from && o.to) as o (o.id)}
              {@const x1 = (o.from as [number, number])[0]}
              {@const y1 = (o.from as [number, number])[1]}
              {@const x2 = (o.to as [number, number])[0]}
              {@const y2 = (o.to as [number, number])[1]}
              <div
                class="ed-linehit"
                class:is-selected={selectedId === o.id}
                style="left:{Math.min(x1, x2) * 100}%;top:{Math.min(y1, y2) * 100}%;width:{Math.abs(
                  x2 - x1,
                ) * 100}%;height:{Math.abs(y2 - y1) * 100}%;"
                role="button"
                tabindex="0"
                aria-label={o.label}
                onpointerdown={(e) => startMove(e, o)}
              ></div>
              {#if selectedId === o.id}
                <span
                  class="ed-handle ed-endpoint"
                  style="left:{x1 * 100}%;top:{y1 * 100}%"
                  role="presentation"
                  onpointerdown={(e) => startEndpoint(e, o, 'from')}
                ></span>
                <span
                  class="ed-handle ed-endpoint"
                  style="left:{x2 * 100}%;top:{y2 * 100}%"
                  role="presentation"
                  onpointerdown={(e) => startEndpoint(e, o, 'to')}
                ></span>
              {/if}
            {/each}

            <!-- Box objects -->
            {#each pageObjects.filter((o) => o.kind !== 'line' && o.kind !== 'freeDraw' && o.kind !== 'replaceText' && o.nx != null) as o (o.id)}
              <div
                class="ed-obj ed-kind-{o.kind}"
                class:is-selected={selectedId === o.id}
                style="left:{(o.nx ?? 0) * 100}%;top:{(o.ny ?? 0) * 100}%;width:{(o.nw ?? 0.05) *
                  100}%;height:{(o.nh ?? 0.05) * 100}%;background:{objBackground(
                  o,
                )};--ed-stroke:{o.stroke ?? o.color ?? 'var(--color-accent-ink, #1a1a1a)'};opacity:{o.kind ===
                'highlight'
                  ? 1
                  : (o.opacity ?? 1)}"
                role="button"
                tabindex="0"
                aria-label={o.label}
                onpointerdown={(e) => startMove(e, o)}
                ondblclick={() => startObjEdit(o)}
              >
                {#if o.kind === 'addText' && editingObjId !== o.id}
                  <span
                    class="ed-objtext"
                    style="color:{o.color};font-size:{Math.max(
                      ptToPx(o.size ?? 12),
                      7,
                    )}px;font-weight:{o.bold ? 700 : 400};font-style:{o.italic
                      ? 'italic'
                      : 'normal'};text-align:{o.align ?? 'left'};font-family:{o.font === 'Times'
                      ? 'Georgia, serif'
                      : o.font === 'Courier'
                        ? 'ui-monospace, monospace'
                        : 'Helvetica, Arial, sans-serif'}"
                  >
                    {o.text || 'Tu texto aquí'}
                  </span>
                {:else if o.kind === 'stamp'}
                  <span class="ed-stamp" style="color:{o.color};border-color:{o.color}">
                    {o.customText || (o.stamp ?? '').toUpperCase()}
                  </span>
                {:else if o.kind === 'note'}
                  <span class="ed-notedot" title={o.text}>N</span>
                {:else if o.kind === 'image'}
                  {#if imageUrls[o.imagePath ?? '']}
                    <img
                      class="ed-img"
                      src={imageUrls[o.imagePath ?? '']}
                      alt={o.label}
                      draggable="false"
                    />
                  {:else}
                    <span class="ed-imgfallback">{o.label}</span>
                  {/if}
                {:else if o.kind === 'underline'}
                  <span class="ed-rule" style="background:{o.color}"></span>
                {:else if o.kind === 'strikeout'}
                  <span class="ed-strike" style="background:{o.color}"></span>
                {/if}

                {#if editingObjId === o.id}
                  <textarea
                    class="ed-objinput"
                    style="font:{o.italic ? 'italic' : 'normal'} {o.bold
                      ? 700
                      : 400} {Math.max(ptToPx(o.size ?? 12), 9)}px/1.25 {cssFamily(
                      o.font ?? 'Helvetica',
                    )};color:{o.color};text-align:{o.align ?? 'left'}"
                    bind:value={editDraft}
                    use:autofocus
                    onpointerdown={(e) => e.stopPropagation()}
                    onblur={commitObjEdit}
                    onkeydown={(e) => {
                      if (e.key === 'Escape') {
                        e.preventDefault()
                        commitObjEdit()
                      }
                      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                        e.preventDefault()
                        commitObjEdit()
                      }
                    }}
                  ></textarea>
                {/if}

                {#if selectedId === o.id && editingObjId !== o.id}
                  {#each HANDLES as h (h)}
                    <span
                      class="ed-handle ed-h-{h}"
                      role="presentation"
                      onpointerdown={(e) => startResize(e, o, h)}
                    ></span>
                  {/each}
                {/if}
              </div>
            {/each}

            <!-- AcroForm widgets, filled in place like the signature tool -->
            {#if mode === 'form'}
              {#each pageFields as f (f.name + f.x + f.y)}
                {@const F = fieldLayout(f)}
                {#if f.kind === 'signature'}
                  <span
                    class="ed-field ed-field-sig"
                    style="left:{F.left}px;top:{F.top}px;width:{F.w}px;height:{F.h}px;font-size:{Math.min(
                      F.fontPx,
                      12,
                    )}px"
                    title="Campo de firma: usa la herramienta Firmar">Firma</span
                  >
                {:else if f.kind === 'checkbox' || f.kind === 'radio'}
                  <input
                    class="ed-field ed-field-check"
                    type="checkbox"
                    data-field={f.name}
                    title={f.name}
                    aria-label={f.name}
                    style="left:{F.left}px;top:{F.top}px;width:{F.w}px;height:{F.h}px"
                    checked={isChecked(formValues[f.name])}
                    onchange={(e) => setField(f.name, e.currentTarget.checked ? 'Yes' : 'Off')}
                  />
                {:else if f.kind === 'choice' && f.options.length}
                  <select
                    class="ed-field"
                    data-field={f.name}
                    title={f.name}
                    aria-label={f.name}
                    style="left:{F.left}px;top:{F.top}px;width:{F.w}px;height:{F.h}px;font-size:{F.fontPx}px"
                    value={formValues[f.name] ?? ''}
                    onchange={(e) => setField(f.name, e.currentTarget.value)}
                  >
                    {#each f.options as opt (opt)}
                      <option value={opt}>{opt}</option>
                    {/each}
                  </select>
                {:else}
                  <input
                    class="ed-field"
                    data-field={f.name}
                    title={f.name}
                    aria-label={f.name}
                    style="left:{F.left}px;top:{F.top}px;width:{F.w}px;height:{F.h}px;font-size:{F.fontPx}px"
                    value={formValues[f.name] ?? ''}
                    oninput={(e) => setField(f.name, e.currentTarget.value)}
                  />
                {/if}
              {/each}
            {/if}

            {#if draftBox && (draftBox.nw > 0.004 || draftBox.nh > 0.004)}
              <div
                class="ed-draft"
                style="left:{draftBox.nx * 100}%;top:{draftBox.ny * 100}%;width:{draftBox.nw *
                  100}%;height:{draftBox.nh * 100}%"
              ></div>
            {/if}
          </div>
        </div>

        <div class="ed-savebar">
          <OutputPicker
            bind:value={output}
            tool="edit"
            defaultName={fileName(path).replace(/\.pdf$/i, '') + '_editado.pdf'}
          />
          <label class="mp-check ed-check-compact">
            <input type="checkbox" bind:checked={flatten} />
            <span class="mp-check-box">
              <svg viewBox="0 0 12 12" fill="none" aria-hidden="true">
                <path
                  d="M2 6.5L4.5 9L10 3"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </span>
            <span class="mp-check-label">Aplanar anotaciones</span>
          </label>
          <button
            type="button"
            class="mp-btn mp-btn-primary"
            onclick={() => void run()}
            disabled={loading || !dirty}
          >
            Guardar cambios
          </button>
        </div>
      </main>

      <aside class="ed-panel ed-list">
        <div class="ed-listhead">
          <h3 class="ed-title">Cambios ({objects.length})</h3>
          {#if pageObjects.length}
            <button type="button" class="mp-btn mp-btn-ghost !min-h-7 !px-2" onclick={clearPage}>
              Limpiar pág.
            </button>
          {/if}
        </div>
        {#if !objects.length}
          <p class="ed-note">Todavía no hay cambios. Elige un modo arriba y actúa sobre la página.</p>
        {:else}
          <ul class="ed-items">
            {#each objects as o (o.id)}
              <li class:is-on={selectedId === o.id}>
                <button
                  type="button"
                  class="ed-itembtn"
                  onclick={() => {
                    if (o.page !== page) void setPage(o.page)
                    selectedId = o.id
                  }}
                >
                  <span class="mono ed-badge">p{o.page}</span>
                  <span class="truncate">{o.label}</span>
                </button>
                <button
                  type="button"
                  class="mp-btn mp-btn-ghost !min-h-7 !px-2"
                  onclick={() => removeObject(o.id)}
                  aria-label="Quitar {o.label}"
                >
                  ×
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </aside>
    </div>
  {/if}
</div>

<style>
  .ed-layout {
    display: flex;
    flex-direction: column;
    gap: var(--space-3, 0.75rem);
  }

  .ed-topbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2, 0.5rem);
    border: 2px solid var(--color-ink, #1a1a1a);
    background: var(--color-paper, #fff);
    padding: 0.4rem 0.5rem;
    box-shadow: var(--shadow-stamp);
  }

  .ed-modes,
  .ed-actions,
  .ed-chiprow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.3rem;
  }

  .ed-hint {
    margin: 0;
    font-size: var(--text-xs, 0.75rem);
    color: var(--color-ink-2, #555);
  }

  .ed-body {
    display: grid;
    grid-template-columns: minmax(210px, 240px) minmax(0, 1fr) minmax(190px, 220px);
    gap: var(--space-3, 0.75rem);
    align-items: start;
  }

  @media (max-width: 1200px) {
    .ed-body {
      grid-template-columns: minmax(200px, 230px) minmax(0, 1fr);
    }
    .ed-list {
      grid-column: 1 / -1;
    }
  }

  @media (max-width: 860px) {
    .ed-body {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .ed-panel {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    border: 2px solid var(--color-ink, #1a1a1a);
    background: var(--color-paper, #fff);
    padding: 0.65rem;
    box-shadow: var(--shadow-stamp);
    max-height: min(78vh, 780px);
    overflow: auto;
  }

  .ed-row {
    display: flex;
    align-items: flex-end;
    gap: 0.4rem;
  }

  .ed-grow {
    flex: 1 1 auto;
    min-width: 0;
  }

  .ed-shrink {
    flex: 0 0 auto;
  }

  .ed-bold {
    font-weight: 800;
  }

  .ed-italic {
    font-style: italic;
  }

  .ed-format {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .ed-title {
    margin: 0;
    font-size: var(--text-xs, 0.75rem);
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-ink-2, #555);
  }

  .ed-note {
    margin: 0;
    font-size: var(--text-xs, 0.75rem);
    color: var(--color-ink-2, #555);
    line-height: 1.4;
  }

  .ed-selected {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    border-top: 1.5px dashed var(--color-rule, #cfc6b4);
    padding-top: 0.5rem;
    margin-top: auto;
  }

  .ed-swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }

  .ed-swatch {
    width: 20px;
    height: 20px;
    border: 1.5px solid var(--color-ink, #1a1a1a);
    cursor: pointer;
    padding: 0;
  }

  .ed-swatch.is-on {
    box-shadow: 0 0 0 2px var(--color-accent, #f5d76e);
  }

  .ed-swatch.is-none {
    background: repeating-linear-gradient(45deg, #fff 0 4px, #ddd 4px 8px);
  }

  .ed-main {
    display: flex;
    flex-direction: column;
    gap: var(--space-2, 0.5rem);
    min-width: 0;
  }

  .ed-canvasbar,
  .ed-savebar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
  }

  .ed-canvasbar {
    justify-content: space-between;
  }

  .ed-fname {
    font-size: var(--text-xs, 0.75rem);
    max-width: 45%;
  }

  .ed-canvastools {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .ed-small {
    font-size: var(--text-xs, 0.75rem);
  }

  .ed-sep {
    width: 1.5px;
    height: 18px;
    background: var(--color-rule, #cfc6b4);
    margin-inline: 0.25rem;
  }

  .ed-viewport {
    overflow: auto;
    max-height: min(74vh, 760px);
    border: 2px solid var(--color-ink, #1a1a1a);
    box-shadow: var(--shadow-stamp);
    background: var(--color-paper-2, #f3efe6);
    padding: var(--space-2, 0.5rem);
  }

  .ed-surface {
    position: relative;
    display: block;
    margin-inline: auto;
    max-width: none;
    user-select: none;
    touch-action: none;
    background: #fff;
    border: 1.5px solid var(--color-rule, #cfc6b4);
    line-height: 0;
  }

  .ed-surface.is-crosshair {
    cursor: crosshair;
  }

  .ed-surface img {
    display: block;
    width: 100%;
    height: auto;
    pointer-events: none;
    -webkit-user-drag: none;
  }

  .ed-placeholder {
    width: 100%;
    aspect-ratio: 3 / 4;
    display: grid;
    place-items: center;
    font-size: var(--text-xs, 0.75rem);
    color: var(--color-ink-2, #555);
    line-height: normal;
  }

  .ed-loading {
    position: absolute;
    top: 0.4rem;
    right: 0.4rem;
    z-index: 12;
    background: var(--color-accent, #f5d76e);
    border: 1.5px solid var(--color-ink, #1a1a1a);
    padding: 0.1rem 0.4rem;
    font-size: var(--text-xs, 0.75rem);
    font-weight: 800;
    line-height: normal;
  }

  .ed-vector {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 4;
    overflow: visible;
  }

  .ed-run {
    position: absolute;
    padding: 0;
    margin: 0;
    border: 1px solid transparent;
    background: color-mix(in srgb, var(--color-accent, #f5d76e) 12%, transparent);
    cursor: text;
    z-index: 3;
    box-sizing: border-box;
    transition: background 120ms ease, border-color 120ms ease;
  }

  .ed-run:hover,
  .ed-run:focus-visible {
    border-color: var(--color-accent-ink, #1a1a1a);
    background: color-mix(in srgb, var(--color-accent, #f5d76e) 35%, transparent);
    outline: none;
  }

  .ed-run.is-locked:hover {
    border-style: dotted;
    background: color-mix(in srgb, #e11d48 18%, transparent);
  }

  /* Hides the rasterised glyphs so the live text can take their place.
     Sits above the page bitmap but below annotations, which must stay visible
     over edited words. */
  .ed-cover {
    position: absolute;
    background: #fff;
    pointer-events: none;
    z-index: 2;
  }

  .ed-inline {
    position: absolute;
    z-index: 7;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    white-space: pre;
    transform-origin: 0 0;
    text-align: left;
    cursor: text;
    outline: none;
    min-width: 1px;
  }

  .ed-inline-done:hover {
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-ink, #1a1a1a) 35%, transparent);
  }

  /* The live editor: a textarea sitting exactly on the paragraph being edited. */
  .ed-textarea {
    position: absolute;
    z-index: 20;
    margin: 0;
    padding: 0 1px;
    border: 1px solid var(--color-accent-ink, #1a1a1a);
    background: #fff;
    outline: none;
    resize: none;
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-accent, #f5d76e) 60%, transparent);
    white-space: pre;
    overflow: hidden;
    line-height: 1;
    caret-color: var(--color-ink, #1a1a1a);
    /* Undo any inherited font normalization so the textarea matches the PDF. */
    font-kerning: none;
    font-variant-ligatures: none;
    text-rendering: optimizeSpeed;
  }

  /* Paragraph mode: multi-line, wraps naturally, scrolls if the user adds
     more text than fits. */
  .ed-textarea-para {
    white-space: pre-wrap;
    overflow-y: auto;
    min-height: 1.2em;
  }

  .ed-rich {
    display: block;
    cursor: text;
    word-break: break-word;
  }

  .ed-rich:focus {
    outline: none;
  }

  .ed-rich :global(div) {
    margin: 0;
    min-height: 1em;
  }

  .ed-rich :global(span) {
    line-height: inherit;
  }

  .ed-inline-done :global(div) {
    margin: 0;
  }

  .ed-obj {
    position: absolute;
    box-sizing: border-box;
    border: 1.5px solid transparent;
    cursor: move;
    z-index: 5;
    line-height: normal;
    display: flex;
    align-items: flex-start;
    overflow: visible;
  }

  .ed-obj.is-selected {
    outline: 1.5px dashed var(--color-ink, #1a1a1a);
    outline-offset: 1px;
    z-index: 10;
  }

  /* Only true shapes show their stroke as a border; highlights, whiteout,
     text and stamps never have a visible frame so they don't look like boxes. */
  .ed-kind-rect,
  .ed-kind-ellipse {
    border-color: var(--ed-stroke, transparent);
  }

  .ed-kind-ellipse {
    border-radius: 50%;
  }

  .ed-kind-whiteout {
    border-color: transparent;
  }

  .ed-kind-highlight,
  .ed-kind-underline,
  .ed-kind-strikeout,
  .ed-kind-addText,
  .ed-kind-replaceText,
  .ed-kind-note,
  .ed-kind-stamp,
  .ed-kind-image {
    border-color: transparent;
  }

  .ed-kind-addText:hover,
  .ed-kind-replaceText:hover {
    border-color: color-mix(in srgb, var(--color-ink, #1a1a1a) 35%, transparent);
    border-style: dashed;
  }

  .ed-objtext {
    display: block;
    width: 100%;
    white-space: pre-wrap;
    word-break: break-word;
    line-height: 1.25;
    pointer-events: none;
  }

  .ed-replaced {
    background: #fff;
    box-shadow: 0 0 0 1px color-mix(in srgb, #16a34a 60%, transparent);
  }

  .ed-objinput {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    min-height: 100%;
    resize: none;
    border: 0;
    outline: 1.5px dashed var(--color-accent-ink, #1a1a1a);
    background: color-mix(in srgb, #fff 88%, transparent);
    padding: 0;
    z-index: 21;
  }

  .ed-stamp {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    border: 2.5px solid currentColor;
    font-weight: 900;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    font-size: clamp(8px, 1.6vw, 15px);
    transform: rotate(-8deg);
    opacity: 0.85;
    pointer-events: none;
  }

  .ed-notedot {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    background: var(--color-accent, #f5d76e);
    border: 1.5px solid var(--color-ink, #1a1a1a);
    font-size: 10px;
    font-weight: 900;
    pointer-events: none;
  }

  .ed-img {
    width: 100%;
    height: 100%;
    object-fit: fill;
    pointer-events: none;
  }

  .ed-imgfallback {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    border: 1.5px dashed var(--color-ink, #1a1a1a);
    font-size: 10px;
    pointer-events: none;
  }

  .ed-rule {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
  }

  .ed-strike {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 2px;
  }

  .ed-linehit {
    position: absolute;
    z-index: 5;
    cursor: move;
    min-width: 8px;
    min-height: 8px;
    margin: -4px 0 0 -4px;
    padding: 4px;
    box-sizing: content-box;
  }

  .ed-linehit.is-selected {
    outline: 1.5px dashed var(--color-ink, #1a1a1a);
  }

  .ed-handle {
    position: absolute;
    width: 10px;
    height: 10px;
    background: var(--color-accent, #f5d76e);
    border: 1.5px solid var(--color-ink, #1a1a1a);
    box-sizing: border-box;
    z-index: 15;
  }

  .ed-endpoint {
    border-radius: 50%;
    margin: -5px 0 0 -5px;
  }

  .ed-h-nw {
    left: -5px;
    top: -5px;
    cursor: nwse-resize;
  }
  .ed-h-n {
    left: calc(50% - 5px);
    top: -5px;
    cursor: ns-resize;
  }
  .ed-h-ne {
    right: -5px;
    top: -5px;
    cursor: nesw-resize;
  }
  .ed-h-e {
    right: -5px;
    top: calc(50% - 5px);
    cursor: ew-resize;
  }
  .ed-h-se {
    right: -5px;
    bottom: -5px;
    cursor: nwse-resize;
  }
  .ed-h-s {
    left: calc(50% - 5px);
    bottom: -5px;
    cursor: ns-resize;
  }
  .ed-h-sw {
    left: -5px;
    bottom: -5px;
    cursor: nesw-resize;
  }
  .ed-h-w {
    left: -5px;
    top: calc(50% - 5px);
    cursor: ew-resize;
  }

  .ed-field {
    position: absolute;
    z-index: 8;
    box-sizing: border-box;
    margin: 0;
    padding: 0 2px;
    border: 1px solid color-mix(in srgb, #2563eb 55%, transparent);
    background: color-mix(in srgb, #2563eb 10%, transparent);
    font-family: Helvetica, Arial, sans-serif;
    line-height: 1;
    color: var(--color-ink, #1a1a1a);
    border-radius: 1px;
  }

  .ed-field:focus {
    outline: 2px solid var(--color-accent-ink, #1a1a1a);
    outline-offset: 0;
    background: #fff;
  }

  .ed-field-check {
    padding: 0;
    accent-color: var(--color-accent-ink, #1a1a1a);
    cursor: pointer;
  }

  .ed-field-sig {
    display: grid;
    place-items: center;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: color-mix(in srgb, #2563eb 85%, #000);
    pointer-events: none;
  }

  .ed-dot {
    flex: 0 0 auto;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #16a34a;
    border: 1.5px solid var(--color-ink, #1a1a1a);
  }

  .ed-draft {
    position: absolute;
    border: 2px dashed var(--color-ink, #1a1a1a);
    background: color-mix(in srgb, var(--color-accent, #f5d76e) 25%, transparent);
    pointer-events: none;
    z-index: 9;
  }

  .ed-savebar {
    justify-content: flex-end;
  }

  .ed-check-compact {
    min-height: 32px;
    font-size: var(--text-xs, 0.75rem);
    white-space: nowrap;
    gap: 0.35rem;
  }

  .ed-listhead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
  }

  .ed-items {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .ed-items li {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    border: 1.5px solid transparent;
    padding: 0.1rem 0.2rem;
  }

  .ed-items li.is-on {
    border-color: var(--color-ink, #1a1a1a);
    background: var(--color-accent-soft, #fdf3d0);
  }

  .ed-itembtn {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    flex: 1 1 auto;
    min-width: 0;
    background: none;
    border: none;
    padding: 0.15rem;
    cursor: pointer;
    font: inherit;
    font-size: var(--text-xs, 0.75rem);
    text-align: left;
    color: inherit;
  }

  .ed-badge {
    flex: 0 0 auto;
    font-size: 10px;
    border: 1.5px solid var(--color-rule, #cfc6b4);
    padding: 0 0.2rem;
  }

  @media (prefers-reduced-motion: reduce) {
    .ed-obj,
    .ed-run {
      transition: none;
    }
  }
</style>
