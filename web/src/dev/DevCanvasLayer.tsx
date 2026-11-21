/**
 * Dev Mode's canvas layer (2.6, upstream `dev_mode::draw` and `saved_lines`, 2.5 §6): the
 * measurements kept on the page, the distances from the selection to the layer under the pointer
 * (Dev Mode measures on hover, without Alt), and the annotation callouts beside their layers. It
 * never takes the pointer: the canvas keeps its input.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import type { KitCanvasView } from '@kition/sdk/editor'
import { useDesignKit, useQuery } from '../kit/hooks'
import type { Rect } from '../kit/types'
import { useDev } from './store'
import { Swatch } from './widgets'
import './dev.css'

type Line = { x0: number; y0: number; x1: number; y1: number; label?: string | null; dashed?: boolean }
type Pinned = { label: string; value: string; color?: string; token?: string }
type Annotation = { id: number; index: number; label: string; category: { id: string; name: string; color: string } | null; properties: Pinned[] }

const GUIDE = '#f24822'
const ACCENT = '#00875f'
const CALLOUT_W = 220
const CALLOUT_GAP = 24

export function DevCanvasLayer({ view }: { view: KitCanvasView }) {
  const { inspect } = useDesignKit()
  const root = useRef<HTMLDivElement>(null)
  const redlines = useQuery<Array<{ lines: Line[] }>>('redlines', {})
  const hover = useHoverDistances(root, view)
  const saved = useMemo(() => (redlines ?? []).flatMap((m) => m.lines), [redlines])
  const lines = [...saved, ...(hover?.lines ?? [])]
  const sx = (x: number) => (x - view.x) * view.zoom
  const sy = (y: number) => (y - view.y) * view.zoom
  return (
    <div ref={root} className="design-dev-canvas" data-testid="design-dev-canvas">
      <Callouts view={view} selection={inspect.selection} />
      <svg width="100%" height="100%">
        {hover?.box ? <rect x={sx(hover.box.x) + 0.5} y={sy(hover.box.y) + 0.5} width={Math.max(0, hover.box.width * view.zoom - 1)} height={Math.max(0, hover.box.height * view.zoom - 1)} fill="none" stroke={GUIDE} strokeWidth={1} /> : null}
        {lines.map((line, index) => (
          <line key={index} x1={sx(line.x0)} y1={sy(line.y0)} x2={sx(line.x1)} y2={sy(line.y1)} stroke={GUIDE} strokeWidth={1} strokeDasharray={line.dashed ? '3 2' : undefined} data-testid="design-dev-measure-line" />
        ))}
      </svg>
      {lines.map((line, index) =>
        line.label ? (
          <span key={index} className="design-dev-measure-label" style={labelAt(line, sx, sy)} data-testid="design-dev-measure-label">
            {line.label}
          </span>
        ) : null,
      )}
    </div>
  )
}

/** A label centred on its line when it fits, else beside it (below a horizontal line, right of a vertical one). */
function labelAt(line: Line, sx: (x: number) => number, sy: (y: number) => number) {
  const x0 = sx(line.x0)
  const y0 = sy(line.y0)
  const x1 = sx(line.x1)
  const y1 = sy(line.y1)
  const length = Math.hypot(x1 - x0, y1 - y0)
  const width = (line.label?.length ?? 0) * 6 + 8
  const mx = (x0 + x1) / 2
  const my = (y0 + y1) / 2
  if (length >= width + 8) return { left: mx, top: my }
  const horizontal = Math.abs(x1 - x0) >= Math.abs(y1 - y0)
  return horizontal ? { left: mx, top: my + 12 } : { left: mx + width / 2 + 6, top: my }
}

/** The distances from the selection to the layer under the pointer, asked as the pointer moves. */
function useHoverDistances(root: React.RefObject<HTMLDivElement | null>, view: KitCanvasView) {
  const { query, inspect, revision } = useDesignKit()
  const [answer, setAnswer] = useState<{ lines: Line[]; box: Rect | null } | null>(null)
  const latest = useRef({ view, selection: inspect.selection, area: inspect.area, revision })
  latest.current = { view, selection: inspect.selection, area: inspect.area, revision }
  useEffect(() => {
    const el = root.current
    const cell = el?.closest('.design-canvas-cell') ?? el?.parentElement
    if (!el || !cell) return undefined
    let busy = false
    let pending: { x: number; y: number } | null = null
    let lastTarget: string | null = null
    const ask = async (point: { x: number; y: number }) => {
      busy = true
      try {
        const { selection, area } = latest.current
        if (!selection.length) {
          setAnswer(null)
          return
        }
        const under = await query<{ layers: Array<{ id: number; depth: number }> }>('layersAt', point).catch(() => null)
        const layers = under?.layers ?? []
        const target = layers.length ? layers[layers.length - 1].id : null
        if (target == null || selection.includes(target)) {
          lastTarget = null
          setAnswer(null)
          return
        }
        // Asked again once the document changed (hovering alone moves the revision too).
        const key = `${target}:${selection.join(',')}:${latest.current.revision.split('.')[0]}`
        if (key === lastTarget) return
        lastTarget = key
        const [distances, node] = await Promise.all([
          query<{ lines: Line[] }>('distances', { ids: selection, to: target }).catch(() => null),
          query<{ absoluteBounds?: Rect }>('node', { id: target }).catch(() => null),
        ])
        const b = node?.absoluteBounds
        setAnswer({ lines: distances?.lines ?? [], box: b ? { x: b.x - area.x, y: b.y - area.y, width: b.width, height: b.height } : null })
      } finally {
        busy = false
        if (pending) {
          const next = pending
          pending = null
          void ask(next)
        }
      }
    }
    const move = (event: PointerEvent) => {
      if (event.buttons) return
      const rect = el.getBoundingClientRect()
      const { view: v } = latest.current
      const point = { x: (event.clientX - rect.left) / v.zoom + v.x, y: (event.clientY - rect.top) / v.zoom + v.y }
      if (busy) pending = point
      else void ask(point)
    }
    const leave = () => {
      pending = null
      lastTarget = null
      setAnswer(null)
    }
    cell.addEventListener('pointermove', move as EventListener, true)
    cell.addEventListener('pointerleave', leave)
    return () => {
      cell.removeEventListener('pointermove', move as EventListener, true)
      cell.removeEventListener('pointerleave', leave)
    }
  }, [query, root])
  return answer
}

/** Annotation cards beside their layers, stacked so they never overlap (upstream `dev_mode::draw`). */
function Callouts({ view, selection }: { view: KitCanvasView; selection: number[] }) {
  const dev = useDev()
  // The page's annotations with their layers' bounds in document pixels (`annotation.callouts`).
  const answer = useQuery<{ callouts: Array<Annotation & { bounds: Rect | null; visible: boolean }> }>('annotation.callouts', {})
  const annotations = useMemo(() => {
    const all = (answer?.callouts ?? []).filter((a) => a.visible && a.bounds)
    return all.filter((a) => (dev.categories === 'all' ? true : dev.categories === 'none' ? !a.category : a.category?.id === dev.categories))
  }, [answer, dev.categories])
  const ids = useMemo(() => [...new Set(annotations.map((a) => a.id))], [annotations])
  const bounds = useMemo(() => new Map(annotations.map((a) => [a.id, a.bounds!] as const)), [annotations])
  // Cards and their heights: measured after the first paint, estimated before.
  const cards = useRef<Map<string, HTMLDivElement>>(new Map())
  const [heights, setHeights] = useState<Map<string, number>>(new Map())
  useEffect(() => {
    const next = new Map<string, number>()
    let changed = false
    for (const [key, el] of cards.current) {
      next.set(key, el.offsetHeight)
      if (heights.get(key) !== el.offsetHeight) changed = true
    }
    if (changed) setHeights(next)
  })
  const placed: Array<{ top: number; bottom: number; left: number; right: number }> = []
  const out: React.ReactNode[] = []
  for (const id of ids) {
    const b = bounds.get(id)
    if (!b) continue
    const left = (b.x + b.width - view.x) * view.zoom
    const top = (b.y - view.y) * view.zoom
    const x = left + CALLOUT_GAP
    const anchor = { x: left, y: top }
    let y = top
    let first = true
    for (const a of annotations.filter((item) => item.id === id)) {
      const key = `${a.id}:${a.index}`
      const height = heights.get(key) ?? 40 + a.properties.length * 18
      let rect = { left: x, right: x + CALLOUT_W, top: y, bottom: y + height }
      for (;;) {
        const hit = placed.filter((p) => p.left < rect.right + 3 && p.right > rect.left - 3 && p.top < rect.bottom + 3 && p.bottom > rect.top - 3)
        if (!hit.length) break
        const below = Math.max(...hit.map((p) => p.bottom)) + 6
        rect = { ...rect, top: below, bottom: below + height }
      }
      const color = a.category?.color ?? ACCENT
      if (first) {
        out.push(
          <svg key={`${key}:line`} width="100%" height="100%" style={{ position: 'absolute', inset: 0 }}>
            <line x1={anchor.x} y1={anchor.y} x2={rect.left} y2={rect.top + 14} stroke={color} strokeWidth={1} />
            <circle cx={anchor.x} cy={anchor.y} r={3.5} fill={color} />
          </svg>,
        )
        first = false
      }
      out.push(
        <div
          key={key}
          ref={(el) => {
            if (el) cards.current.set(key, el)
            else cards.current.delete(key)
          }}
          className="design-dev-callout"
          data-selected={selection.includes(id) || undefined}
          style={{ left: rect.left, top: rect.top }}
          data-testid={`design-dev-callout-${a.id}-${a.index}`}
        >
          <span className="design-dev-callout-bar" style={{ background: color }} />
          {a.category ? (
            <div>
              <span className="design-dev-callout-pill" style={{ background: color }}>
                {a.category.name}
              </span>
            </div>
          ) : null}
          {a.label.trim() ? <div className="design-dev-callout-note">{a.label}</div> : null}
          {a.properties.map((p, k) => (
            <div key={k} className="design-dev-callout-pinned">
              <span>{p.label}</span>
              <span>
                {p.color ? <Swatch color={p.color} /> : null}
                {p.token ?? (p.value || '—')}
              </span>
            </div>
          ))}
        </div>,
      )
      placed.push(rect)
      y = rect.bottom + 6
    }
  }
  return <>{out}</>
}
