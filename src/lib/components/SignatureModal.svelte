<script lang="ts">
  import {
    initialsFromName,
    saveSignatureAsset,
  } from '../signatureStore.svelte'
  import type { SignatureAsset, SignatureKind, SignatureMethod } from '../api'
  import Icon from './Icon.svelte'

  interface Props {
    open?: boolean
    asset?: SignatureAsset | null
    initialKind?: SignatureKind
    fullName?: string
    onClose?: () => void
    onApply?: (asset: SignatureAsset) => void
    onDelete?: (asset: SignatureAsset) => void | Promise<void>
  }

  let {
    open = true,
    asset = null,
    initialKind = 'signature',
    fullName: initialName = '',
    onClose,
    onApply,
    onDelete,
  }: Props = $props()

  const isEditing = $derived(!!asset)

  export const SIGN_FONTS = [
    { id: 'Great Vibes', label: 'Great Vibes', family: '"Great Vibes", cursive' },
    { id: 'Dancing Script', label: 'Dancing Script', family: '"Dancing Script", cursive' },
    { id: 'Sacramento', label: 'Sacramento', family: '"Sacramento", cursive' },
    { id: 'Allura', label: 'Allura', family: '"Allura", cursive' },
  ] as const

  export const SIGN_COLORS = [
    { id: '#1a1a1a', label: 'Tinta' },
    { id: '#c0392b', label: 'Rojo' },
    { id: '#1e5bb8', label: 'Azul' },
    { id: '#1e7a3a', label: 'Verde' },
  ] as const

  const seed = asset
  let tab = $state<SignatureKind>(seed?.kind ?? initialKind)
  let fullName = $state(seed?.name ?? initialName)
  let initials = $state(
    (seed?.source?.initials as string) ||
      initialsFromName(seed?.name ?? initialName) ||
      'YL',
  )
  let initialsLocked = $state(false)
  let saving = $state(false)
  let deleting = $state(false)
  let error = $state<string | null>(null)

  // Per-tab state so signature/initials/logo don't bleed into each other.
  interface TabState {
    method: SignatureMethod
    fontId: string
    color: string
    uploadPreview: string | null
  }
  function defaultTabState(kind: SignatureKind): TabState {
    if (kind === 'logo') {
      return { method: 'upload', fontId: 'Great Vibes', color: '#1a1a1a', uploadPreview: seed?.method === 'upload' && seed?.kind === 'logo' ? seed.pngDataUrl : null }
    }
    const fromSeed = seed?.kind === kind
    return {
      method: fromSeed ? seed!.method : 'type',
      fontId: fromSeed ? seed!.font ?? 'Great Vibes' : 'Great Vibes',
      color: fromSeed ? seed!.color ?? '#1a1a1a' : '#1a1a1a',
      uploadPreview: fromSeed && seed!.method === 'upload' ? seed!.pngDataUrl : null,
    }
  }
  let tabState = $state<Record<SignatureKind, TabState>>({
    signature: defaultTabState('signature'),
    initials: defaultTabState('initials'),
    logo: defaultTabState('logo'),
  })

  // Draw canvases — always mounted so the drawing persists across method/tab switches.
  let drawCanvasSig = $state<HTMLCanvasElement | null>(null)
  let drawCanvasInit = $state<HTMLCanvasElement | null>(null)
  let typeCanvas = $state<HTMLCanvasElement | null>(null)
  let drawing = $state(false)
  let lastX = 0
  let lastY = 0

  const current = $derived(tabState[tab])
  const displayText = $derived(tab === 'initials' ? initials : fullName || 'Tu nombre')
  const fontFamily = $derived(
    SIGN_FONTS.find((f) => f.id === current.fontId)?.family ?? '"Great Vibes", cursive',
  )

  $effect(() => {
    if (!initialsLocked) {
      const auto = initialsFromName(fullName)
      if (auto) initials = auto
    }
  })

  $effect(() => {
    if (current.method === 'type' && tab !== 'logo') {
      void document.fonts.ready.then(() => paintTypePreview())
    }
  })

  // Restore a drawn signature onto its canvas when editing.
  $effect(() => {
    if (tab === 'logo') return
    const canvas = tab === 'signature' ? drawCanvasSig : drawCanvasInit
    if (!canvas) return
    if (seed?.method === 'draw' && seed?.kind === tab && seed.pngDataUrl) {
      const img = new Image()
      img.onload = () => {
        const ctx = canvas.getContext('2d')
        if (!ctx) return
        ctx.clearRect(0, 0, canvas.width, canvas.height)
        const scale = Math.min(canvas.width / img.width, canvas.height / img.height)
        const dw = img.width * scale
        const dh = img.height * scale
        ctx.drawImage(img, (canvas.width - dw) / 2, (canvas.height - dh) / 2, dw, dh)
      }
      img.src = seed.pngDataUrl
    }
  })

  function paintTypePreview() {
    const c = typeCanvas
    if (!c) return
    const ctx = c.getContext('2d')
    if (!ctx) return
    ctx.clearRect(0, 0, c.width, c.height)
    ctx.fillStyle = current.color
    const size = tab === 'initials' ? 72 : 48
    ctx.font = `${size}px ${fontFamily}`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillText(displayText, c.width / 2, c.height / 2)
  }

  function clearDraw() {
    const c = tab === 'signature' ? drawCanvasSig : drawCanvasInit
    if (!c) return
    const ctx = c.getContext('2d')
    if (!ctx) return
    ctx.clearRect(0, 0, c.width, c.height)
  }

  function activeDrawCanvas(): HTMLCanvasElement | null {
    return tab === 'signature' ? drawCanvasSig : drawCanvasInit
  }

  function onDrawDown(e: PointerEvent) {
    const c = activeDrawCanvas()
    if (!c) return
    drawing = true
    const rect = c.getBoundingClientRect()
    lastX = ((e.clientX - rect.left) / rect.width) * c.width
    lastY = ((e.clientY - rect.top) / rect.height) * c.height
    c.setPointerCapture(e.pointerId)
  }

  function onDrawMove(e: PointerEvent) {
    if (!drawing) return
    const c = activeDrawCanvas()
    if (!c) return
    const ctx = c.getContext('2d')
    if (!ctx) return
    const rect = c.getBoundingClientRect()
    const x = ((e.clientX - rect.left) / rect.width) * c.width
    const y = ((e.clientY - rect.top) / rect.height) * c.height
    ctx.strokeStyle = current.color
    ctx.lineWidth = 3.2
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'
    ctx.beginPath()
    ctx.moveTo(lastX, lastY)
    ctx.lineTo(x, y)
    ctx.stroke()
    lastX = x
    lastY = y
  }

  function onDrawUp(e: PointerEvent) {
    drawing = false
    try {
      activeDrawCanvas()?.releasePointerCapture(e.pointerId)
    } catch {
      /* ignore */
    }
  }

  async function onUpload(files: FileList | null) {
    const file = files?.[0]
    if (!file) return
    error = null
    try {
      const dataUrl = file.type === 'image/svg+xml'
        ? await svgToPngDataUrl(await file.text())
        : await fileToDataUrl(file)
      tabState = { ...tabState, [tab]: { ...current, uploadPreview: dataUrl } }
    } catch (e) {
      error = String(e)
    }
  }

  function fileToDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const r = new FileReader()
      r.onload = () => resolve(String(r.result))
      r.onerror = () => reject(new Error('No se pudo leer el archivo'))
      r.readAsDataURL(file)
    })
  }

  async function svgToPngDataUrl(svg: string): Promise<string> {
    const blob = new Blob([svg], { type: 'image/svg+xml' })
    const url = URL.createObjectURL(blob)
    try {
      const img = await loadImage(url)
      const c = document.createElement('canvas')
      c.width = 800
      c.height = 280
      const ctx = c.getContext('2d')!
      ctx.clearRect(0, 0, c.width, c.height)
      const scale = Math.min(c.width / img.width, c.height / img.height)
      const dw = img.width * scale
      const dh = img.height * scale
      ctx.drawImage(img, (c.width - dw) / 2, (c.height - dh) / 2, dw, dh)
      return c.toDataURL('image/png')
    } finally {
      URL.revokeObjectURL(url)
    }
  }

  function loadImage(src: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image()
      img.onload = () => resolve(img)
      img.onerror = () => reject(new Error('Imagen inválida'))
      img.src = src
    })
  }

  /** Crop a canvas to its non-transparent bounding box so the signature sits on a baseline. */
  function trimToContent(canvas: HTMLCanvasElement): string | null {
    const ctx = canvas.getContext('2d')
    if (!ctx) return null
    const { width, height } = canvas
    if (width === 0 || height === 0) return null
    const img = ctx.getImageData(0, 0, width, height)
    const data = img.data
    let minX = width
    let minY = height
    let maxX = 0
    let maxY = 0
    let found = false
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const a = data[(y * width + x) * 4 + 3]
        if (a > 10) {
          found = true
          if (x < minX) minX = x
          if (x > maxX) maxX = x
          if (y < minY) minY = y
          if (y > maxY) maxY = y
        }
      }
    }
    if (!found) return null
    const pad = 4
    minX = Math.max(0, minX - pad)
    minY = Math.max(0, minY - pad)
    maxX = Math.min(width, maxX + pad)
    maxY = Math.min(height, maxY + pad)
    const w = Math.max(1, maxX - minX)
    const h = Math.max(1, maxY - minY)
    const out = document.createElement('canvas')
    out.width = w
    out.height = h
    const octx = out.getContext('2d')!
    octx.drawImage(canvas, minX, minY, w, h, 0, 0, w, h)
    return out.toDataURL('image/png')
  }

  function capturePng(): string | null {
    if (tab === 'logo' || current.method === 'upload') {
      return current.uploadPreview
    }
    if (current.method === 'draw') {
      const c = activeDrawCanvas()
      return c ? trimToContent(c) : null
    }
    paintTypePreview()
    return typeCanvas ? trimToContent(typeCanvas) : null
  }

  async function apply() {
    error = null
    if (tab !== 'logo' && current.method === 'type' && !displayText.trim()) {
      error = 'Escribe un nombre o iniciales'
      return
    }
    const png = capturePng()
    if (!png) {
      error = tab === 'logo' || current.method === 'upload' ? 'Sube una imagen' : 'Dibuja o escribe tu firma'
      return
    }
    saving = true
    try {
      const saved = await saveSignatureAsset({
        id: asset?.id ?? null,
        kind: tab,
        name: fullName.trim() || null,
        method: tab === 'logo' ? 'upload' : current.method,
        font: current.method === 'type' ? current.fontId : null,
        color: current.method === 'type' || current.method === 'draw' ? current.color : null,
        pngDataUrl: png,
        source: { initials, fullName },
      })
      onApply?.(saved)
      onClose?.()
    } catch (e) {
      error = String(e)
    } finally {
      saving = false
    }
  }

  async function removeAsset() {
    if (!asset || !onDelete) return
    const ok = window.confirm(`¿Borrar «${asset.name || asset.kind}»?`)
    if (!ok) return
    deleting = true
    error = null
    try {
      await onDelete(asset)
      onClose?.()
    } catch (e) {
      error = String(e)
    } finally {
      deleting = false
    }
  }
</script>

{#if open}
  <div class="sig-backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onClose?.()}>
    <div class="sig-modal" role="dialog" aria-modal="true" aria-label="Editor de firma">
      <header class="sig-head">
        <div class="sig-fields">
          <label class="mp-field">
            <span>Nombre completo</span>
            <input class="mp-input" bind:value={fullName} placeholder="Tu nombre" />
          </label>
          <label class="mp-field initials">
            <span>Iniciales</span>
            <input
              class="mp-input"
              value={initials}
              oninput={(e) => {
                initialsLocked = true
                initials = e.currentTarget.value.toUpperCase().slice(0, 4)
              }}
              placeholder="YL"
              maxlength="4"
            />
          </label>
        </div>
        <button type="button" class="mp-btn mp-btn-ghost" onclick={() => onClose?.()} aria-label="Cerrar">
          <Icon name="x" size={16} />
        </button>
      </header>

      <nav class="sig-tabs" aria-label="Tipo de activo">
        <button type="button" class:is-on={tab === 'signature'} onclick={() => (tab = 'signature')}>
          Firma
        </button>
        <button type="button" class:is-on={tab === 'initials'} onclick={() => (tab = 'initials')}>
          Iniciales
        </button>
        <button type="button" class:is-on={tab === 'logo'} onclick={() => (tab = 'logo')}>
          Logo de empresa
        </button>
      </nav>

      <div class="sig-body">
        {#if tab !== 'logo'}
          <aside class="sig-methods" aria-label="Método">
            <button
              type="button"
              class:is-on={current.method === 'type'}
              title="Escribir"
              onclick={() => (tabState = { ...tabState, [tab]: { ...current, method: 'type' } })}
            >T</button>
            <button
              type="button"
              class:is-on={current.method === 'draw'}
              title="Dibujar"
              onclick={() => (tabState = { ...tabState, [tab]: { ...current, method: 'draw' } })}
            >
              <Icon name="sign" size={16} />
            </button>
            <button
              type="button"
              class:is-on={current.method === 'upload'}
              title="Subir"
              onclick={() => (tabState = { ...tabState, [tab]: { ...current, method: 'upload' } })}
            >
              <Icon name="upload" size={16} />
            </button>
          </aside>
        {/if}

        <div class="sig-workspace">
          {#if tab === 'logo' || current.method === 'upload'}
            <div class="sig-upload">
              {#if current.uploadPreview}
                <img src={current.uploadPreview} alt="Vista previa" />
              {:else}
                <p>PNG, JPG o SVG</p>
              {/if}
              <label class="mp-btn mp-btn-ghost">
                Examinar
                <input
                  type="file"
                  accept=".png,.jpg,.jpeg,.svg,image/png,image/jpeg,image/svg+xml"
                  hidden
                  onchange={(e) => onUpload(e.currentTarget.files)}
                />
              </label>
            </div>
          {:else if current.method === 'draw'}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <canvas
              class="sig-canvas"
              width="720"
              height="220"
              bind:this={drawCanvasSig}
              hidden={tab !== 'signature'}
              onpointerdown={onDrawDown}
              onpointermove={onDrawMove}
              onpointerup={onDrawUp}
              onpointercancel={onDrawUp}
            ></canvas>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <canvas
              class="sig-canvas"
              width="720"
              height="220"
              bind:this={drawCanvasInit}
              hidden={tab !== 'initials'}
              onpointerdown={onDrawDown}
              onpointermove={onDrawMove}
              onpointerup={onDrawUp}
              onpointercancel={onDrawUp}
            ></canvas>
            <button type="button" class="mp-btn mp-btn-ghost" onclick={clearDraw}>Limpiar</button>
          {:else}
            <div class="sig-fonts">
              {#each SIGN_FONTS as f}
                <label class="sig-font-row" class:is-on={current.fontId === f.id}>
                  <input
                    type="radio"
                    name="sig-font"
                    checked={current.fontId === f.id}
                    onchange={() => (tabState = { ...tabState, [tab]: { ...current, fontId: f.id } })}
                  />
                  <span style="font-family: {f.family}; font-size: {tab === 'initials' ? '1.8rem' : '1.45rem'}; color: {current.color}">
                    {displayText}
                  </span>
                </label>
              {/each}
            </div>
            <canvas class="sig-canvas hidden" width="720" height="220" bind:this={typeCanvas}></canvas>
          {/if}

          {#if tab !== 'logo' && current.method !== 'upload'}
            <div class="sig-colors">
              <span>Color:</span>
              {#each SIGN_COLORS as c}
                <button
                  type="button"
                  class="swatch"
                  class:is-on={current.color === c.id}
                  style="background: {c.id}"
                  title={c.label}
                  aria-label={c.label}
                  onclick={() => (tabState = { ...tabState, [tab]: { ...current, color: c.id } })}
                ></button>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      {#if error}
        <p class="sig-error">{error}</p>
      {/if}

      <footer class="sig-foot" class:has-delete={isEditing && !!onDelete}>
        {#if isEditing && onDelete}
          <button
            type="button"
            class="mp-btn mp-btn-ghost sig-del"
            disabled={saving || deleting}
            onclick={() => void removeAsset()}
          >
            {deleting ? 'Borrando…' : 'Borrar'}
          </button>
        {/if}
        <div class="sig-foot-actions">
          <button type="button" class="mp-btn mp-btn-ghost" onclick={() => onClose?.()}>Cancelar</button>
          <button type="button" class="mp-btn mp-btn-primary" disabled={saving || deleting} onclick={apply}>
            {saving ? 'Guardando…' : 'Aplicar'}
          </button>
        </div>
      </footer>
    </div>
  </div>
{/if}

<style>
  .sig-backdrop {
    position: fixed; inset: 0; z-index: 80;
    background: color-mix(in srgb, var(--color-ink) 45%, transparent);
    display: grid; place-items: center; padding: 1rem;
  }
  .sig-modal {
    width: min(720px, 100%); max-height: min(92vh, 860px); overflow: auto;
    background: var(--color-paper, #fff); color: var(--color-ink);
    border: 2px solid var(--color-ink); box-shadow: 6px 6px 0 var(--color-ink);
    display: flex; flex-direction: column;
  }
  .sig-head { display: flex; gap: 0.75rem; align-items: flex-start; padding: 1rem 1rem 0.5rem; }
  .sig-fields { flex: 1; display: grid; grid-template-columns: 1fr 120px; gap: 0.75rem; }
  .sig-tabs { display: flex; border-bottom: 2px solid var(--color-ink); padding: 0 1rem; }
  .sig-tabs button {
    flex: 1; padding: 0.65rem 0.5rem; border: 0; background: transparent;
    font: inherit; font-weight: 700; color: var(--color-ink-2);
    border-top: 3px solid transparent; cursor: pointer;
  }
  .sig-tabs button.is-on { color: var(--color-ink); border-top-color: var(--color-banana, #f5d547); }
  .sig-body { display: flex; gap: 0.75rem; padding: 1rem; min-height: 280px; }
  .sig-methods { display: flex; flex-direction: column; gap: 0.4rem; }
  .sig-methods button {
    width: 40px; height: 40px; border: 2px solid var(--color-ink);
    background: var(--color-paper); cursor: pointer; display: grid; place-items: center; font-weight: 800;
  }
  .sig-methods button.is-on { background: var(--color-banana, #f5d547); }
  .sig-workspace { flex: 1; display: flex; flex-direction: column; gap: 0.75rem; min-width: 0; }
  .sig-fonts { display: flex; flex-direction: column; gap: 0.35rem; max-height: 240px; overflow: auto; }
  .sig-font-row {
    display: flex; align-items: center; gap: 0.75rem; padding: 0.55rem 0.75rem;
    border: 2px solid color-mix(in srgb, var(--color-ink) 25%, transparent); cursor: pointer;
  }
  .sig-font-row.is-on { border-color: var(--color-ink); background: color-mix(in srgb, var(--color-banana, #f5d547) 25%, transparent); }
  .sig-canvas {
    width: 100%; height: 180px; border: 2px dashed color-mix(in srgb, var(--color-ink) 40%, transparent);
    background: #fafafa; touch-action: none; cursor: crosshair;
  }
  .sig-canvas.hidden { position: absolute; left: -9999px; width: 720px; height: 220px; }
  .sig-upload {
    border: 2px dashed color-mix(in srgb, var(--color-ink) 40%, transparent);
    min-height: 180px; display: flex; flex-direction: column; align-items: center;
    justify-content: center; gap: 0.75rem; padding: 1rem;
  }
  .sig-upload img { max-width: 100%; max-height: 140px; object-fit: contain; }
  .sig-colors { display: flex; align-items: center; gap: 0.5rem; font-size: var(--text-xs); }
  .swatch { width: 22px; height: 22px; border-radius: 999px; border: 2px solid transparent; cursor: pointer; }
  .swatch.is-on { border-color: var(--color-ink); box-shadow: 2px 2px 0 var(--color-ink); }
  .sig-error { color: #b33; padding: 0 1rem; font-size: var(--text-xs); }
  .sig-foot {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem 1rem;
  }
  .sig-foot.has-delete {
    justify-content: space-between;
  }
  .sig-foot-actions {
    display: flex;
    gap: 0.5rem;
    margin-left: auto;
  }
  .sig-del:hover {
    background: var(--color-danger-soft);
    border-color: var(--color-danger);
    color: var(--color-danger);
  }
</style>
