/**
 * The pointer over the canvas, as the tool (kit-editor's canvas, then the engine) sees it, and
 * the two things the editor sends it again at the same place:
 *
 * - **A modifier pressed or let go with the pointer still** (Shift, Alt, Cmd, Ctrl): the tool gets
 *   the pointer again with the new keys, so Alt's distances show, Shift constrains and Alt turns a
 *   drag into a copy without moving the mouse. The browser sends no pointer event for a key.
 * - **A drag held at the canvas's edge** pans the view toward it (`drag.autopan`, which the
 *   engine says for moving layers, a selection box, drawing, resizing and a pen handle), and the
 *   tool gets the pointer again, so what is dragged follows the document under the pointer. The
 *   speed grows from nothing 15 px inside the edge to 600 px a second at it and past it, 40 px a
 *   second more for each pixel nearer; each axis by its own distance, so a corner pans both ways.
 *
 * The events sent again are ordinary `pointermove` events on the canvas surface, so they take
 * kit-editor's path (its queue, its coalescing) like any other.
 */
import { useEffect, useRef } from 'react'
import type { KitCanvasHandle } from '@kition/sdk/editor'
import { composingKey } from '../inputMethod'
import { useUiRequest } from '../../kit/requests'
import { canvasSurface, onPressRouted, onSurface } from './order'

/** The edge zone, in screen pixels inside the canvas. */
export const AUTOPAN_ZONE_PX = 15
/** How much faster the pan gets for each pixel nearer the edge (px/s per px). */
export const AUTOPAN_RATE = 40
/** The fastest pan (px/s), at the edge and past it. */
export const AUTOPAN_MAX = 600
/** The longest frame a pan step counts (s), so a stall does not jump the view. */
const AUTOPAN_MAX_STEP = 0.1

/** The pan speed (screen px/s) with the pointer `inside` pixels inside the edge (negative: past it). */
export function autopanSpeed(inside: number) {
  if (!Number.isFinite(inside)) return 0
  return Math.min(AUTOPAN_MAX, Math.max(0, AUTOPAN_RATE * (AUTOPAN_ZONE_PX - inside)))
}

/** The pan's velocity (screen px/s; right and down positive) for the pointer at (`x`, `y`) in a canvas of `width` × `height`. */
export function autopanVelocity(x: number, y: number, width: number, height: number) {
  const axis = (at: number, length: number) => (at < length / 2 ? -autopanSpeed(at) : autopanSpeed(length - at))
  return { x: axis(x, width), y: axis(y, height) }
}

type Mods = { shiftKey: boolean; altKey: boolean; metaKey: boolean; ctrlKey: boolean }
type Seen = Mods & { clientX: number; clientY: number; pointerId: number; pointerType: string }

/** The events this module sent, which say nothing new about the pointer. */
const sentAgain = new WeakSet<Event>()

/** Sends the tool the pointer again at `at`, with `mods` and `buttons` held. */
export function sendPointerAgain(surface: Element, at: Seen, mods: Mods, buttons: number) {
  const event = new PointerEvent('pointermove', {
    bubbles: true,
    cancelable: true,
    composed: true,
    clientX: at.clientX,
    clientY: at.clientY,
    pointerId: at.pointerId,
    pointerType: at.pointerType || 'mouse',
    isPrimary: true,
    button: -1,
    buttons,
    pressure: buttons ? 0.5 : 0,
    ...mods,
  })
  // kit-editor reads a move's samples from `getCoalescedEvents()`, which a constructed event
  // leaves empty: this one is its own only sample.
  Object.defineProperty(event, 'getCoalescedEvents', { value: () => [event] })
  sentAgain.add(event)
  surface.dispatchEvent(event)
}

/**
 * An edge pan for a drag: `update` with each pointer position (client pixels) keeps a pan going,
 * one step per frame while the pointer is in the zone (also when it does not move), each step
 * moving the view and then calling `panned`; `stop` ends it.
 */
export function edgePan(canvas: () => KitCanvasHandle | null, panned: () => void) {
  let at: { x: number; y: number } | null = null
  let frame = 0
  let last = 0
  const step = (now: number) => {
    frame = 0
    const surface = canvasSurface()
    const handle = canvas()
    const view = handle?.view()
    if (!at || !surface || !handle || !view || !(view.zoom > 0)) return
    const box = surface.getBoundingClientRect()
    const v = autopanVelocity(at.x - box.left, at.y - box.top, box.width, box.height)
    if (v.x === 0 && v.y === 0) {
      last = 0
      return
    }
    const dt = last ? Math.min(AUTOPAN_MAX_STEP, Math.max(0, (now - last) / 1000)) : 1 / 60
    last = now
    handle.panTo(view.x + (v.x * dt) / view.zoom, view.y + (v.y * dt) / view.zoom)
    panned()
    frame = requestAnimationFrame(step)
  }
  return {
    update(x: number, y: number) {
      at = { x, y }
      if (!frame) frame = requestAnimationFrame(step)
    },
    stop() {
      at = null
      last = 0
      if (frame) cancelAnimationFrame(frame)
      frame = 0
    },
  }
}

const MODIFIERS = new Set(['Shift', 'Alt', 'Meta', 'Control'])
/** How long a modifier pressed while hovering waits before the tool hears it (ms). */
const HOVER_MODIFIER_MS = 120

/**
 * Follows the pointer over the canvas surface, and sends it to the tool again when a modifier
 * changes with the pointer still and while a drag the engine pans is held at the edge.
 */
export function useCanvasPointer(canvas: () => KitCanvasHandle | null) {
  const state = useRef({ seen: null as Seen | null, over: false, pressed: false, enginePans: false })
  const pan = useRef<ReturnType<typeof edgePan> | null>(null)
  useUiRequest('drag.autopan', (data) => {
    const on = (data as { on?: unknown } | null)?.on === true
    state.current.enginePans = on
    if (!on) pan.current?.stop()
    else if (state.current.pressed && state.current.seen) pan.current?.update(state.current.seen.clientX, state.current.seen.clientY)
  })
  useEffect(() => {
    const s = state.current
    const again = (mods: Mods) => {
      const surface = canvasSurface()
      if (surface && s.seen) sendPointerAgain(surface, s.seen, mods, s.pressed ? 1 : 0)
    }
    const edge = edgePan(canvas, () => {
      if (s.seen && s.pressed) again(s.seen)
    })
    pan.current = edge
    const seenFrom = (event: PointerEvent): Seen => ({
      clientX: event.clientX,
      clientY: event.clientY,
      pointerId: event.pointerId,
      pointerType: event.pointerType,
      shiftKey: event.shiftKey,
      altKey: event.altKey,
      metaKey: event.metaKey,
      ctrlKey: event.ctrlKey,
    })
    const move = (event: PointerEvent) => {
      if (sentAgain.has(event)) return
      // Over the surface, or anywhere while the tool's drag holds the pointer.
      if (!s.pressed && !onSurface(event.target)) return
      s.seen = seenFrom(event)
      if (s.pressed && s.enginePans) edge.update(event.clientX, event.clientY)
    }
    // A press the tool has (no layer over the canvas took it) starts its drag.
    const unrouted = onPressRouted((event, by) => {
      if (by !== null || !onSurface(event.target)) return
      s.pressed = true
      s.enginePans = false
      s.seen = seenFrom(event)
    })
    const release = (event: PointerEvent) => {
      if (sentAgain.has(event) || !s.pressed) return
      s.pressed = false
      s.enginePans = false
      edge.stop()
    }
    const enter = (event: PointerEvent) => {
      if (onSurface(event.target)) s.over = true
    }
    const leave = (event: PointerEvent) => {
      if (onSurface(event.target)) s.over = false
    }
    let queued: Mods | null = null
    let timer = 0
    const flush = () => {
      timer = 0
      const mods = queued
      queued = null
      if (!mods || !s.seen || !(s.over || s.pressed)) return
      if (mods.shiftKey === s.seen.shiftKey && mods.altKey === s.seen.altKey && mods.metaKey === s.seen.metaKey && mods.ctrlKey === s.seen.ctrlKey) return
      s.seen = { ...s.seen, ...mods }
      again(mods)
    }
    const key = (event: KeyboardEvent) => {
      if (!MODIFIERS.has(event.key)) {
        // A modifier that is part of a shortcut (Shift+1, Cmd+Z) sends nothing: a pointer event
        // the engine is still answering when the shortcut's key comes would hold the key back.
        if (event.type === 'keydown' && !s.pressed) {
          queued = null
          window.clearTimeout(timer)
          timer = 0
        }
        return
      }
      if (composingKey(event) || !s.seen || !(s.over || s.pressed)) return
      queued = { shiftKey: event.shiftKey, altKey: event.altKey, metaKey: event.metaKey, ctrlKey: event.ctrlKey }
      if (timer) return
      // During a drag at the next frame; hovering a moment later, after any shortcut's key.
      if (s.pressed) timer = requestAnimationFrame(flush) as unknown as number
      else timer = window.setTimeout(flush, HOVER_MODIFIER_MS)
    }
    document.addEventListener('pointermove', move, true)
    window.addEventListener('pointerup', release, true)
    window.addEventListener('pointercancel', release, true)
    document.addEventListener('pointerover', enter, true)
    document.addEventListener('pointerout', leave, true)
    window.addEventListener('keydown', key, true)
    window.addEventListener('keyup', key, true)
    return () => {
      window.clearTimeout(timer)
      cancelAnimationFrame(timer)
      unrouted()
      edge.stop()
      pan.current = null
      document.removeEventListener('pointermove', move, true)
      window.removeEventListener('pointerup', release, true)
      window.removeEventListener('pointercancel', release, true)
      document.removeEventListener('pointerover', enter, true)
      document.removeEventListener('pointerout', leave, true)
      window.removeEventListener('keydown', key, true)
      window.removeEventListener('keyup', key, true)
    }
  }, [canvas])
}
