/**
 * The Prototype tab's canvas layer (2.5 §16, upstream `canvas/proto_canvas.rs`): a connection
 * per interaction from its layer to its destination (`query('protoConnections')`), brighter when
 * its layer is selected, its editor is open or the pointer is near; the destination outlined; a
 * "+" handle on each selected layer that drags out a connection and drops it on a frame
 * (`proto.interaction.add { ids, dest }`, then its editor opens). A click on a connection
 * selects its layer and opens its editor; a press elsewhere closes the editor.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import type { KitCanvasView } from '@kition/kit-editor'
import { useDesignKit, useQuery } from '../kit/hooks'
import type { Rect } from '../kit/types'
import { topLevelFrame, useTree } from './data'
import { openEditor, useProtoState } from './store'
import './prototype.css'

type Point = [number, number]
type Connection = { source: number; index: number; dest: number; points: Point[]; arrow: Point[]; selected: boolean; destination: Rect | null }
type Answer = { connections: Connection[]; handles: Array<{ id: number; x: number; y: number }>; handleRadius: number; frameAt: number | null }

const BRAND = '#02da96'
const ACCENT = '#00875f'
/** Within this many screen pixels of a connection the pointer is "near" it (upstream 5). */
const NEAR = 5

/** `canvas_accent`: whichever of BRAND and ACCENT contrasts more with the page background. */
export function canvasAccent(background: string) {
  const n = parseInt(background.replace('#', ''), 16)
  if (!Number.isFinite(n)) return ACCENT
  const lum = (v: number) => {
    const c = v / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  }
  const l = (hex: number) => 0.2126 * lum((hex >> 16) & 255) + 0.7152 * lum((hex >> 8) & 255) + 0.0722 * lum(hex & 255)
  const contrast = (a: number, b: number) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
  const page = l(n)
  return contrast(page, l(0x02da96)) >= contrast(page, l(0x00875f)) ? BRAND : ACCENT
}

const bezierAt = ([a, b, c, d]: Point[], t: number): Point => {
  const u = 1 - t
  const k = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t]
  return [k[0]! * a![0] + k[1]! * b![0] + k[2]! * c![0] + k[3]! * d![0], k[0]! * a![1] + k[1]! * b![1] + k[2]! * c![1] + k[3]! * d![1]]
}

/** The distance from `p` to the cubic (32 samples), in the points' units. */
function distance(points: Point[], p: Point) {
  let best = Infinity
  let prev = bezierAt(points, 0)
  for (let i = 1; i <= 32; i += 1) {
    const next = bezierAt(points, i / 32)
    const dx = next[0] - prev[0]
    const dy = next[1] - prev[1]
    const len = dx * dx + dy * dy
    const t = len > 0 ? Math.max(0, Math.min(1, ((p[0] - prev[0]) * dx + (p[1] - prev[1]) * dy) / len)) : 0
    best = Math.min(best, Math.hypot(prev[0] + t * dx - p[0], prev[1] + t * dy - p[1]))
    prev = next
  }
  return best
}

export function ProtoCanvasLayer({ view }: { view: KitCanvasView }) {
  const { run, query, inspect, ui } = useDesignKit()
  const zoom = Math.round(view.zoom * 1000) / 1000
  const answer = useQuery<Answer>('protoConnections', { zoom })
  const tree = useTree()
  const editor = useProtoState((s) => s.editor)
  const root = useRef<HTMLDivElement>(null)
  const [near, setNear] = useState<string | null>(null)
  const [drag, setDrag] = useState<{ source: number; from: Point; to: Point; frame: number | null; bounds: Rect | null } | null>(null)
  const accent = canvasAccent(inspect.background)
  const toScreen = (x: number, y: number): Point => [(x - view.x) * view.zoom, (y - view.y) * view.zoom]
  const connections = useMemo(() => answer?.connections ?? [], [answer])
  const latest = useRef({ connections, view })
  latest.current = { connections, view }

  /** The connection within `NEAR` screen pixels of a client point. */
  const hitAt = (clientX: number, clientY: number) => {
    const box = root.current?.getBoundingClientRect()
    if (!box) return null
    const { connections: list, view: v } = latest.current
    const p: Point = [(clientX - box.left) / v.zoom + v.x, (clientY - box.top) / v.zoom + v.y]
    let best: Connection | null = null
    let bestDistance = NEAR / v.zoom
    for (const c of list) {
      const d = distance(c.points, p)
      if (d < bestDistance) {
        bestDistance = d
        best = c
      }
    }
    return best
  }

  // Hover and clicks on connections: the canvas surface is under this layer, so listen around it.
  useEffect(() => {
    const surface = root.current?.closest<HTMLElement>('.design-canvas-cell')
    if (!surface) return undefined
    const move = (event: PointerEvent) => {
      const hit = hitAt(event.clientX, event.clientY)
      setNear(hit ? `${hit.source}:${hit.index}` : null)
    }
    const down = (event: PointerEvent) => {
      if (event.button !== 0 || (event.target instanceof Element && event.target.closest('.design-proto-handle'))) return
      const hit = hitAt(event.clientX, event.clientY)
      if (!hit) {
        openEditor(null)
        return
      }
      event.preventDefault()
      event.stopPropagation()
      void run('select.set', { ids: [hit.source] }).then(() => openEditor({ node: hit.source, index: hit.index }))
    }
    surface.addEventListener('pointermove', move)
    surface.addEventListener('pointerdown', down, true)
    return () => {
      surface.removeEventListener('pointermove', move)
      surface.removeEventListener('pointerdown', down, true)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run])

  const startDrag = (source: number, x: number, y: number, event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return
    event.preventDefault()
    event.stopPropagation()
    const target = event.currentTarget
    target.setPointerCapture(event.pointerId)
    const box = root.current!.getBoundingClientRect()
    const own = topLevelFrame(tree, source)
    let current = { source, from: [x, y] as Point, to: [x, y] as Point, frame: null as number | null, bounds: null as Rect | null }
    setDrag(current)
    let asking = 0
    const move = (e: PointerEvent) => {
      const v = latest.current.view
      const to: Point = [(e.clientX - box.left) / v.zoom + v.x, (e.clientY - box.top) / v.zoom + v.y]
      current = { ...current, to }
      setDrag(current)
      const ask = (asking += 1)
      void query<Answer>('protoConnections', { zoom: v.zoom, x: Math.round(to[0]), y: Math.round(to[1]) }).then(async (a) => {
        if (ask !== asking) return
        const frame = a.frameAt != null && a.frameAt !== source && a.frameAt !== own ? a.frameAt : null
        const bounds = frame == null ? null : await frameBounds(frame)
        if (ask !== asking) return
        current = { ...current, frame, bounds }
        setDrag(current)
      })
    }
    const frameBounds = async (id: number): Promise<Rect | null> => {
      const node = await query<{ absoluteBounds?: Rect }>('node', { id }).catch(() => null)
      const b = node?.absoluteBounds
      return b ? { x: b.x - inspect.area.x, y: b.y - inspect.area.y, width: b.width, height: b.height } : null
    }
    const up = () => {
      target.removeEventListener('pointermove', move)
      target.removeEventListener('pointerup', up)
      target.removeEventListener('pointercancel', up)
      const dest = current.frame
      setDrag(null)
      if (dest == null) return
      void run<{ index?: number }>('proto.interaction.add', { ids: [source], dest }).then((added) => {
        if (!added) return
        ui.setRightTab('prototype')
        openEditor({ node: source, index: added.index ?? 0 })
      })
    }
    target.addEventListener('pointermove', move)
    target.addEventListener('pointerup', up)
    target.addEventListener('pointercancel', up)
  }

  const r = answer?.handleRadius ?? 7
  return (
    <div ref={root} className="design-proto-canvas" data-testid="design-proto-canvas">
      <svg className="design-proto-noodles" width="100%" height="100%">
        {connections.map((c) => {
          const key = `${c.source}:${c.index}`
          const open = editor?.node === c.source && editor.index === c.index
          const hot = open || near === key
          const bright = c.selected || hot
          const width = hot ? 2.5 : c.selected ? 2 : 1.5
          const [a, b, cc, d] = c.points.map(([x, y]) => toScreen(x, y))
          const arrow = c.arrow.map(([x, y]) => toScreen(x, y).join(',')).join(' ')
          const dest = c.destination
          return (
            <g key={key} opacity={bright ? 1 : 0.55} data-testid={`design-proto-connection-${c.source}-${c.index}`} data-selected={c.selected || undefined} data-open={open || undefined}>
              {(c.selected || open) && dest ? (
                <rect x={(dest.x - view.x) * view.zoom - 1} y={(dest.y - view.y) * view.zoom - 1} width={dest.width * view.zoom + 2} height={dest.height * view.zoom + 2} fill="none" stroke={accent} strokeOpacity={0.7} strokeWidth={1} />
              ) : null}
              <path d={`M${a![0]},${a![1]} C${b![0]},${b![1]} ${cc![0]},${cc![1]} ${d![0]},${d![1]}`} fill="none" stroke={accent} strokeWidth={width} />
              <circle cx={a![0]} cy={a![1]} r={3.5} fill={accent} stroke="#ffffff" strokeWidth={1} />
              <polygon points={arrow} fill={accent} />
            </g>
          )
        })}
        {drag ? (
          <g data-testid="design-proto-drag">
            {drag.bounds ? <rect x={(drag.bounds.x - view.x) * view.zoom - 2} y={(drag.bounds.y - view.y) * view.zoom - 2} width={drag.bounds.width * view.zoom + 4} height={drag.bounds.height * view.zoom + 4} fill="none" stroke={accent} strokeWidth={2} /> : null}
            <DragNoodle from={toScreen(...drag.from)} to={toScreen(...drag.to)} color={accent} />
          </g>
        ) : null}
      </svg>
      {inspect.editing === 'text'
        ? null
        : (answer?.handles ?? []).map((h) => {
            const [x, y] = toScreen(h.x, h.y)
            return (
              <div key={h.id} className="design-proto-handle" style={{ left: x - r, top: y - r, width: 2 * r, height: 2 * r, background: accent }} onPointerDown={(event) => startDrag(h.id, h.x, h.y, event)} data-testid={`design-proto-handle-${h.id}`}>
                <svg width={2 * r} height={2 * r} viewBox={`${-r} ${-r} ${2 * r} ${2 * r}`} aria-hidden="true">
                  <path d="M-3.5 0H3.5M0 -3.5V3.5" stroke="#ffffff" strokeWidth="1.5" />
                </svg>
              </div>
            )
          })}
    </div>
  )
}

/** The connection being dragged out: 2 px, horizontal control offsets max(|dx| / 2, 40), an arrow at the end. */
function DragNoodle({ from, to, color }: { from: Point; to: Point; color: string }) {
  const dx = to[0] - from[0]
  const off = Math.max(Math.abs(dx) / 2, 40) * (dx >= 0 ? 1 : -1)
  const c1: Point = [from[0] + off, from[1]]
  const c2: Point = [to[0] - off, to[1]]
  const tangent = [to[0] - c2[0], to[1] - c2[1]]
  const len = Math.hypot(tangent[0]!, tangent[1]!) || 1
  const ux = tangent[0]! / len
  const uy = tangent[1]! / len
  const arrow = [to, [to[0] - ux * 8 - uy * 4.5, to[1] - uy * 8 + ux * 4.5], [to[0] - ux * 8 + uy * 4.5, to[1] - uy * 8 - ux * 4.5]].map((p) => p.join(',')).join(' ')
  return (
    <>
      <path d={`M${from[0]},${from[1]} C${c1[0]},${c1[1]} ${c2[0]},${c2[1]} ${to[0]},${to[1]}`} fill="none" stroke={color} strokeWidth={2} />
      <polygon points={arrow} fill={color} />
    </>
  )
}
