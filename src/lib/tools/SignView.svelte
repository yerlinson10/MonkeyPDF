<script lang="ts">
  import { onMount } from 'svelte'
  import FileDropZone from '../components/FileDropZone.svelte'
  import OutputPicker from '../components/OutputPicker.svelte'
  import ResultBanner from '../components/ResultBanner.svelte'
  import SignatureModal from '../components/SignatureModal.svelte'
  import PdfPageView from '../components/PdfPageView.svelte'
  import DatePicker from '../components/DatePicker.svelte'
  import Icon from '../components/Icon.svelte'
  import {
    getFormFields,
    getPageMediabox,
    getPdfPageCount,
    normRectToPdf,
    previewPdf,
    signPdf,
    type FormField,
    type OpResult,
    type PreviewTextSpan,
    type SignatureAsset,
    type SignatureKind,
  } from '../api'
  import {
    assetById,
    getSignatureAssets,
    refreshSignatures,
    removeSignatureAsset,
  } from '../signatureStore.svelte'
  import { previewRenderWidth } from '../previewScale'
  import { attachMiddlePan } from '../panScroll'
  import {
    formatDisplayDate,
    isoFromLoose,
    looksLikeDateField,
    toIsoDay,
  } from '../pdfDate'

  interface Placement {
    id: string
    assetId?: string
    inlinePng?: string
    label?: string
    /** Inline stamp kinds that can be edited on the page. */
    textKind?: 'name' | 'date' | 'text'
    dateFormat?: string
    /** ISO day for date stamps. */
    dateIso?: string
    /** AcroForm field this placement filled via «Firmar aquí». */
    formFieldName?: string
    page: number
    nx: number
    ny: number
    nw: number
    nh: number
  }

  type PagePreview = { dataUrl: string; textSpans: PreviewTextSpan[] }

  const ZOOM_MIN = 0.5
  const ZOOM_MAX = 3

  const DATE_FORMATS: { id: string; label: string }[] = [
    { id: 'es-short', label: 'D/M/AAAA' },
    { id: 'es-long', label: 'Largo' },
    { id: 'iso', label: 'ISO' },
    { id: 'us', label: 'US' },
    { id: 'dot', label: 'DD.MM' },
  ]

  let paths = $state<string[]>([])
  let output = $state('')
  let pageCount = $state(1)
  let pagePreviews = $state<Record<number, PagePreview>>({})
  let placements = $state<Placement[]>([])
  let formFields = $state<FormField[]>([])
  let formValues = $state<Record<string, string>>({})
  /** Signature fields already filled — hide yellow «Firmar aquí» chrome. */
  let signedFields = $state<Record<string, true>>({})
  let selectedId = $state<string | null>(null)
  let editingId = $state<string | null>(null)
  let editDraft = $state('')
  let editInputEl = $state<HTMLInputElement | null>(null)
  let loading = $state(false)
  let previewLoading = $state(false)
  let error = $state<string | null>(null)
  let result = $state<OpResult | null>(null)
  let modalOpen = $state(false)
  let modalAsset = $state<SignatureAsset | null>(null)
  let modalKind = $state<SignatureKind>('signature')
  let profileName = $state('')
  let activePage = $state(1)
  let zoom = $state(1)
  let renderWidth = $state(previewRenderWidth(1))
  let zoomTimer: ReturnType<typeof setTimeout> | null = null
  let previewScrollEl = $state<HTMLElement | null>(null)

  let dragKind = $state<'move' | 'resize' | null>(null)
  let resizeHandle = $state<string | null>(null)
  let dragPid = $state<string | null>(null)
  let dragStart = $state({ x: 0, y: 0, nx: 0, ny: 0, nw: 0, nh: 0 })

  // Virtual drag from sidebar (threshold so Edit/+ clicks still work).
  let sidebarPending = $state<{
    payload: string
    startX: number
    startY: number
  } | null>(null)
  let sidebarDrag = $state<{ payload: string; x: number; y: number } | null>(null)
  let sidebarDidDrag = $state(false)
  const SIDEBAR_DRAG_THRESHOLD = 6

  const assets = $derived(getSignatureAssets())
  const path = $derived(paths[0] ?? '')

  onMount(() => {
    void refreshSignatures().catch(() => {})
  })

  $effect(() => {
    const el = previewScrollEl
    if (!el) return
    return attachMiddlePan(el)
  })

  $effect(() => {
    if (path) {
      void loadDocument(path)
    } else {
      pageCount = 1
      pagePreviews = {}
      placements = []
      formFields = []
      formValues = {}
      signedFields = {}
      selectedId = null
      editingId = null
      zoom = 1
      renderWidth = previewRenderWidth(1)
    }
  })

  async function loadPagePreviews(p: string) {
    const previews: Record<number, PagePreview> = {}
    const max = Math.min(pageCount, 40)
    for (let i = 1; i <= max; i++) {
      const prev = await previewPdf(p, i, renderWidth)
      previews[i] = { dataUrl: prev.dataUrl, textSpans: prev.textSpans ?? [] }
    }
    pagePreviews = previews
  }

  async function loadDocument(p: string) {
    previewLoading = true
    error = null
    zoom = 1
    renderWidth = previewRenderWidth(1)
    try {
      pageCount = await getPdfPageCount(p)
      await loadPagePreviews(p)
      const fields = await getFormFields(p)
      formFields = fields
      signedFields = {}
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

  function setZoom(next: number) {
    zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(next * 100) / 100))
    if (zoomTimer) clearTimeout(zoomTimer)
    zoomTimer = setTimeout(() => {
      const w = previewRenderWidth(zoom)
      if (w === renderWidth || !path) return
      renderWidth = w
      previewLoading = true
      void loadPagePreviews(path).finally(() => {
        previewLoading = false
      })
    }, 160)
  }

  function onPreviewWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey)) return
    e.preventDefault()
    setZoom(zoom + (e.deltaY > 0 ? -0.1 : 0.1))
  }

  function pageBoxRect(host: HTMLElement): DOMRect {
    const page = host.querySelector('.pdf-page') as HTMLElement | null
    return (page ?? host).getBoundingClientRect()
  }

  function openCreate(kind: SignatureKind) {
    modalKind = kind
    modalAsset = null
    modalOpen = true
  }

  function openEdit(asset: SignatureAsset) {
    modalAsset = asset
    modalKind = asset.kind
    if (asset.name) profileName = asset.name
    modalOpen = true
  }

  function onModalApply(asset: SignatureAsset) {
    if (asset.name) profileName = asset.name
    placements = [...placements]
  }

  function uid() {
    return `p_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`
  }

  function defaultSize(kind: SignatureKind | 'text'): { nw: number; nh: number } {
    if (kind === 'initials') return { nw: 0.12, nh: 0.06 }
    if (kind === 'logo') return { nw: 0.18, nh: 0.1 }
    if (kind === 'text') return { nw: 0.22, nh: 0.05 }
    return { nw: 0.28, nh: 0.1 }
  }

  function placeAsset(asset: SignatureAsset, page = activePage, nx = 0.35, ny = 0.75) {
    const size = defaultSize(asset.kind)
    placements = [
      ...placements,
      { id: uid(), assetId: asset.id, page, nx, ny, ...size },
    ]
    selectedId = placements[placements.length - 1].id
  }

  async function renderTextPng(text: string, color = '#1a1a1a'): Promise<string> {
    await document.fonts.ready
    const c = document.createElement('canvas')
    c.width = 640
    c.height = 160
    const ctx = c.getContext('2d')!
    ctx.clearRect(0, 0, c.width, c.height)
    ctx.fillStyle = color
    ctx.font = '700 42px Syne, sans-serif'
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    const t = text.trim() || ' '
    ctx.fillText(t, c.width / 2, c.height / 2)
    return c.toDataURL('image/png')
  }

  async function placeText(
    kind: 'name' | 'date' | 'text',
    page = activePage,
    nx = 0.35,
    ny = 0.7,
  ) {
    const dateFormat = 'es-short'
    const dateIso = toIsoDay(new Date())
    let label = ''
    if (kind === 'name') {
      label = profileName || assets.find((a) => a.name)?.name || 'Nombre'
    } else if (kind === 'date') {
      label = formatDisplayDate(dateIso, dateFormat)
    } else {
      label = 'Texto'
    }
    const png = await renderTextPng(label)
    const size = defaultSize('text')
    const id = uid()
    placements = [
      ...placements,
      {
        id,
        inlinePng: png,
        label,
        textKind: kind,
        dateFormat: kind === 'date' ? dateFormat : undefined,
        dateIso: kind === 'date' ? dateIso : undefined,
        page,
        nx,
        ny,
        ...size,
      },
    ]
    selectedId = id
    if (kind === 'text') {
      startInlineEdit(placements[placements.length - 1])
    }
  }

  function startInlineEdit(p: Placement) {
    if (!p.textKind || p.textKind === 'date') return
    editingId = p.id
    editDraft = p.label === 'Texto' ? '' : (p.label ?? '')
    selectedId = p.id
    requestAnimationFrame(() => {
      editInputEl?.focus()
      editInputEl?.select()
    })
  }

  async function commitInlineEdit() {
    const id = editingId
    if (!id) return
    const draft = editDraft
    editingId = null
    const p = placements.find((x) => x.id === id)
    if (!p || !p.textKind || p.textKind === 'date') return
    const label = draft.trim() || (p.textKind === 'name' ? 'Nombre' : 'Texto')
    const png = await renderTextPng(label)
    placements = placements.map((x) =>
      x.id === id ? { ...x, label, inlinePng: png } : x,
    )
  }

  function cancelInlineEdit() {
    editingId = null
    editDraft = ''
  }

  async function applyDateFormat(p: Placement, formatId: string) {
    const iso = p.dateIso || toIsoDay(new Date())
    const label = formatDisplayDate(iso, formatId)
    const png = await renderTextPng(label)
    placements = placements.map((x) =>
      x.id === p.id
        ? { ...x, dateFormat: formatId, dateIso: iso, label, inlinePng: png }
        : x,
    )
  }

  async function applyDateIso(p: Placement, iso: string) {
    if (!iso) return
    const formatId = p.dateFormat ?? 'es-short'
    const label = formatDisplayDate(iso, formatId)
    const png = await renderTextPng(label)
    placements = placements.map((x) =>
      x.id === p.id ? { ...x, dateIso: iso, label, inlinePng: png } : x,
    )
  }

  function setFormDate(name: string, iso: string) {
    formValues = {
      ...formValues,
      [name]: iso ? formatDisplayDate(iso, 'es-short') : '',
    }
  }

  function removePlacement(id: string) {
    const gone = placements.find((p) => p.id === id)
    placements = placements.filter((p) => p.id !== id)
    if (selectedId === id) selectedId = null
    if (editingId === id) editingId = null
    if (gone?.formFieldName) {
      const { [gone.formFieldName]: _, ...rest } = signedFields
      signedFields = rest
    }
  }

  async function deleteAsset(asset: SignatureAsset) {
    await removeSignatureAsset(asset.id)
    const removed = placements.filter((p) => p.assetId === asset.id)
    placements = placements.filter((p) => p.assetId !== asset.id)
    let nextSigned = { ...signedFields }
    for (const p of removed) {
      if (p.formFieldName) delete nextSigned[p.formFieldName]
    }
    signedFields = nextSigned
    if (selectedId && removed.some((p) => p.id === selectedId)) selectedId = null
  }

  function pngFor(p: Placement): string | null {
    if (p.inlinePng) return p.inlinePng
    if (p.assetId) return assetById(p.assetId)?.pngDataUrl ?? null
    return null
  }

  function fieldToNorm(f: FormField, box: { x: number; y: number; width: number; height: number }) {
    const nx = (f.x - box.x) / box.width
    const nh = f.h / box.height
    const nw = f.w / box.width
    const ny = 1 - (f.y - box.y) / box.height - nh
    return { nx, ny, nw, nh }
  }

  let fieldNormCache = $state<Record<string, { nx: number; ny: number; nw: number; nh: number }>>({})

  $effect(() => {
    if (!path || formFields.length === 0) {
      fieldNormCache = {}
      return
    }
    void (async () => {
      const cache: Record<string, { nx: number; ny: number; nw: number; nh: number }> = {}
      const pages = [...new Set(formFields.map((f) => f.page))]
      const boxes: Record<number, Awaited<ReturnType<typeof getPageMediabox>>> = {}
      for (const pg of pages) {
        boxes[pg] = await getPageMediabox(path, pg)
      }
      for (const f of formFields) {
        cache[f.name] = fieldToNorm(f, boxes[f.page])
      }
      fieldNormCache = cache
    })()
  })

  async function autoSignField(f: FormField) {
    const sig =
      assets.find((a) => a.kind === 'signature') ||
      assets.find((a) => a.kind === 'initials')
    if (!sig) {
      openCreate('signature')
      error = 'Crea una firma primero, luego pulsa «Firmar aquí» de nuevo'
      return
    }
    const norm = fieldNormCache[f.name]
    if (!norm) return
    placements = placements.filter((p) => p.formFieldName !== f.name)
    placements = [
      ...placements,
      {
        id: uid(),
        assetId: sig.id,
        formFieldName: f.name,
        page: f.page,
        nx: norm.nx,
        ny: norm.ny,
        nw: Math.max(norm.nw, 0.01),
        nh: Math.max(norm.nh, 0.01),
      },
    ]
    signedFields = { ...signedFields, [f.name]: true }
    selectedId = placements[placements.length - 1].id
  }

  function startMove(e: PointerEvent, p: Placement) {
    if (e.button !== 0) return
    if (editingId === p.id) return
    const t = e.target as HTMLElement
    if (t.closest('.pl-toolbar, .pl-del, .handle, .pl-edit, .pl-date-fmt, .pl-date-tools, .form-date, .mp-date')) return
    e.stopPropagation()
    e.preventDefault()
    selectedId = p.id
    dragKind = 'move'
    dragPid = p.id
    dragStart = { x: e.clientX, y: e.clientY, nx: p.nx, ny: p.ny, nw: p.nw, nh: p.nh }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }

  function startResize(e: PointerEvent, p: Placement, handle: string) {
    if (e.button !== 0) return
    e.stopPropagation()
    e.preventDefault()
    selectedId = p.id
    dragKind = 'resize'
    resizeHandle = handle
    dragPid = p.id
    dragStart = { x: e.clientX, y: e.clientY, nx: p.nx, ny: p.ny, nw: p.nw, nh: p.nh }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }

  function onPlacementMove(e: PointerEvent, host: HTMLElement) {
    if (!dragKind || !dragPid) return
    const rect = pageBoxRect(host)
    if (rect.width <= 0 || rect.height <= 0) return
    const dx = (e.clientX - dragStart.x) / rect.width
    const dy = (e.clientY - dragStart.y) / rect.height
    placements = placements.map((p) => {
      if (p.id !== dragPid) return p
      if (dragKind === 'move') {
        return {
          ...p,
          nx: clamp(dragStart.nx + dx, 0, 1 - p.nw),
          ny: clamp(dragStart.ny + dy, 0, 1 - p.nh),
        }
      }
      let { nx, ny, nw, nh } = {
        nx: dragStart.nx, ny: dragStart.ny, nw: dragStart.nw, nh: dragStart.nh,
      }
      const h = resizeHandle || ''
      if (h.includes('e')) nw = clamp(dragStart.nw + dx, 0.04, 1 - nx)
      if (h.includes('s')) nh = clamp(dragStart.nh + dy, 0.03, 1 - ny)
      if (h.includes('w')) {
        const nx2 = clamp(dragStart.nx + dx, 0, dragStart.nx + dragStart.nw - 0.04)
        nw = dragStart.nw - (nx2 - dragStart.nx)
        nx = nx2
      }
      if (h.includes('n')) {
        const ny2 = clamp(dragStart.ny + dy, 0, dragStart.ny + dragStart.nh - 0.03)
        nh = dragStart.nh - (ny2 - dragStart.ny)
        ny = ny2
      }
      return { ...p, nx, ny, nw, nh }
    })
  }

  function endDrag() {
    dragKind = null
    dragPid = null
    resizeHandle = null
  }

  function clamp(n: number, a: number, b: number) {
    return Math.min(b, Math.max(a, n))
  }

  function onSidebarPointerDown(e: PointerEvent, payload: string) {
    // Nested edit/+ buttons — not the chip itself when it is a <button>
    if ((e.target as HTMLElement).closest('.asset-actions')) return
    if (e.button !== 0) return
    sidebarPending = { payload, startX: e.clientX, startY: e.clientY }
    sidebarDrag = null
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }

  function onSidebarPointerMove(e: PointerEvent) {
    if (sidebarDrag) {
      sidebarDrag = { ...sidebarDrag, x: e.clientX, y: e.clientY }
      return
    }
    if (!sidebarPending) return
    const dx = e.clientX - sidebarPending.startX
    const dy = e.clientY - sidebarPending.startY
    if (Math.hypot(dx, dy) < SIDEBAR_DRAG_THRESHOLD) return
    sidebarDidDrag = true
    sidebarDrag = {
      payload: sidebarPending.payload,
      x: e.clientX,
      y: e.clientY,
    }
    sidebarPending = null
  }

  function placeFromSidebar(payload: string, x: number, y: number) {
    const el = document.elementFromPoint(x, y) as HTMLElement | null
    const host = el?.closest('[data-page]') as HTMLElement | null
    if (!host) return
    const page = Number(host.dataset.page)
    if (!page) return
    const rect = pageBoxRect(host)
    const nx = clamp((x - rect.left) / rect.width - 0.1, 0, 0.8)
    const ny = clamp((y - rect.top) / rect.height - 0.05, 0, 0.9)
    if (payload.startsWith('asset:')) {
      const id = payload.slice(6)
      const asset = assetById(id)
      if (asset) placeAsset(asset, page, nx, ny)
    } else if (payload === 'name' || payload === 'date' || payload === 'text') {
      void placeText(payload, page, nx, ny)
    }
  }

  function onSidebarPointerUp(e: PointerEvent) {
    if (sidebarDrag) {
      const { x, y, payload } = sidebarDrag
      sidebarDrag = null
      sidebarPending = null
      placeFromSidebar(payload, x, y)
      // Swallow the synthetic click that follows a drag
      setTimeout(() => {
        sidebarDidDrag = false
      }, 0)
      return
    }
    sidebarPending = null
    sidebarDidDrag = false
    void e
  }

  function onSidebarPointerCancel() {
    sidebarDrag = null
    sidebarPending = null
    sidebarDidDrag = false
  }

  function stopSidebarDrag(e: Event) {
    e.stopPropagation()
  }

  function onOptChipClick(kind: 'name' | 'date' | 'text') {
    if (sidebarDidDrag) {
      sidebarDidDrag = false
      return
    }
    void placeText(kind)
  }

  function onPageDragOver(e: DragEvent) {
    e.preventDefault()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
  }

  function onPageDrop(e: DragEvent, page: number, host: HTMLElement) {
    e.preventDefault()
    const data = e.dataTransfer?.getData('text/plain')
    if (!data) return
    const rect = pageBoxRect(host)
    const nx = clamp((e.clientX - rect.left) / rect.width - 0.1, 0, 0.8)
    const ny = clamp((e.clientY - rect.top) / rect.height - 0.05, 0, 0.9)
    if (data.startsWith('asset:')) {
      const id = data.slice(6)
      const asset = assetById(id)
      if (asset) placeAsset(asset, page, nx, ny)
    } else if (data === 'name' || data === 'date' || data === 'text') {
      void placeText(data, page, nx, ny)
    }
  }

  async function run() {
    error = null
    result = null
    if (!path) { error = 'Selecciona un PDF'; return }
    if (!output) { error = 'Selecciona un PDF de salida'; return }
    if (placements.length === 0 && Object.keys(formValues).length === 0) {
      error = 'Coloca una firma o rellena un campo'
      return
    }
    loading = true
    try {
      const pdfPlacements = []
      for (const p of placements) {
        const box = await getPageMediabox(path, p.page)
        const pts = normRectToPdf(p.nx, p.ny, p.nw, p.nh, box)
        pdfPlacements.push({
          assetId: p.assetId ?? null,
          pngDataUrl: p.inlinePng ?? null,
          page: p.page,
          ...pts,
        })
      }
      const fills = Object.entries(formValues)
        .filter(([, v]) => v && v !== 'Off')
        .map(([name, value]) => ({ name, value }))
      result = await signPdf(path, output, pdfPlacements, fills)
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  const pages = $derived(Array.from({ length: Math.min(pageCount, 40) }, (_, i) => i + 1))
  const sigAssets = $derived(assets.filter((a) => a.kind === 'signature'))
  const initAssets = $derived(assets.filter((a) => a.kind === 'initials'))
  const logoAssets = $derived(assets.filter((a) => a.kind === 'logo'))
</script>

<div class="sign-layout">
  <ResultBanner {loading} {error} {result} toolLabel="Firmar" />

  <div class="sign-main">
    <div class="sign-preview" bind:this={previewScrollEl}>
      <FileDropZone
        bind:paths
        accept=".pdf"
        multiple={false}
        label="Arrastra el PDF a firmar"
        showPreview={false}
      />
      {#if path && pages.length > 0}
        <div class="preview-toolbar">
          <div class="mp-preview-pager">
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              disabled={zoom <= ZOOM_MIN}
              onclick={() => setZoom(zoom - 0.25)}
              aria-label="Alejar"
            >
              −
            </button>
            <span class="mono text-[var(--text-xs)] w-10 text-center"
              >{Math.round(zoom * 100)}%</span
            >
            <button
              type="button"
              class="mp-btn mp-btn-ghost !min-h-8 !px-2"
              disabled={zoom >= ZOOM_MAX}
              onclick={() => setZoom(zoom + 0.25)}
              aria-label="Acercar"
            >
              +
            </button>
          </div>
          {#if previewLoading}
            <span class="hint">Actualizando vista…</span>
          {/if}
        </div>
        <p class="hint preview-hint">
          Selecciona texto · clic derecho = copiar / zoom · rueda pulsada = mover · Ctrl + rueda = zoom
        </p>
      {/if}
      {#each pages as pg (pg)}
        {@const prev = pagePreviews[pg]}
        {#if prev}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="page-shell"
            data-page={pg}
            ondragover={onPageDragOver}
            ondrop={(e) => onPageDrop(e, pg, e.currentTarget as HTMLElement)}
            onpointermove={(e) => onPlacementMove(e, e.currentTarget as HTMLElement)}
            onpointerup={endDrag}
            onpointercancel={endDrag}
            onpointerdown={(e) => {
              if (e.button === 0) activePage = pg
            }}
          >
            <PdfPageView
              src={prev.dataUrl}
              alt="Página {pg}"
              bare={true}
              widthPercent={zoom * 100}
              textSpans={prev.textSpans}
              showTextPanel={false}
              onCtrlWheel={onPreviewWheel}
              onZoomIn={() => setZoom(zoom + 0.25)}
              onZoomOut={() => setZoom(zoom - 0.25)}
              onZoomReset={() => setZoom(1)}
            >
              {#snippet children()}
                {#each formFields.filter((f) => f.page === pg && !signedFields[f.name]) as f (f.name + pg)}
                  {@const n = fieldNormCache[f.name]}
                  {#if n}
                    <div
                      class="form-field"
                      class:is-sig={f.kind === 'signature'}
                      style="left:{n.nx * 100}%;top:{n.ny * 100}%;width:{n.nw * 100}%;height:{n.nh * 100}%;"
                    >
                      {#if f.kind === 'signature'}
                        <button type="button" class="sig-here" onclick={() => autoSignField(f)}>
                          Firmar aquí
                        </button>
                      {:else if f.kind === 'checkbox'}
                        <input
                          type="checkbox"
                          checked={formValues[f.name] === 'Yes' || formValues[f.name] === 'On'}
                          onchange={(e) => {
                            formValues = {
                              ...formValues,
                              [f.name]: e.currentTarget.checked ? 'Yes' : 'Off',
                            }
                          }}
                        />
                      {:else if f.kind === 'choice' && f.options.length}
                        <select
                          value={formValues[f.name] ?? ''}
                          onchange={(e) => {
                            formValues = { ...formValues, [f.name]: e.currentTarget.value }
                          }}
                        >
                          <option value="">—</option>
                          {#each f.options as opt}
                            <option value={opt}>{opt}</option>
                          {/each}
                        </select>
                      {:else if f.kind !== 'unknown' && looksLikeDateField(f.name)}
                        <!-- svelte-ignore a11y_no_static_element_interactions -->
                        <div class="form-date" onpointerdown={(e) => e.stopPropagation()}>
                          <DatePicker
                            size="compact"
                            value={isoFromLoose(formValues[f.name] ?? f.value ?? '')}
                            placeholder={f.name}
                            onchange={(iso) => setFormDate(f.name, iso)}
                          />
                        </div>
                      {:else if f.kind !== 'unknown'}
                        <input
                          type="text"
                          value={formValues[f.name] ?? ''}
                          oninput={(e) => {
                            formValues = { ...formValues, [f.name]: e.currentTarget.value }
                          }}
                          placeholder={f.name}
                        />
                      {/if}
                    </div>
                  {/if}
                {/each}

                {#each placements.filter((p) => p.page === pg) as p (p.id)}
                  {@const png = pngFor(p)}
                  {#if png || editingId === p.id}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div
                      class="placement"
                      class:is-selected={selectedId === p.id}
                      class:is-field-fit={!!p.formFieldName}
                      class:is-editing={editingId === p.id}
                      data-placement={p.id}
                      style="left:{p.nx * 100}%;top:{p.ny * 100}%;width:{p.nw * 100}%;height:{p.nh * 100}%;"
                      onpointerdown={(e) => startMove(e, p)}
                      onclick={() => (selectedId = p.id)}
                      ondblclick={() => startInlineEdit(p)}
                    >
                      {#if editingId === p.id}
                        <input
                          class="pl-edit"
                          bind:this={editInputEl}
                          bind:value={editDraft}
                          aria-label="Editar texto"
                          placeholder={p.textKind === 'name' ? 'Nombre' : 'Escribe aquí…'}
                          onpointerdown={(e) => e.stopPropagation()}
                          onkeydown={(e) => {
                            if (e.key === 'Enter') {
                              e.preventDefault()
                              void commitInlineEdit()
                            }
                            if (e.key === 'Escape') {
                              e.preventDefault()
                              cancelInlineEdit()
                            }
                          }}
                          onblur={() => void commitInlineEdit()}
                        />
                      {:else if png}
                        <img src={png} alt={p.label || 'Sello'} draggable="false" />
                      {/if}
                      {#if selectedId === p.id && editingId !== p.id}
                        <div class="pl-toolbar">
                          {#if p.textKind === 'date'}
                            <!-- svelte-ignore a11y_no_static_element_interactions -->
                            <div
                              class="pl-date-tools"
                              onpointerdown={(e) => e.stopPropagation()}
                            >
                              <div class="pl-date-pick">
                                <DatePicker
                                  size="compact"
                                  value={p.dateIso || toIsoDay(new Date())}
                                  displayFormat={p.dateFormat ?? 'es-short'}
                                  clearable={false}
                                  onchange={(iso) => void applyDateIso(p, iso)}
                                />
                              </div>
                              <div class="pl-date-fmts">
                                {#each DATE_FORMATS as fmt}
                                  <button
                                    type="button"
                                    class="pl-fmt"
                                    class:is-on={(p.dateFormat ?? 'es-short') === fmt.id}
                                    title={fmt.label}
                                    onclick={() => void applyDateFormat(p, fmt.id)}
                                  >
                                    {fmt.label}
                                  </button>
                                {/each}
                              </div>
                            </div>
                          {/if}
                          {#if p.textKind === 'text' || p.textKind === 'name'}
                            <button
                              type="button"
                              class="pl-tool"
                              onpointerdown={(e) => e.stopPropagation()}
                              onclick={(e) => {
                                e.stopPropagation()
                                startInlineEdit(p)
                              }}
                              title="Editar texto"
                            >
                              Editar
                            </button>
                          {/if}
                          <button
                            type="button"
                            class="pl-del"
                            onpointerdown={(e) => e.stopPropagation()}
                            onclick={(e) => {
                              e.stopPropagation()
                              removePlacement(p.id)
                            }}
                            aria-label="Eliminar"
                            title="Eliminar"
                          >
                            <Icon name="x" size={12} />
                          </button>
                        </div>
                        {#each ['nw', 'ne', 'sw', 'se', 'n', 's', 'e', 'w'] as h}
                          <span
                            class="handle handle-{h}"
                            onpointerdown={(e) => startResize(e, p, h)}
                          ></span>
                        {/each}
                      {/if}
                    </div>
                  {/if}
                {/each}
                <span class="page-tag">Pág. {pg}</span>
              {/snippet}
            </PdfPageView>
          </div>
        {/if}
      {/each}
    </div>

    <aside class="sign-side">
      <div class="side-block">
        <div class="side-head">
          <strong>Tus firmas</strong>
          <button type="button" class="mp-btn mp-btn-ghost" onclick={() => openCreate('signature')}>
            + Nueva
          </button>
        </div>
        {#each sigAssets as a (a.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="asset-card"
            onpointerdown={(e) => onSidebarPointerDown(e, `asset:${a.id}`)}
            onpointermove={onSidebarPointerMove}
            onpointerup={onSidebarPointerUp}
            onpointercancel={onSidebarPointerCancel}
          >
            <img src={a.pngDataUrl} alt="Firma" />
            <div class="asset-actions">
              <button
                type="button"
                class="mp-btn mp-btn-ghost"
                onpointerdown={stopSidebarDrag}
                onclick={() => openEdit(a)}
                title="Editar"
              >Editar</button>
              <button
                type="button"
                class="mp-btn mp-btn-ghost"
                onpointerdown={stopSidebarDrag}
                onclick={() => placeAsset(a)}
                title="Colocar en página"
              >+</button>
            </div>
          </div>
        {:else}
          <p class="hint">Crea tu firma (escribir, dibujar o subir imagen).</p>
        {/each}
      </div>

      <div class="side-block">
        <div class="side-head">
          <strong>Iniciales</strong>
          <button type="button" class="mp-btn mp-btn-ghost" onclick={() => openCreate('initials')}>
            + Nueva
          </button>
        </div>
        {#each initAssets as a (a.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="asset-card"
            onpointerdown={(e) => onSidebarPointerDown(e, `asset:${a.id}`)}
            onpointermove={onSidebarPointerMove}
            onpointerup={onSidebarPointerUp}
            onpointercancel={onSidebarPointerCancel}
          >
            <img src={a.pngDataUrl} alt="Iniciales" />
            <div class="asset-actions">
              <button
                type="button"
                class="mp-btn mp-btn-ghost"
                onpointerdown={stopSidebarDrag}
                onclick={() => openEdit(a)}
                title="Editar"
              >Editar</button>
              <button
                type="button"
                class="mp-btn mp-btn-ghost"
                onpointerdown={stopSidebarDrag}
                onclick={() => placeAsset(a)}
                title="Colocar en página"
              >+</button>
            </div>
          </div>
        {:else}
          <p class="hint">Sin iniciales aún.</p>
        {/each}
      </div>

      <div class="side-block">
        <div class="side-head">
          <strong>Logo</strong>
          <button type="button" class="mp-btn mp-btn-ghost" onclick={() => openCreate('logo')}>
            + Nuevo
          </button>
        </div>
        {#each logoAssets as a (a.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="asset-card"
            onpointerdown={(e) => onSidebarPointerDown(e, `asset:${a.id}`)}
            onpointermove={onSidebarPointerMove}
            onpointerup={onSidebarPointerUp}
            onpointercancel={onSidebarPointerCancel}
          >
            <img src={a.pngDataUrl} alt="Logo" />
            <div class="asset-actions">
              <button
                type="button"
                class="mp-btn mp-btn-ghost"
                onpointerdown={stopSidebarDrag}
                onclick={() => openEdit(a)}
                title="Editar"
              >Editar</button>
              <button
                type="button"
                class="mp-btn mp-btn-ghost"
                onpointerdown={stopSidebarDrag}
                onclick={() => placeAsset(a)}
                title="Colocar en página"
              >+</button>
            </div>
          </div>
        {:else}
          <p class="hint">Sin logo aún.</p>
        {/each}
      </div>

      <div class="side-block">
        <strong>Campos opcionales</strong>
        <div class="opt-row">
          {#each [['name', 'Nombre'], ['date', 'Fecha'], ['text', 'Texto']] as [k, label]}
            <button
              type="button"
              class="opt-chip"
              onpointerdown={(e) => onSidebarPointerDown(e, k)}
              onpointermove={onSidebarPointerMove}
              onpointerup={onSidebarPointerUp}
              onpointercancel={onSidebarPointerCancel}
              onclick={() => onOptChipClick(k as 'name' | 'date' | 'text')}
            >
              {label}
            </button>
          {/each}
        </div>
        <p class="hint">Texto: doble clic en la página para editar. Fecha: calendario + formato en la barra.</p>
      </div>

      {#if formFields.length > 0}
        <p class="hint">{formFields.length} campo(s) de formulario detectados. Rellenalos directamente o pulsa «Firmar aquí».</p>
      {:else}
        <p class="hint">Sin campos de formulario. Arrastra tu firma donde quieras.</p>
      {/if}

      <OutputPicker bind:value={output} tool="sign" defaultName="firmado.pdf" label="PDF de salida" />
      <button type="button" class="mp-btn mp-btn-primary w-full" disabled={loading} onclick={run}>
        Firmar
      </button>
    </aside>
  </div>
</div>

{#if modalOpen}
  <SignatureModal
    open={modalOpen}
    asset={modalAsset}
    initialKind={modalKind}
    fullName={profileName}
    onClose={() => (modalOpen = false)}
    onApply={onModalApply}
    onDelete={modalAsset ? deleteAsset : undefined}
  />
{/if}

{#if sidebarDrag}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="drag-ghost"
    style="left:{sidebarDrag.x}px;top:{sidebarDrag.y}px;"
    aria-hidden="true"
  >
    {sidebarDrag.payload.startsWith('asset:')
      ? '✎ Firma'
      : sidebarDrag.payload === 'name'
        ? 'Nombre'
        : sidebarDrag.payload === 'date'
          ? 'Fecha'
          : 'Texto'}
  </div>
{/if}

<style>
  .sign-layout { display: flex; flex-direction: column; gap: 1rem; }
  .sign-main {
    display: grid;
    grid-template-columns: 1fr min(300px, 34%);
    gap: 1rem;
    align-items: start;
  }
  @media (max-width: 900px) { .sign-main { grid-template-columns: 1fr; } }

  .sign-preview {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    max-height: 78vh;
    overflow: auto;
    padding-right: 0.25rem;
  }

  .preview-toolbar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    position: sticky;
    top: 0;
    z-index: 5;
    background: color-mix(in srgb, var(--color-paper, #fff) 92%, transparent);
    padding: 0.25rem 0;
  }
  .preview-hint { margin: 0; }

  .page-shell {
    width: 100%;
    border: 2px solid var(--color-ink);
    box-shadow: 4px 4px 0 var(--color-ink);
    background: var(--color-paper-2, #f3efe6);
    padding: 0.75rem;
    /* Single scroller: .sign-preview — nested overflow broke middle-mouse pan */
    overflow: visible;
  }
  .page-tag {
    position: absolute;
    top: 8px;
    left: 8px;
    font-size: 12px;
    line-height: 1.25;
    background: var(--color-banana, #f5d547);
    border: 1.5px solid var(--color-ink);
    padding: 3px 8px;
    font-weight: 800;
    color: var(--color-ink);
    pointer-events: none;
    z-index: 4;
    white-space: nowrap;
    box-shadow: 2px 2px 0 var(--color-ink);
  }

  .placement {
    position: absolute;
    cursor: move;
    border: 2px solid transparent;
    touch-action: none;
    line-height: normal;
    box-sizing: border-box;
  }
  .placement.is-selected {
    border-color: var(--color-ink);
    box-shadow: 3px 3px 0 var(--color-accent);
  }
  .placement.is-editing {
    cursor: text;
    border-color: var(--color-ink);
    box-shadow: 3px 3px 0 var(--color-accent);
    background: var(--color-paper);
  }
  .placement img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: none;
  }
  .placement.is-field-fit img {
    object-fit: fill;
  }
  .pl-edit {
    display: block;
    width: 100%;
    height: 100%;
    border: 0;
    margin: 0;
    padding: 2px 6px;
    background: transparent;
    font: inherit;
    font-family: var(--font-ui);
    font-size: clamp(11px, 55%, 16px);
    font-weight: 700;
    color: var(--color-ink);
    text-align: center;
    box-sizing: border-box;
  }
  .pl-edit:focus {
    outline: none;
  }
  .pl-toolbar {
    position: absolute;
    top: -34px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 4px;
    z-index: 6;
    pointer-events: auto;
    padding: 2px;
    background: var(--color-paper);
    border: 1.5px solid var(--color-ink);
    box-shadow: 2px 2px 0 var(--color-ink);
  }
  .pl-tool,
  .pl-del {
    min-width: 28px;
    height: 26px;
    border-radius: var(--radius-sm, 2px);
    border: 1.5px solid var(--color-ink);
    background: var(--color-paper);
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 0 6px;
    font: inherit;
    font-size: 11px;
    font-weight: 700;
    color: var(--color-ink);
    box-shadow: 1px 1px 0 var(--color-ink);
  }
  .pl-tool:hover {
    background: var(--color-accent);
  }
  .pl-del:hover {
    background: var(--color-danger-soft);
  }
  .pl-date-tools {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 11rem;
    max-width: 14rem;
  }
  .pl-date-pick {
    height: 28px;
    border: 1.5px solid var(--color-ink);
    background: var(--color-accent);
    box-shadow: 1px 1px 0 var(--color-ink);
  }
  .pl-date-pick :global(.mp-date-trigger) {
    background: transparent;
    min-height: 28px;
    height: 28px;
    font-size: 11px;
    font-weight: 700;
  }
  .pl-date-pick :global(.mp-date-pop) {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 40;
  }
  .pl-date-fmts {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
  }
  .pl-fmt {
    border: 1.5px solid var(--color-ink);
    background: var(--color-paper);
    color: var(--color-ink);
    font: inherit;
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    padding: 2px 5px;
    cursor: pointer;
    box-shadow: 1px 1px 0 var(--color-ink);
  }
  .pl-fmt.is-on,
  .pl-fmt:hover {
    background: var(--color-accent);
  }
  .form-date {
    width: 100%;
    height: 100%;
    min-height: 100%;
  }
  .form-date :global(.mp-date) {
    height: 100%;
  }
  .form-date :global(.mp-date-pop) {
    z-index: 30;
  }
  .handle {
    position: absolute;
    width: 11px;
    height: 11px;
    background: var(--color-accent);
    border: 1.5px solid var(--color-ink);
    box-shadow: 1px 1px 0 var(--color-ink);
    z-index: 5;
    box-sizing: border-box;
    border-radius: 0;
  }
  .handle-nw { top: -6px; left: -6px; cursor: nwse-resize; }
  .handle-ne { top: -6px; right: -6px; cursor: nesw-resize; }
  .handle-sw { bottom: -6px; left: -6px; cursor: nesw-resize; }
  .handle-se { bottom: -6px; right: -6px; cursor: nwse-resize; }
  .handle-n { top: -6px; left: 50%; transform: translateX(-50%); cursor: ns-resize; }
  .handle-s { bottom: -6px; left: 50%; transform: translateX(-50%); cursor: ns-resize; }
  .handle-e { right: -6px; top: 50%; transform: translateY(-50%); cursor: ew-resize; }
  .handle-w { left: -6px; top: 50%; transform: translateY(-50%); cursor: ew-resize; }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  .form-field {
    position: absolute;
    background: color-mix(in srgb, var(--color-accent) 22%, transparent);
    border: 1.5px dashed var(--color-ink);
    display: flex;
    align-items: stretch;
    padding: 2px;
    box-sizing: border-box;
    line-height: normal;
    overflow: visible;
    min-width: 4px;
    min-height: 4px;
  }
  .form-field input,
  .form-field select {
    width: 100%;
    min-height: 100%;
    border: 0;
    background: color-mix(in srgb, var(--color-paper) 92%, transparent);
    font: inherit;
    font-size: clamp(10px, 70%, 13px);
    line-height: 1.2;
    padding: 2px 4px;
    color: var(--color-ink);
    box-sizing: border-box;
  }
  .form-field input:focus,
  .form-field select:focus {
    outline: 2px solid var(--color-focus);
  }
  .form-field.is-sig {
    background: color-mix(in srgb, var(--color-accent) 35%, transparent);
    border-color: var(--color-ink);
    align-items: center;
    justify-content: center;
    overflow: visible;
  }
  .sig-here {
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    line-height: 1.25;
    border: 1.5px solid var(--color-ink);
    background: var(--color-accent);
    color: var(--color-ink);
    padding: 5px 10px;
    cursor: pointer;
    white-space: nowrap;
    box-sizing: border-box;
    box-shadow: 2px 2px 0 var(--color-ink);
    flex-shrink: 0;
  }
  .sig-here:hover {
    translate: 1px 1px;
    box-shadow: 1px 1px 0 var(--color-ink);
  }

  .sign-side {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    border: 2px solid var(--color-ink);
    box-shadow: 4px 4px 0 var(--color-ink);
    background: var(--color-paper, #fff);
    color: var(--color-ink);
    padding: 0.75rem;
    position: sticky;
    top: 0.5rem;
  }

  .side-block { display: flex; flex-direction: column; gap: 0.4rem; }
  .side-head { display: flex; justify-content: space-between; align-items: center; }

  .asset-card {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    border: 1.5px dashed color-mix(in srgb, var(--color-ink) 40%, transparent);
    padding: 0.35rem;
    cursor: grab;
    background: var(--color-paper);
  }
  .asset-card:hover { border-color: var(--color-ink); }
  .asset-card img { height: 36px; max-width: 140px; object-fit: contain; }
  .asset-actions {
    margin-left: auto;
    display: flex;
    gap: 0.2rem;
    flex-shrink: 0;
  }
  .asset-actions .mp-btn {
    font-size: 11px;
    font-weight: 700;
    min-height: 28px;
    padding-inline: 0.45rem;
  }

  .opt-row { display: flex; flex-wrap: wrap; gap: 0.35rem; }
  .opt-chip {
    border: 1px solid var(--color-ink);
    background: #fff;
    font: inherit;
    font-size: 12px;
    padding: 0.3rem 0.5rem;
    cursor: grab;
  }
  .opt-chip:hover { background: var(--color-banana, #f5d547); }

  .hint { font-size: var(--text-xs); color: var(--color-ink-2); margin: 0; }
  .w-full { width: 100%; }

  .drag-ghost {
    position: fixed;
    z-index: 90;
    transform: translate(-50%, -50%);
    background: var(--color-banana, #f5d547);
    border: 2px solid var(--color-ink);
    box-shadow: 3px 3px 0 var(--color-ink);
    padding: 0.35rem 0.6rem;
    font: inherit;
    font-size: 12px;
    font-weight: 700;
    color: var(--color-ink);
    pointer-events: none;
    white-space: nowrap;
  }
</style>
