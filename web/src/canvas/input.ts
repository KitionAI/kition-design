/**
 * What the editor adds to kit-editor's canvas input: the view keeps the document still when the
 * engine's document area moves its origin (a new page's first layer), a mouse wheel's notch with
 * Cmd zooms one step around the pointer, and a third click while editing text selects its
 * paragraph.
 */
import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react'
import type { KitCanvasHandle } from '@kition/sdk/editor'
import type { DesignKit } from '../kit/context'

/** How much one notch of a mouse wheel zooms (with Cmd, or Ctrl off the Mac). */
export const WHEEL_NOTCH_ZOOM = 1.2

/**
 * The number of notches a wheel event is when it comes from a mouse wheel: whole lines or pages,
 * or a pixel delta that is a whole number of 100 px notches; null for a trackpad's (and a pinch's)
 * small, smooth deltas, which keep kit-editor's own zoom.
 */
export function wheelNotches(event: Pick<WheelEvent, 'deltaMode' | 'deltaX' | 'deltaY'>): number | null {
  if (event.deltaX !== 0 || event.deltaY === 0 || !Number.isFinite(event.deltaY)) return null
  if (event.deltaMode === 1) return Math.sign(event.deltaY) * Math.max(1, Math.round(Math.abs(event.deltaY) / 3))
  if (event.deltaMode === 2) return Math.sign(event.deltaY) * Math.max(1, Math.round(Math.abs(event.deltaY)))
  const notches = event.deltaY / 100
  const whole = Math.round(notches)
  return whole !== 0 && Math.abs(notches - whole) < 0.02 ? whole : null
}

/** Zooms the canvas by `factor` keeping the document point under (`clientX`, `clientY`) there. */
export function zoomAt(handle: KitCanvasHandle, surface: Element, factor: number, clientX: number, clientY: number) {
  const before = handle.view()
  if (!before || !(before.zoom > 0)) return
  const box = surface.getBoundingClientRect()
  const sx = clientX - box.left
  const sy = clientY - box.top
  const x = before.x + sx / before.zoom
  const y = before.y + sy / before.zoom
  handle.zoomTo(before.zoom * factor)
  // The canvas keeps the zoom within its range: the pointer's point stays at the zoom it took.
  const after = handle.view()
  if (!after || !(after.zoom > 0)) return
  handle.panTo(x - sx / after.zoom, y - sy / after.zoom)
}

/**
 * The engine draws the page's document area, whose origin is the canvas's (0, 0). The area grows
 * to whole chunks around the content, so a new page's first layer moves its origin (from 0, 0 to
 * -1000, -1000): the view moves by as much in the same frame, and what is on the canvas stays where
 * it was. Another page or another document is a new view of its own.
 */
export function useAreaOriginKept(designKit: DesignKit) {
  const { inspect, doc, canvas } = designKit
  const last = useRef<{ doc: unknown; page: number; x: number; y: number } | null>(null)
  const { x, y } = inspect.area
  const page = inspect.page
  useLayoutEffect(() => {
    const before = last.current
    last.current = { doc, page, x, y }
    if (!before || before.doc !== doc || before.page !== page) return
    const dx = before.x - x
    const dy = before.y - y
    if (dx === 0 && dy === 0) return
    const handle = canvas.current
    const view = handle?.view()
    if (!handle || !view) return
    handle.panTo(view.x + dx, view.y + dy)
  }, [canvas, doc, page, x, y])
}

/** The canvas surface an event happened on (kit-editor's canvas element), if any. */
function surfaceOf(target: EventTarget | null): Element | null {
  return target instanceof Element ? target.closest('[data-testid="kit-canvas-surface"]') : null
}

/**
 * The editor's own wheel and click handling on the canvas, in the capture phase on `root` (around
 * kit-editor's canvas): a mouse wheel's notch with Cmd zooms by {@link WHEEL_NOTCH_ZOOM}; a third
 * click while editing text selects the paragraph under it (`text.edit.selectLine`).
 */
export function useCanvasInput(root: RefObject<HTMLElement | null>, designKit: DesignKit, mac: boolean) {
  const latest = useRef(designKit)
  latest.current = designKit
  useEffect(() => {
    const element = root.current
    if (!element) return undefined
    const wheel = (event: WheelEvent) => {
      const surface = surfaceOf(event.target)
      const zooms = mac ? event.metaKey : event.ctrlKey
      if (!surface || !zooms) return
      const notches = wheelNotches(event)
      const handle = latest.current.canvas.current
      if (notches === null || !handle) return
      event.preventDefault()
      event.stopPropagation()
      zoomAt(handle, surface, WHEEL_NOTCH_ZOOM ** -notches, event.clientX, event.clientY)
    }
    const click = (event: MouseEvent) => {
      if (event.detail !== 3 || event.button !== 0 || !surfaceOf(event.target)) return
      const kit = latest.current
      if (kit.inspect.editing !== 'text' && !kit.tools.wantsText) return
      // After the press and release the engine already has (the press put the caret there).
      void kit.run<{ caret?: number } | null>('text.edit.selection').then((selection) => {
        if (typeof selection?.caret === 'number') void kit.run('text.edit.selectLine', { index: selection.caret })
      })
    }
    element.addEventListener('wheel', wheel, { capture: true, passive: false })
    element.addEventListener('click', click, true)
    return () => {
      element.removeEventListener('wheel', wheel, { capture: true })
      element.removeEventListener('click', click, true)
    }
  }, [mac, root])
}
