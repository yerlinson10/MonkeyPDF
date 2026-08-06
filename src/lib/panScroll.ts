/** Middle-mouse (wheel button) pan for scrollable preview containers. */
export function attachMiddlePan(el: HTMLElement): () => void {
  let panning = false
  let pointerId: number | null = null

  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 1) return
    e.preventDefault()
    panning = true
    pointerId = e.pointerId
    el.setPointerCapture(e.pointerId)
    el.classList.add('is-panning')
  }

  const onPointerMove = (e: PointerEvent) => {
    if (!panning) return
    el.scrollLeft -= e.movementX
    el.scrollTop -= e.movementY
  }

  const end = (e: PointerEvent) => {
    if (!panning) return
    panning = false
    el.classList.remove('is-panning')
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
    if (e.button === 1) e.preventDefault()
  }

  const onContextLost = () => {
    panning = false
    el.classList.remove('is-panning')
    pointerId = null
  }

  el.addEventListener('pointerdown', onPointerDown)
  el.addEventListener('pointermove', onPointerMove)
  el.addEventListener('pointerup', end)
  el.addEventListener('pointercancel', end)
  el.addEventListener('auxclick', onAuxClick)
  el.addEventListener('lostpointercapture', onContextLost)

  return () => {
    el.removeEventListener('pointerdown', onPointerDown)
    el.removeEventListener('pointermove', onPointerMove)
    el.removeEventListener('pointerup', end)
    el.removeEventListener('pointercancel', end)
    el.removeEventListener('auxclick', onAuxClick)
    el.removeEventListener('lostpointercapture', onContextLost)
    el.classList.remove('is-panning')
  }
}
