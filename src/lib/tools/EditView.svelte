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
  import { previewRenderWidth } from '../previewScale'
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
    font?: string
    size?: number
    bold?: boolean
    italic?: boolean
    color?: string
    align?: string
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
    { id: 'edit', label: 'Editar texto', hint: 'Clic en cualquier texto del PDF para reescribirlo.' },
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
  let renderWidth = $state(previewRenderWidth(1))
  let zoomTimer: ReturnType<typeof setTimeout> | null = null

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
  let imageUrls = $state<Record<string, string>>({})

  const path = $derived(paths[0] ?? '')
  const selected = $derived(objects.find((o) => o.id === selectedId) ?? null)
  const pageObjects = $derived(objects.filter((o) => o.page === page))
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
    } catch {
      textRuns = []
    }
  }

  async function setPage(pg: number) {
    if (!path || pg < 1 || pg > pageCount || pg === page) return
    commitObjEdit()
    commitRunEdit()
    page = pg
    selectedId = null
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
    zoom = Math.min(3, Math.max(0.5, Math.round(next * 20) / 20))
    if (zoomTimer) clearTimeout(zoomTimer)
    zoomTimer = setTimeout(() => {
      const w = previewRenderWidth(zoom)
      if (w === renderWidth || !path) return
      renderWidth = w
      previewLoading = true
      void loadPage(path, page).finally(() => {
        previewLoading = false
      })
    }, 200)
  }

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

  /**
   * A run's `y` is its baseline; glyphs span roughly 0.80 above and 0.24 below,
   * so the clickable box has to be built around it rather than from it.
   */
  function runToNorm(run: TextRun) {
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
  }

  function redo() {
    if (!redoStack.length) return
    undoStack = [...undoStack, $state.snapshot(objects) as EditObject[]]
    objects = redoStack[redoStack.length - 1]
    redoStack = redoStack.slice(0, -1)
    selectedId = null
    editingObjId = null
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
    if (target.closest('.ed-obj, .ed-handle, .ed-run, .ed-runedit, .ed-linehit')) return

    commitObjEdit()
    selectedId = null
    editingRunId = null

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

  function onRunPointerDown(e: PointerEvent, run: TextRun) {
    if (e.button !== 0) return
    e.stopPropagation()
    if (mode === 'edit') {
      commitRunEdit()
      const existing = objects.find(
        (o) => o.kind === 'replaceText' && o.page === page && o.runId === run.runId,
      )
      editingRunId = run.runId
      editDraft = existing?.text ?? run.text
      selectedId = null
      return
    }
    if (mode === 'annotate' && annotSub !== 'note') {
      makeAnnot(runToNorm(run), `${annotSub}: ${run.text.slice(0, 24)}`)
      return
    }
    if (mode === 'whiteout') {
      addObject({
        id: uid(),
        kind: 'whiteout',
        page,
        label: `Borrado: ${run.text.slice(0, 20)}`,
        ...runToNorm(run),
        color: '#ffffff',
      })
    }
  }

  function commitRunEdit() {
    const runId = editingRunId
    editingRunId = null
    if (runId == null) return
    const run = textRuns.find((r) => r.runId === runId)
    if (!run) return
    const next = editDraft
    if (next === run.text) {
      objects = objects.filter(
        (o) => !(o.kind === 'replaceText' && o.page === page && o.runId === runId),
      )
      return
    }
    pushUndo()
    const n = runToNorm(run)
    objects = [
      ...objects.filter((o) => !(o.kind === 'replaceText' && o.page === page && o.runId === runId)),
      {
        id: uid(),
        kind: 'replaceText',
        page,
        label: `«${run.text.slice(0, 20)}» → «${next.slice(0, 20)}»`,
        runId,
        text: next,
        ...n,
        size: run.fontSize,
        color: run.color,
      },
    ]
  }

  function revertRun(runId: number) {
    pushUndo()
    objects = objects.filter(
      (o) => !(o.kind === 'replaceText' && o.page === page && o.runId === runId),
    )
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
          ops.push({ op: 'replaceText', page: o.page, runId: o.runId, newText: o.text })
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

        {#if mode === 'text' || selected?.kind === 'addText'}
          <div class="mp-field">
            <span>Fuente</span>
            <select
              class="mp-input"
              bind:value={font}
              onchange={() => selected?.kind === 'addText' && updateSelected({ font })}
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
                onchange={() => selected?.kind === 'addText' && updateSelected({ size: fontSize })}
              />
            </label>
            <div class="ed-chiprow ed-shrink">
              <button
                type="button"
                class="mp-chip ed-bold"
                class:is-on={bold}
                title="Negrita"
                onclick={() => {
                  bold = !bold
                  if (selected?.kind === 'addText') updateSelected({ bold })
                }}
              >
                B
              </button>
              <button
                type="button"
                class="mp-chip ed-italic"
                class:is-on={italic}
                title="Cursiva"
                onclick={() => {
                  italic = !italic
                  if (selected?.kind === 'addText') updateSelected({ italic })
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
                    if (selected?.kind === 'addText') updateSelected({ color: c })
                  }}
                ></button>
              {/each}
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
              class="mp-input"
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
              class="mp-input"
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
            {#each formFields.filter((f) => f.kind !== 'signature') as f (f.name)}
              <label class="mp-field">
                <span class="truncate" title={f.name}>{f.name}</span>
                {#if f.kind === 'checkbox'}
                  <input
                    type="checkbox"
                    checked={formValues[f.name] === 'Yes' ||
                      formValues[f.name] === 'On' ||
                      formValues[f.name] === 'true'}
                    onchange={(e) => {
                      formValues = {
                        ...formValues,
                        [f.name]: e.currentTarget.checked ? 'Yes' : 'Off',
                      }
                    }}
                  />
                {:else}
                  <input
                    class="mp-input"
                    value={formValues[f.name] ?? ''}
                    oninput={(e) => {
                      formValues = { ...formValues, [f.name]: e.currentTarget.value }
                    }}
                  />
                {/if}
              </label>
            {/each}
          {/if}
        {/if}

        {#if mode === 'edit'}
          <h3 class="ed-title">Texto de la página</h3>
          <p class="ed-note">
            {textRuns.length} fragmento{textRuns.length === 1 ? '' : 's'} detectado{textRuns.length ===
            1
              ? ''
              : 's'}. Los marcados con candado se taparán y reescribirán con Helvetica.
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
              {#each textRuns as tr (tr.runId)}
                {@const n = runToNorm(tr)}
                <button
                  type="button"
                  class="ed-run"
                  class:is-locked={!tr.editable}
                  class:is-replaced={replacedRunIds.has(tr.runId)}
                  style="left:{n.nx * 100}%;top:{n.ny * 100}%;width:{n.nw * 100}%;height:{n.nh *
                    100}%"
                  title={tr.editable
                    ? tr.text
                    : `${tr.text} — fuente no editable: se tapará y reescribirá`}
                  aria-label="Editar «{tr.text}»"
                  onpointerdown={(e) => onRunPointerDown(e, tr)}
                ></button>
              {/each}
            {/if}

            <!-- In-place editor for an existing run -->
            {#if editingRunId != null}
              {@const tr = textRuns.find((r) => r.runId === editingRunId)}
              {#if tr}
                {@const n = runToNorm(tr)}
                <div
                  class="ed-runedit"
                  style="left:{n.nx * 100}%;top:{n.ny * 100}%;min-width:{Math.max(
                    n.nw * 100,
                    10,
                  )}%;"
                >
                  <input
                    class="ed-runinput"
                    style="font-size:{Math.max(ptToPx(tr.h), 9)}px"
                    bind:value={editDraft}
                    use:autofocus
                    onkeydown={(e) => {
                      if (e.key === 'Enter') {
                        e.preventDefault()
                        commitRunEdit()
                      }
                      if (e.key === 'Escape') {
                        e.preventDefault()
                        editingRunId = null
                      }
                    }}
                  />
                  <button
                    type="button"
                    class="mp-btn mp-btn-primary !min-h-7 !px-2"
                    onclick={commitRunEdit}
                  >
                    OK
                  </button>
                  {#if replacedRunIds.has(tr.runId)}
                    <button
                      type="button"
                      class="mp-btn mp-btn-ghost !min-h-7 !px-2"
                      onclick={() => {
                        editingRunId = null
                        revertRun(tr.runId)
                      }}
                    >
                      Revertir
                    </button>
                  {/if}
                </div>
              {/if}
            {/if}

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
            {#each pageObjects.filter((o) => o.kind !== 'line' && o.kind !== 'freeDraw' && o.nx != null) as o (o.id)}
              <div
                class="ed-obj ed-kind-{o.kind}"
                class:is-selected={selectedId === o.id}
                style="left:{(o.nx ?? 0) * 100}%;top:{(o.ny ?? 0) * 100}%;width:{(o.nw ?? 0.05) *
                  100}%;height:{(o.nh ?? 0.05) * 100}%;background:{objBackground(
                  o,
                )};border-color:{o.stroke ?? o.color ?? 'var(--color-accent-ink, #1a1a1a)'};opacity:{o.kind ===
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
                {:else if o.kind === 'replaceText'}
                  <span
                    class="ed-objtext ed-replaced"
                    style="font-size:{Math.max(ptToPx(o.size ?? 11), 7)}px;color:{o.color ??
                      '#1a1a1a'}"
                  >
                    {o.text}
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
                    style="font-size:{Math.max(ptToPx(o.size ?? 12), 9)}px"
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
          <label class="ed-check">
            <input type="checkbox" bind:checked={flatten} />
            <span>Aplanar anotaciones</span>
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
    border: 1px dashed transparent;
    background: transparent;
    cursor: text;
    z-index: 3;
    box-sizing: border-box;
  }

  .ed-run:hover,
  .ed-run:focus-visible {
    border-color: var(--color-accent-ink, #1a1a1a);
    background: color-mix(in srgb, var(--color-accent, #f5d76e) 30%, transparent);
    outline: none;
  }

  .ed-run.is-locked:hover {
    border-style: dotted;
    background: color-mix(in srgb, #e11d48 18%, transparent);
  }

  .ed-run.is-replaced {
    border-color: #16a34a;
    background: color-mix(in srgb, #16a34a 16%, transparent);
  }

  .ed-runedit {
    position: absolute;
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    transform: translateY(-2px);
    line-height: normal;
  }

  .ed-runinput {
    border: 2px solid var(--color-ink, #1a1a1a);
    background: #fff;
    padding: 0.1rem 0.3rem;
    min-width: 8ch;
    font-family: inherit;
    box-shadow: var(--shadow-stamp);
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
    outline: 2px solid var(--color-ink, #1a1a1a);
    outline-offset: 1px;
    z-index: 10;
  }

  .ed-kind-rect,
  .ed-kind-ellipse,
  .ed-kind-whiteout {
    border-style: solid;
  }

  .ed-kind-ellipse {
    border-radius: 50%;
  }

  .ed-kind-whiteout {
    border-color: color-mix(in srgb, #e11d48 45%, transparent);
    border-style: dashed;
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
    border: 2px solid var(--color-ink, #1a1a1a);
    background: #fff;
    padding: 0.1rem 0.2rem;
    font-family: inherit;
    line-height: 1.25;
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

  .ed-check {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: var(--text-xs, 0.75rem);
    white-space: nowrap;
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
