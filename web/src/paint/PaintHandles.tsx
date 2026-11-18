/**
 * Canvas handles for the paint the colour picker edits (2.5 §15, upstream `canvas/handles.rs`):
 * a gradient's line, its from / to / width handles and its stops (drag a stop along the line, click
 * the line to add one, Delete removes the selected one), an image's crop box in Crop mode (corners
 * scale about the opposite corner, keeping the aspect unless Shift; inside moves), and a
 * progressive blur's start and end points. Positions come from `query('paintHandles')` and
 * `query('blurHandles')`; every drag is one undo step.
 */
import { useEffect, useMemo, useRef } from 'react'
import { useDesignKit, useQuery, useView } from '../kit/hooks'
import { useCanvasOverlay } from '../canvas/overlays'
import { cssColor } from './color'
import type { ChangePhase } from './ColorPicker'
import { type Paint, type Point, sample } from './paint'
import { setStop } from './store'

type Matrix = [number, number, number, number, number, number]
type GradientHandles = { kind: 'gradient'; gradientType: string; from: Point; to: Point; width: Point; stops: Array<{ position: number; color: string; point: Point }>; unit: Matrix }
type CropHandles = { kind: 'crop'; corners: Point[]; rect: [number, number, number, number]; unit: Matrix }
type Handles = GradientHandles | CropHandles | { kind: 'none' }

const WHITE = '#ffffff'

/** The page's accent for handles (`canvas_accent`): the brand green on dark pages, the darker one on light. */
function canvasAccent(background: string): string {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(background)
  if (!m) return '#00875f'
  const lum = (0.2126 * parseInt(m[1], 16) + 0.7152 * parseInt(m[2], 16) + 0.0722 * parseInt(m[3], 16)) / 255
  return lum < 0.5 ? '#02da96' : '#00875f'
}

/** Document point → unit box of the layer (inverse of `unit`). */
function toUnit(m: Matrix, [x, y]: Point): Point {
  const [a, b, c, d, e, f] = m
  const det = a * d - b * c
  if (Math.abs(det) < 1e-12) return [0, 0]
  const dx = x - e
  const dy = y - f
  return [(d * dx - c * dy) / det, (-b * dx + a * dy) / det]
}

/** The layer's size in document pixels (the unit box's sides). */
const sizeOf = (m: Matrix): [number, number] => [Math.hypot(m[0], m[1]), Math.hypot(m[2], m[3])]

/** `Gradient::square_linear_width`: a linear gradient's width handle square to its axis on screen. */
function squareWidth(from: Point, to: Point, [w0, h0]: [number, number]): Point {
  const [w, h] = w0 > 0 && h0 > 0 && Number.isFinite(w0) && Number.isFinite(h0) ? [w0, h0] : [1, 1]
  const dx = (to[0] - from[0]) * w
  const dy = (to[1] - from[1]) * h
  return [from[0] - dy / w, from[1] + dx / h]
}

/** `crop_from_rect`: the crop transform placing the image at (x0, y0)–(x1, y1) of the unit box. */
function cropFromRect(x0: number, y0: number, x1: number, y1: number): number[] {
  const a = 1 / Math.max(Math.abs(x1 - x0), 1e-6)
  const d = 1 / Math.max(Math.abs(y1 - y0), 1e-6)
  return [a, 0, 0, d, -Math.min(x0, x1) * a, -Math.min(y0, y1) * d]
}

/** Starts a drag on `event`: `move(doc point, shift)` while the pointer moves, `end()` on release. */
function drag(event: React.PointerEvent<Element>, toDoc: (cx: number, cy: number) => Point, move: (p: Point, shift: boolean) => void, end: () => void) {
  if (event.button !== 0) return
  event.preventDefault()
  event.stopPropagation()
  const el = event.currentTarget
  try {
    el.setPointerCapture(event.pointerId)
  } catch {
    // Synthetic pointer.
  }
  const onMove = (e: PointerEvent) => move(toDoc(e.clientX, e.clientY), e.shiftKey)
  const onUp = () => {
    el.removeEventListener('pointermove', onMove as EventListener)
    el.removeEventListener('pointerup', onUp)
    el.removeEventListener('pointercancel', onUp)
    end()
  }
  el.addEventListener('pointermove', onMove as EventListener)
  el.addEventListener('pointerup', onUp)
  el.addEventListener('pointercancel', onUp)
}

/** Gradient and crop handles for paint `index` of layer `id` (the picker's), written with `onChange`. */
export function PaintHandles({ id, index, fills, paint, stop, onChange }: { id: number; index: number; fills: boolean; paint: Paint; stop: number; onChange: (paint: Paint, phase: ChangePhase) => void }) {
  const { inspect } = useDesignKit()
  const view = useView()
  const answer = useQuery<Handles>('paintHandles', { id, index, fills })
  const latest = useRef({ paint, onChange, stop })
  latest.current = { paint, onChange, stop }
  const svg = useRef<SVGSVGElement>(null)
  const accent = canvasAccent(inspect.background)
  const gradient = paint.type === 'gradient' ? paint.gradient : null
  // The element changes with what it draws, not with every new paint object.
  const gradientKey = gradient ? JSON.stringify(gradient) : ''

  // Delete removes the selected stop while a gradient is open (two always stay).
  useEffect(() => {
    if (!gradient) return undefined
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Delete' && event.key !== 'Backspace') return
      const target = event.target as HTMLElement | null
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) return
      const { paint: p, stop: s, onChange: write } = latest.current
      const g = p.gradient
      if (!g || g.stops.length <= 2 || s >= g.stops.length) return
      event.preventDefault()
      event.stopPropagation()
      setStop(Math.max(0, s - 1))
      write({ ...p, gradient: { ...g, stops: g.stops.filter((_, i) => i !== s) } }, 'done')
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [gradientKey])

  const element = useMemo(() => {
    if (!view || !answer || answer.kind === 'none') return null
    const sx = (p: Point): Point => [(p[0] - view.x) * view.zoom, (p[1] - view.y) * view.zoom]
    const toDoc = (cx: number, cy: number): Point => {
      const r = svg.current?.getBoundingClientRect()
      return [view.x + (cx - (r?.left ?? 0)) / view.zoom, view.y + (cy - (r?.top ?? 0)) / view.zoom]
    }
    if (answer.kind === 'gradient') {
      const g = latest.current.paint.gradient
      if (!g) return null
      const [a, b, c] = [sx(answer.from), sx(answer.to), sx(answer.width)]
      const linear = answer.gradientType === 'linear'
      const size = sizeOf(answer.unit)
      const writeG = (next: typeof g, phase: ChangePhase) => latest.current.onChange({ ...latest.current.paint, gradient: next }, phase)
      const endDrag = (which: 0 | 1 | 2) => (event: React.PointerEvent<Element>) => {
        let next = latest.current.paint.gradient
        if (!next) return
        drag(
          event,
          toDoc,
          (p) => {
            const u = toUnit(answer.unit, p)
            const base = latest.current.paint.gradient ?? next!
            const out = { ...base, from: which === 0 ? u : base.from, to: which === 1 ? u : base.to, width: which === 2 ? u : base.width }
            if (which < 2 && out.kind === 'linear') out.width = squareWidth(out.from, out.to, size)
            next = out
            writeG(out, 'live')
          },
          () => next && writeG(next, 'done'),
        )
      }
      /** Where the pointer projects on the from → to line (0..1). */
      const along = (p: Point) => {
        const [ax, ay] = answer.from
        const [bx, by] = answer.to
        const dx = bx - ax
        const dy = by - ay
        const len2 = dx * dx + dy * dy
        return len2 < 1e-9 ? 0.5 : Math.min(1, Math.max(0, ((p[0] - ax) * dx + (p[1] - ay) * dy) / len2))
      }
      const stopDrag = (i: number, event: React.PointerEvent<Element>, added: typeof g | null) => {
        let next = added ?? latest.current.paint.gradient
        if (!next) return
        setStop(i)
        if (added) writeG(added, 'live')
        drag(
          event,
          toDoc,
          (p) => {
            const base = next!
            next = { ...base, stops: base.stops.map((s, k) => (k === i ? { ...s, position: along(p) } : s)) }
            writeG(next, 'live')
          },
          () => next && writeG(next, 'done'),
        )
      }
      return (
        <svg ref={svg} className="design-paint-handles" data-testid="design-gradient-handles">
          <line x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} stroke={WHITE} strokeWidth={2} />
          <line x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} stroke="rgba(0,0,0,0.47)" strokeWidth={1} />
          {!linear ? <line x1={a[0]} y1={a[1]} x2={c[0]} y2={c[1]} stroke={WHITE} strokeWidth={1} strokeDasharray="4 3" /> : null}
          <line
            className="design-handle-hit"
            x1={a[0]}
            y1={a[1]}
            x2={b[0]}
            y2={b[1]}
            stroke="transparent"
            strokeWidth={10}
            style={{ cursor: 'copy' }}
            data-testid="design-gradient-line"
            onPointerDown={(event) => {
              const base = latest.current.paint.gradient
              if (!base) return
              const pos = along(toDoc(event.clientX, event.clientY))
              const added = { ...base, stops: [...base.stops, { position: pos, color: sample(base, pos) }] }
              stopDrag(base.stops.length, event, added)
            }}
          />
          {answer.stops.map((s, i) => {
            const [x, y] = sx(s.point)
            const fill = g.stops[i] ? cssColor(g.stops[i].color, 1) : s.color.slice(0, 7)
            return (
              <g key={i} className="design-handle-hit" style={{ cursor: 'grab' }} data-testid={`design-gradient-stop-${i}`} onPointerDown={(event) => stopDrag(i, event, null)}>
                {i === latest.current.stop ? <rect x={x - 9} y={y - 9} width={18} height={18} rx={3} fill="none" stroke={accent} strokeWidth={1.5} /> : null}
                <rect x={x - 7} y={y - 7} width={14} height={14} rx={3} fill={WHITE} />
                <rect x={x - 6} y={y - 6} width={12} height={12} rx={2} fill={fill} />
              </g>
            )
          })}
          {([a, b, c] as Point[]).map((p, which) =>
            which === 2 && linear ? null : (
              <circle key={which} className="design-handle-hit" cx={p[0]} cy={p[1]} r={5} fill={WHITE} stroke={accent} strokeWidth={1.5} style={{ cursor: 'grab' }} data-testid={`design-gradient-handle-${which}`} onPointerDown={endDrag(which as 0 | 1 | 2)} />
            ),
          )}
        </svg>
      )
    }
    // Crop.
    const corners = answer.corners.map(sx)
    const [x0, y0, x1, y1] = answer.rect
    const [w, h] = sizeOf(answer.unit)
    const writeCrop = (r: [number, number, number, number], phase: ChangePhase) => {
      const p = latest.current.paint
      if (p.type !== 'image' || !p.image) return
      latest.current.onChange({ ...p, image: { ...p.image, crop: cropFromRect(...r) } }, phase)
    }
    const cropDrag = (which: number) => (event: React.PointerEvent<Element>) => {
      const start = toUnit(answer.unit, toDoc(event.clientX, event.clientY))
      let last: [number, number, number, number] | null = null
      drag(
        event,
        toDoc,
        (p, shift) => {
          const u = toUnit(answer.unit, p)
          let r: [number, number, number, number]
          if (which === 4) {
            const dx = u[0] - start[0]
            const dy = u[1] - start[1]
            r = [x0 + dx, y0 + dy, x1 + dx, y1 + dy]
          } else {
            const cs: Point[] = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
            const [ax, ay] = cs[(which + 2) % 4]
            const [cx, cy] = cs[which]
            const ow = cx - ax
            const oh = cy - ay
            let nw = u[0] - ax
            let nh = u[1] - ay
            if (!shift) {
              const k = Math.max(Math.abs(nw * w) / Math.max(Math.abs(ow * w), 1e-9), Math.abs(nh * h) / Math.max(Math.abs(oh * h), 1e-9))
              nw = ow * k
              nh = oh * k
            }
            if (Math.abs(nw) < 1e-3 || Math.abs(nh) < 1e-3 || Math.sign(nw) !== Math.sign(ow) || Math.sign(nh) !== Math.sign(oh)) return
            r = [Math.min(ax, ax + nw), Math.min(ay, ay + nh), Math.max(ax, ax + nw), Math.max(ay, ay + nh)]
          }
          last = r
          writeCrop(r, 'live')
        },
        () => last && writeCrop(last, 'done'),
      )
    }
    const outline = corners.map((p) => p.join(',')).join(' ')
    return (
      <svg ref={svg} className="design-paint-handles" data-testid="design-crop-handles">
        <polygon points={outline} fill="transparent" stroke={WHITE} strokeWidth={2} className="design-handle-hit" style={{ cursor: 'move' }} onPointerDown={cropDrag(4)} />
        <polygon points={outline} fill="none" stroke={accent} strokeWidth={1} />
        {corners.map((p, i) => (
          <rect key={i} className="design-handle-hit" x={p[0] - 4} y={p[1] - 4} width={8} height={8} fill={WHITE} stroke={accent} strokeWidth={1} style={{ cursor: i % 2 === 0 ? 'nwse-resize' : 'nesw-resize' }} onPointerDown={cropDrag(i)} data-testid={`design-crop-corner-${i}`} />
        ))}
      </svg>
    )
    // `stop` and `paint` re-render through `latest`; the element follows the handles and the view.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [accent, answer, view, stop, gradientKey])
  useCanvasOverlay('paint.handles', element)
  return null
}

/** A progressive blur's start (hollow) and end (filled) points, dragged with `effect.set`. */
export function BlurHandles({ id, index, effect, onChange }: { id: number; index: number; effect: Record<string, unknown>; onChange: (effect: Record<string, unknown>, phase: ChangePhase) => void }) {
  const { inspect } = useDesignKit()
  const view = useView()
  const answer = useQuery<{ start: Point; end: Point; unit: Matrix } | null>('blurHandles', { id, index })
  const svg = useRef<SVGSVGElement>(null)
  const latest = useRef({ effect, onChange })
  latest.current = { effect, onChange }
  const accent = canvasAccent(inspect.background)
  const element = useMemo(() => {
    if (!view || !answer) return null
    const sx = (p: Point): Point => [(p[0] - view.x) * view.zoom, (p[1] - view.y) * view.zoom]
    const toDoc = (cx: number, cy: number): Point => {
      const r = svg.current?.getBoundingClientRect()
      return [view.x + (cx - (r?.left ?? 0)) / view.zoom, view.y + (cy - (r?.top ?? 0)) / view.zoom]
    }
    const [a, b] = [sx(answer.start), sx(answer.end)]
    const start = (key: 'start' | 'end') => (event: React.PointerEvent<Element>) => {
      let next: Record<string, unknown> | null = null
      drag(
        event,
        toDoc,
        (p) => {
          next = { ...latest.current.effect, [key]: toUnit(answer.unit, p) }
          latest.current.onChange(next, 'live')
        },
        () => next && latest.current.onChange(next, 'done'),
      )
    }
    return (
      <svg ref={svg} className="design-paint-handles" data-testid="design-blur-handles">
        <line x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} stroke={WHITE} strokeWidth={2} />
        <line x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} stroke="rgba(0,0,0,0.47)" strokeWidth={1} />
        <circle className="design-handle-hit" cx={a[0]} cy={a[1]} r={5} fill={WHITE} stroke={accent} strokeWidth={1.5} style={{ cursor: 'grab' }} onPointerDown={start('start')} data-testid="design-blur-start" />
        <circle className="design-handle-hit" cx={b[0]} cy={b[1]} r={5} fill={accent} stroke={WHITE} strokeWidth={1.5} style={{ cursor: 'grab' }} onPointerDown={start('end')} data-testid="design-blur-end" />
      </svg>
    )
  }, [accent, answer, view])
  useCanvasOverlay(`paint.blur.${index}`, element)
  return null
}
