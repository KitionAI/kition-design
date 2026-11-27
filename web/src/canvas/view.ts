/** View helpers: the page's content bounds (Zoom to fit) and document ↔ screen mapping. */
import type { KitCanvasView, RasterRect } from '@kition/sdk/editor'
import type { DesignInspect, Rect } from '../kit/types'

type Query = <R>(name: string, params?: Record<string, unknown>) => Promise<R>
type Child = { absoluteBounds?: Rect }

/**
 * The current page's content (its top-level layers' bounds) in document pixels, or null on an
 * empty page. (The engine's `inspect().area` is the content grown to whole chunks; see the engine
 * requests in web/README.md.)
 */
export async function contentBounds(query: Query, inspect: DesignInspect): Promise<Rect | null> {
  const document = await query<{ currentPage?: number; pages?: Array<{ id: number; children?: Child[] }> }>('document', { depth: 1 }).catch(() => null)
  const page = document?.pages?.find((item) => item.id === (document.currentPage ?? inspect.page)) ?? document?.pages?.[0]
  let box: { x0: number; y0: number; x1: number; y1: number } | null = null
  for (const child of page?.children ?? []) {
    const b = child.absoluteBounds
    if (!b || ![b.x, b.y, b.width, b.height].every(Number.isFinite)) continue
    const next = { x0: b.x - inspect.area.x, y0: b.y - inspect.area.y, x1: b.x + b.width - inspect.area.x, y1: b.y + b.height - inspect.area.y }
    box = box ? { x0: Math.min(box.x0, next.x0), y0: Math.min(box.y0, next.y0), x1: Math.max(box.x1, next.x1), y1: Math.max(box.y1, next.y1) } : next
  }
  return box ? { x: box.x0, y: box.y0, width: box.x1 - box.x0, height: box.y1 - box.y0 } : null
}

/** A document point on the canvas, in CSS pixels from the canvas's top-left. */
export function toScreen(view: KitCanvasView, x: number, y: number) {
  return { x: (x - view.x) * view.zoom, y: (y - view.y) * view.zoom }
}

/** A `{ x, y, width, height }` rectangle as kit-editor's canvas takes areas. */
export function raster(rect: Rect): RasterRect {
  return { x0: rect.x, y0: rect.y, x1: rect.x + rect.width, y1: rect.y + rect.height }
}

/**
 * The view moved onto whole device pixels, or null when it is on them already or cannot be. The
 * canvas copies its frame to the screen pixel for pixel, without smoothing, whenever the zoom
 * times the pixel ratio is a whole number (100 %, 200 % on a 1× screen); placed half a pixel off
 * (a centred 100 % on a canvas of odd height), the GPU's nearest-pixel copy drops or doubles rows
 * and columns: round edges turn to steps, thin lines break up (M5). Off by less than half a device
 * pixel, which nobody sees.
 */
export function onDevicePixels(view: { x: number; y: number; zoom: number }, dpr: number): { x: number; y: number } | null {
  const scale = view.zoom * dpr
  const whole = Math.round(scale)
  if (!(whole >= 1) || Math.abs(scale - whole) > 1e-9 || ![view.x, view.y].every(Number.isFinite)) return null
  const x = Math.round(view.x * whole) / whole
  const y = Math.round(view.y * whole) / whole
  return Math.abs(x - view.x) > 1e-9 || Math.abs(y - view.y) > 1e-9 ? { x, y } : null
}
