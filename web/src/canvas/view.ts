/** View helpers: the page's content bounds (Zoom to fit) and document ↔ screen mapping. */
import type { KitCanvasView, RasterRect } from '@kition/kit-editor'
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
