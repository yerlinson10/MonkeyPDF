/** Middle-mouse (wheel button) pan for scrollable preview containers. */

function canScroll(el: HTMLElement): boolean {
  const style = getComputedStyle(el)
  const ox = style.overflowX
  const oy = style.overflowY
  const scrollX = ox === 'auto' || ox === 'scroll' || ox === 'overlay'
  const scrollY = oy === 'auto' || oy === 'scroll' || oy === 'overlay'
  if (!scrollX && !scrollY) return false
  return (
    (scrollY && el.scrollHeight > el.clientHeight + 1) ||
    (scrollX && el.scrollWidth > el.clientWidth + 1)
  )
}

/** Nearest scrollable ancestor within `root` (inclusive), else `root`. */
function nearestScroller(start: EventTarget | null, root: HTMLElement): HTMLElement {
  let node =
    start instanceof Element ? (start as HTMLElement) : null
  while (node) {
    if (canScroll(node)) return node
    if (node === root) break
    node = node.parentElement
  }
  return root
}

export function attachMiddlePan(el: HTMLElement): () => void {
  let panning = false
  let pointerId: number | null = null
  let target: HTMLElement = el

  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 1) return
    e.preventDefault()
    e.stopPropagation()
    panning = true
    pointerId = e.pointerId
    target = nearestScroller(e.target, el)
    try {
      el.setPointerCapture(e.pointerId)
    } catch {
      /* ignore */
    }
    el.classList.add('is-panning')
    target.classList.add('is-panning')
  }

  const onMouseDown = (e: MouseEvent) => {
    // WebView2 / Chromium: block native autoscroll early
    if (e.button !== 1) return
    e.preventDefault()
  }

  const onPointerMove = (e: PointerEvent) => {
    if (!panning) return
    e.preventDefault()
    target.scrollLeft -= e.movementX
    target.scrollTop -= e.movementY
  }

  const end = (e: PointerEvent) => {
    if (!panning) return
    panning = false
    el.classList.remove('is-panning')
    target.classList.remove('is-panning')
    if (pointerId != null) {
      try {
        el.releasePointerCapture(pointerId)
      } catch {
        /* ignore */
      }
      pointerId = null
    }
    void e
  }

  const onAuxClick = (e: MouseEvent) => {
    if (e.button === 1) {
      e.preventDefault()
      e.stopPropagation()
    }
  }

  const onContextLost = () => {
    panning = false
    el.classList.remove('is-panning')
    target.classList.remove('is-panning')
    pointerId = null
  }

  const opts: AddEventListenerOptions = { capture: true }
  el.addEventListener('pointerdown', onPointerDown, opts)
  el.addEventListener('mousedown', onMouseDown, opts)
  el.addEventListener('pointermove', onPointerMove, opts)
  el.addEventListener('pointerup', end, opts)
  el.addEventListener('pointercancel', end, opts)
  el.addEventListener('auxclick', onAuxClick, opts)
  el.addEventListener('lostpointercapture', onContextLost)

  return () => {
    el.removeEventListener('pointerdown', onPointerDown, opts)
    el.removeEventListener('mousedown', onMouseDown, opts)
    el.removeEventListener('pointermove', onPointerMove, opts)
    el.removeEventListener('pointerup', end, opts)
    el.removeEventListener('pointercancel', end, opts)
    el.removeEventListener('auxclick', onAuxClick, opts)
    el.removeEventListener('lostpointercapture', onContextLost)
    el.classList.remove('is-panning')
    target.classList.remove('is-panning')
  }
}
