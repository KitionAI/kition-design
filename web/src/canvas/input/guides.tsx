/**
 * Ruler guides on the canvas (R-B16), while the rulers show (Shift+R): drag out of the top ruler
 * for a horizontal guide or the left one for a vertical guide; drag a guide to move it (Alt at the
 * press: a copy, the guide stays); drag one back onto its ruler to remove it; a click selects it,
 * and then Delete removes it, the arrows nudge it (Shift: the big nudge) and Escape lets it go
 * (the engine's keys for a selected guide). A guide dropped on a top-level frame belongs to the
 * frame (it moves with it and spans only it); elsewhere it is the page's, across the whole canvas.
 * While dragged it lines up with an edge or centre of a layer within 5 screen pixels (not with
 * Cmd or Ctrl held, nor before a guide that was only clicked has moved), else lands on whole
 * pixels. Existing guides move with the Move tool, not while a text or a vector is edited.
 * Where it lands comes from the engine (`query('guidePlace')`); the edit is one command, one
 * undo step (`guide.add`, `guide.move`, `guide.remove`).
 */
import { useEffect, useRef, useState } from 'react'
import type { KitCanvasView } from '@kition/sdk/editor'
import type { DesignKit } from '../../kit/context'
import type { DesignChrome } from '../../kit/types'
import { canvasSurface, followPress, onSurface, usePressLayer } from './order'

/** The rulers' thickness (screen pixels), as `chrome.ts` draws them. */
export const RULER_PX = 20
/** How near (screen pixels) a press must be to a guide to take it. */
const GUIDE_REACH_PX = 3

type Guide = DesignChrome['guides'][number]
/** Where a dragged guide lands (`query('guidePlace')`). */
export type GuidePlace = { owner: number; page: boolean; vertical: boolean; offset: number; a: [number, number] | null; b: [number, number] | null; snapped: boolean }
type Drag =
  | { kind: 'new'; vertical: boolean }
  | { kind: 'move'; owner: number; page: boolean; index: number; vertical: boolean; offset: number; copy: boolean; from: { x: number; y: number } }

/** The ends of `guide`'s line on the canvas (canvas pixels); a page guide crosses the canvas. */
function screenLine(guide: { a: [number, number] | null; b: [number, number] | null; vertical: boolean; offset: number; page: boolean }, view: { x: number; y: number; zoom: number }, area: { x: number; y: number }, size: { width: number; height: number }) {
  if (guide.a && guide.b) {
    return [
      { x: (guide.a[0] - view.x) * view.zoom, y: (guide.a[1] - view.y) * view.zoom },
      { x: (guide.b[0] - view.x) * view.zoom, y: (guide.b[1] - view.y) * view.zoom },
    ]
  }
  if (guide.vertical) {
    const x = (guide.offset - area.x - view.x) * view.zoom
    return [
      { x, y: 0 },
      { x, y: size.height },
    ]
  }
  const y = (guide.offset - area.y - view.y) * view.zoom
  return [
    { x: 0, y },
    { x: size.width, y },
  ]
}

/** The distance from `p` to the segment `a`–`b`. */
function distance(p: { x: number; y: number }, a: { x: number; y: number }, b: { x: number; y: number }) {
  const dx = b.x - a.x
  const dy = b.y - a.y
  const length = dx * dx + dy * dy
  const t = length > 1e-9 ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / length)) : 0
  return Math.hypot(a.x + t * dx - p.x, a.y + t * dy - p.y)
}

/** The resize cursor across a line running from `a` to `b` on the screen. */
function cursorAcross(a: { x: number; y: number }, b: { x: number; y: number }) {
  const eighths = Math.floor((((Math.atan2(b.y - a.y, b.x - a.x) * 180) / Math.PI + 180) % 180 + 22.5) / 45) % 4
  return ['ns-resize', 'nesw-resize', 'ew-resize', 'nwse-resize'][eighths]!
}

/** The guide whose line on the canvas passes within reach of canvas point `p`, frame guides first. */
function guideAt(chrome: DesignChrome, view: { x: number; y: number; zoom: number }, area: { x: number; y: number }, size: { width: number; height: number }, p: { x: number; y: number }) {
  const ordered = [...chrome.guides].sort((a, b) => Number(Boolean(a.page)) - Number(Boolean(b.page)))
  for (const guide of ordered) {
    const [a, b] = screenLine({ ...guide, page: Boolean(guide.page) }, view, area, size)
    if (distance(p, a!, b!) <= GUIDE_REACH_PX) return { guide, a: a!, b: b! }
  }
  return null
}

/**
 * Ruler guides: the presses on the rulers and on guides (at `guides` in the canvas's press order),
 * the cursor over them, and the guide being dragged: `place` where it lands (null: nowhere, or
 * back on its ruler), `hidden` the guide it moves (drawn at `place` instead).
 */
export function useRulerGuides(designKit: DesignKit, chrome: DesignChrome | null) {
  const latest = useRef({ designKit, chrome })
  latest.current = { designKit, chrome }
  const [place, setPlace] = useState<GuidePlace | null>(null)
  const [hidden, setHidden] = useState<{ owner: number; index: number } | null>(null)
  const [dragging, setDragging] = useState<string | null>(null)
  const [hover, setHover] = useState<string | null>(null)

  /** The press or pointer at client point (`x`, `y`) on the surface: canvas point, view, its box. */
  const where = (x: number, y: number) => {
    const surface = canvasSurface()
    const view = latest.current.designKit.canvas.current?.view()
    if (!surface || !view || !(view.zoom > 0)) return null
    const box = surface.getBoundingClientRect()
    return { p: { x: x - box.left, y: y - box.top }, view, box }
  }

  // The cursor over the rulers and over guides (not while a button is held).
  useEffect(() => {
    const move = (event: PointerEvent) => {
      const { chrome: c, designKit: kit } = latest.current
      if (event.buttons !== 0 || !c?.rulers || !onSurface(event.target)) {
        setHover((was) => (was && event.buttons === 0 ? null : was))
        return
      }
      const at = where(event.clientX, event.clientY)
      if (!at) return
      const { p, view, box } = at
      let next: string | null = null
      if (p.x < RULER_PX && p.y >= RULER_PX) next = 'ew-resize'
      else if (p.y < RULER_PX && p.x >= RULER_PX) next = 'ns-resize'
      else if (p.x >= RULER_PX && p.y >= RULER_PX && idle(kit)) {
        const hit = guideAt(c, view, kit.inspect.area, box, p)
        if (hit) next = cursorAcross(hit.a, hit.b)
      }
      setHover((was) => (was === next ? was : next))
    }
    document.addEventListener('pointermove', move, true)
    return () => document.removeEventListener('pointermove', move, true)
    // `where` reads the latest state through refs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  usePressLayer('guides', (event) => {
    const { chrome: c, designKit: kit } = latest.current
    if (!c?.rulers || kit.readOnly) return undefined
    const at = where(event.clientX, event.clientY)
    if (!at) return undefined
    const { p, view, box } = at
    if (p.x < 0 || p.y < 0 || p.x > box.width || p.y > box.height) return undefined
    const inLeft = p.x < RULER_PX && p.y >= RULER_PX
    const inTop = p.y < RULER_PX && p.x >= RULER_PX
    let drag: Drag
    if (inLeft || inTop) drag = { kind: 'new', vertical: inLeft }
    else if (p.x < RULER_PX || p.y < RULER_PX) return 'take'
    else {
      if (!idle(kit)) return undefined
      const hit = guideAt(c, view, kit.inspect.area, box, p)
      if (!hit || hit.guide.owner === null) return undefined
      const g: Guide = hit.guide
      const owner = g.owner as number
      const page = Boolean(g.page)
      void kit.run('guide.select', page ? { index: g.index } : { index: g.index, frame: owner })
      drag = { kind: 'move', owner, page, index: g.index, vertical: g.vertical, offset: g.offset, copy: event.altKey, from: { x: event.clientX, y: event.clientY } }
      if (!drag.copy) setHidden({ owner, index: g.index })
    }
    begin(event, drag)
    return 'take'
  })

  const begin = (event: PointerEvent, drag: Drag) => {
    setDragging(drag.vertical ? 'ew-resize' : 'ns-resize')
    let asked = 0
    let landed: GuidePlace | null = null
    let moved = false
    /** Where the guide lands with the pointer at `e`. */
    const ask = async (e: { clientX: number; clientY: number; metaKey: boolean; ctrlKey: boolean }): Promise<GuidePlace | null> => {
      const at = where(e.clientX, e.clientY)
      const kit = latest.current.designKit
      if (!at) return null
      if (drag.kind === 'move' && Math.hypot(e.clientX - drag.from.x, e.clientY - drag.from.y) > 0.5) moved = true
      const params = {
        x: at.view.x + at.p.x / at.view.zoom,
        y: at.view.y + at.p.y / at.view.zoom,
        vertical: drag.vertical,
        snap: !(e.metaKey || e.ctrlKey) && (drag.kind === 'new' || moved),
        zoom: at.view.zoom,
        ...(drag.kind === 'move' ? { owner: drag.owner } : {}),
      }
      const text = await kit.invoke<string>('query', 'guidePlace', JSON.stringify(params)).catch(() => 'null')
      return JSON.parse(text) as GuidePlace | null
    }
    /** Whether the pointer at `e` is back on the ruler of a guide running as `landed` does. */
    const onRuler = (e: { clientX: number; clientY: number }, guide: GuidePlace | null) => {
      const at = where(e.clientX, e.clientY)
      if (!at) return false
      const area = latest.current.designKit.inspect.area
      const down = guide
        ? (() => {
            const [a, b] = screenLine(guide, at.view, area, at.box)
            return Math.abs(b!.y - a!.y) >= Math.abs(b!.x - a!.x)
          })()
        : drag.vertical
      return down ? at.p.x < RULER_PX : at.p.y < RULER_PX
    }
    const show = (e: PointerEvent) => {
      const mine = ++asked
      void ask(e).then((next) => {
        if (mine !== asked) return
        landed = next
        setPlace(next && !onRuler(e, next) ? next : null)
      })
    }
    show(event)
    const done = () => {
      asked += 1
      setPlace(null)
      setHidden(null)
      setDragging(null)
    }
    followPress(event, {
      move: show,
      end: (e) => {
        asked += 1
        void ask(e).then((final) => {
          const back = onRuler(e, final ?? landed)
          done()
          const kit = latest.current.designKit
          const frame = (owner: number, page: boolean) => (page ? {} : { frame: owner })
          if (drag.kind === 'move' && !drag.copy) {
            if (back) void kit.run('guide.remove', { index: drag.index, ...frame(drag.owner, drag.page) })
            else if (final && moved && final.offset !== drag.offset) void kit.run('guide.move', { index: drag.index, offset: final.offset, ...frame(drag.owner, drag.page) })
            return
          }
          if (back || !final) return
          if (drag.kind === 'move' && final.offset === drag.offset) return
          void kit.run('guide.add', { vertical: final.vertical, offset: final.offset, ...frame(final.owner, final.page) })
        })
      },
      cancel: done,
    })
  }

  return { place, hidden, cursor: dragging ?? hover }
}

/** Whether existing guides take presses now: the Move tool, no text or vector being edited. */
function idle(kit: DesignKit) {
  return kit.inspect.tool === 'move' && kit.inspect.editing !== 'text' && !kit.inspect.vectorEdit && !kit.tools.wantsText
}

/** The guide being dragged, where it will land, with its offset. */
export function GuidePreview({ place, view, area, color }: { place: GuidePlace | null; view: KitCanvasView | null; area: { x: number; y: number }; color: string }) {
  if (!place || !view) return null
  const [a, b] = screenLine(place, view, area, view)
  const label = Number.isInteger(place.offset) ? String(place.offset) : place.offset.toFixed(2).replace(/\.?0+$/, '')
  const tagX = Math.min(Math.max(a!.x, RULER_PX), view.width) + 4
  const tagY = Math.min(Math.max(a!.y, RULER_PX), view.height) + 4
  return (
    <svg className="design-guide-preview" data-testid="design-guide-preview" width="100%" height="100%">
      <line x1={a!.x} y1={a!.y} x2={b!.x} y2={b!.y} stroke={color} strokeWidth={1} />
      <g transform={`translate(${tagX} ${tagY})`}>
        <rect width={label.length * 6 + 8} height={14} rx={3} fill={color} />
        <text x={4} y={10.5} className="design-guide-preview-label">
          {label}
        </text>
      </g>
    </svg>
  )
}
