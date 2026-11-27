/** What the engine answers: `inspect()` (as kit-editor's `info.inspected`) and the canvas chrome. */

export type Rect = { x: number; y: number; width: number; height: number }

/** `DesignDocument::inspect` (bindings/wasm/src/lib.rs). Rects are in document pixels. */
export type DesignInspect = {
  canUndo: boolean
  canRedo: boolean
  name: string
  /** `design` or `board`. */
  kind: string
  /** Where document pixel (0, 0) is on the canvas, and the document's size. */
  area: Rect
  page: number
  pages: Array<{ id: number; name: string; layers: number }>
  /** The page's background, `#RRGGBB`. */
  background: string
  selection: number[]
  selectionBounds: Rect | null
  layers: number
  /** The engine's tool id (`move`, `frame`, `rectangle`, …). */
  tool: string
  editing: 'none' | 'text' | 'vector'
  textEdit: { id: number; caret: number; anchor: number } | null
  vectorEdit: { id: number; tool: string } | null
  /** The first top-level frame, or the page's content. */
  initialArea: Rect | null
  colorProfile: string
  /** The label of the step Undo takes. */
  undo: string | null
  /** A pointer gesture runs on the canvas (a drag, a resize, a marquee). */
  gesture?: boolean
  /** The layer the canvas or the Layers panel hovers. */
  hover?: number | null
}

export const EMPTY_INSPECT: DesignInspect = {
  canUndo: false,
  canRedo: false,
  name: '',
  kind: 'design',
  area: { x: 0, y: 0, width: 1920, height: 1080 },
  page: 0,
  pages: [],
  background: '#f5f5f5',
  selection: [],
  selectionBounds: null,
  layers: 0,
  tool: 'move',
  editing: 'none',
  textEdit: null,
  vectorEdit: null,
  initialArea: null,
  colorProfile: 'srgb',
  undo: null,
}

/** Reads `inspect()` defensively (an older engine may lack fields). */
export function describe(info: { inspected: unknown }): DesignInspect {
  const raw = (info.inspected ?? {}) as Partial<DesignInspect>
  return { ...EMPTY_INSPECT, ...raw, selection: Array.isArray(raw.selection) ? raw.selection : [] }
}

type Point = [number, number]

/** `chrome(zoom)` (bindings/wasm/src/chrome.rs). Positions in document pixels. */
export type DesignChrome = {
  zoom: number
  colors: { accent: string; component: string; guide: string }
  labels: Array<{
    id: number
    name: string
    kind: 'frame' | 'component' | 'componentSet' | 'instance' | 'section'
    x: number
    y: number
    angle: number
    maxWidth: number
    color: string | null
    selected: boolean
    hovered: boolean
    status: { label: string; color: string } | null
    contentsHidden: boolean
    quad: Point[]
  }>
  sizeLabel: { text: string; x: number; y: number; color: string } | null
  autoLayout: {
    id: number
    padding: Array<{ side: number; key: string; value: number; rect: Rect }>
    gaps: Array<{ key: string; value: number; vertical: boolean; rect: Rect }>
    tracks: Array<[Point, Point]>
  } | null
  grids: Array<{ frame: number; color: string; lines: boolean; rects: Rect[] }>
  pixelGrid: boolean
  rulers: { step: number; originX: number; originY: number; selection: Rect | null } | null
  guides: Array<{ owner: number | null; index: number; vertical: boolean; offset: number; a: Point; b: Point; selected: boolean }>
}

/** The right panel's tab: Design, Prototype, or Dev Mode (Inspect, reached from the toolbar). */
export type RightTab = 'design' | 'prototype' | 'inspect'
/** The left panel's tab (the rail). */
export type LeftTab = 'file' | 'assets'
