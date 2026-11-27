/**
 * The order in which the canvas's layers are offered a press (R1). Several things over the canvas
 * take presses: the Prototype tab's connections and their "+" handles, the zoom tool while Z is
 * held, comment pins and threads, ruler guides and the rulers, the auto layout padding and gap
 * bands, the colour picker's gradient, crop and blur handles, the field over a frame's name, and
 * last the active tool (kit-editor's canvas, which hands the press to the engine). Each used to
 * listen on its own, so which one had a press depended on which listener ran first. Here one
 * listener, on the document in the capture phase (before any of theirs), asks them in
 * {@link PRESS_ORDER}; the first that answers has it:
 *
 * - `take`: the layer runs the gesture itself; nothing after it, nor the element pressed, sees the
 *   press (the zoom tool, a ruler guide, a click on a connection).
 * - `own`: the press is on the layer's own element (a handle, a pin, a field), which handles it;
 *   no later layer is asked.
 * - nothing: the next layer is asked. A layer may still act on a press it lets go (a press on the
 *   canvas closes an open comment thread or a prototype interaction's editor).
 *
 * A press no layer takes or owns is the tool's. Only primary-button presses in the canvas cell
 * (and on the comment layer, which floats over it) are routed; the context menu and the middle
 * button keep their own paths.
 */
import { useEffect, useRef } from 'react'

/** The layers, first asked first: the order of the upstream canvas's own input. */
export const PRESS_ORDER = ['prototype', 'zoom', 'comments', 'guides', 'handles'] as const
export type PressLayer = (typeof PRESS_ORDER)[number]

export type PressAnswer = 'take' | 'own' | undefined
/** A layer's answer to a press (`event`, a primary-button `pointerdown`). */
export type PressClaim = (event: PointerEvent) => PressAnswer

const claims = new Map<PressLayer, Set<{ current: PressClaim }>>()
/** Told after every routed press, with the layer that had it (null: the tool). */
const routed = new Set<(event: PointerEvent, by: PressLayer | null) => void>()

/** kit-editor's canvas surface, the element the tool's presses land on. */
export const SURFACE = '[data-testid="kit-canvas-surface"]'

/** Whether `target` is the canvas surface itself (no element of a layer over it). */
export function onSurface(target: EventTarget | null): target is HTMLCanvasElement {
  return target instanceof Element && target.matches(SURFACE)
}

/** The canvas surface element, if the canvas is shown. */
export function canvasSurface(): HTMLCanvasElement | null {
  return document.querySelector<HTMLCanvasElement>(SURFACE)
}

/** Whether a press on `target` is on the canvas: in its cell, or on the comment pins over it. */
function onCanvas(target: EventTarget | null) {
  return target instanceof Element && target.closest('.design-canvas-cell, .design-comment-layer') != null
}

function route(event: PointerEvent) {
  if (event.button !== 0 || !onCanvas(event.target)) return
  let by: PressLayer | null = null
  for (const layer of PRESS_ORDER) {
    let answer: PressAnswer
    for (const claim of claims.get(layer) ?? []) {
      answer = claim.current(event)
      if (answer) break
    }
    if (answer === 'take') {
      event.preventDefault()
      event.stopImmediatePropagation()
    }
    if (answer) {
      by = layer
      break
    }
  }
  for (const listener of routed) listener(event, by)
}

let installed = 0

/** Routes the canvas's presses while mounted (the canvas mounts it once). */
export function usePressRouting() {
  useEffect(() => {
    if (installed++ === 0) document.addEventListener('pointerdown', route, true)
    return () => {
      if (--installed === 0) document.removeEventListener('pointerdown', route, true)
    }
  }, [])
}

/** Offers the canvas's presses to `claim` at `layer`'s place in {@link PRESS_ORDER} while mounted. */
export function usePressLayer(layer: PressLayer, claim: PressClaim) {
  const latest = useRef(claim)
  latest.current = claim
  useEffect(() => {
    let set = claims.get(layer)
    if (!set) claims.set(layer, (set = new Set()))
    const entry = latest
    set.add(entry)
    return () => {
      set.delete(entry)
    }
  }, [layer])
}

/** Calls `listener` after each routed press with the layer that had it (null: the tool). */
export function onPressRouted(listener: (event: PointerEvent, by: PressLayer | null) => void) {
  routed.add(listener)
  return () => {
    routed.delete(listener)
  }
}

/**
 * Follows a press a layer took until its release: `move` for each move (the pointer is captured,
 * so moves off the canvas come too), `end` on the release, `cancel` when the browser cancels it.
 */
export function followPress(event: PointerEvent, handlers: { move?: (event: PointerEvent) => void; end?: (event: PointerEvent) => void; cancel?: () => void }) {
  const target = event.target instanceof Element ? event.target : null
  try {
    target?.setPointerCapture(event.pointerId)
  } catch {
    // A pointer the browser already let go: the window's events still come.
  }
  const id = event.pointerId
  const move = (e: PointerEvent) => {
    if (e.pointerId === id) handlers.move?.(e)
  }
  const up = (e: PointerEvent) => {
    if (e.pointerId !== id) return
    stop()
    handlers.end?.(e)
  }
  const cancel = (e: PointerEvent) => {
    if (e.pointerId !== id) return
    stop()
    handlers.cancel?.()
  }
  const stop = () => {
    window.removeEventListener('pointermove', move, true)
    window.removeEventListener('pointerup', up, true)
    window.removeEventListener('pointercancel', cancel, true)
  }
  window.addEventListener('pointermove', move, true)
  window.addEventListener('pointerup', up, true)
  window.addEventListener('pointercancel', cancel, true)
  return stop
}
