/**
 * What the Design panel reads (2.3): the selection as `query('selection')` answers it (each
 * layer's `node_json` with the binding's added keys: where it sits, its own variable modes and
 * property bindings), and helpers that take a value common to every selected layer (null when
 * they differ), as upstream's `common()` does.
 */

/** A layer as the engine's `node_json` writes it (keys left out hold their defaults). */
export type NodeJson = {
  id: number
  name: string
  type: string
  x?: number
  y?: number
  width?: number
  height?: number
  rotation?: number
  visible?: boolean
  locked?: boolean
  opacity?: number
  blend?: string
  cornerRadius?: number | number[]
  cornerSmoothing?: number
  layout?: AutoLayout
  sizing?: { horizontal: Sizing; vertical: Sizing }
  absolute?: boolean
  minWidth?: number
  maxWidth?: number
  minHeight?: number
  maxHeight?: number
  gridCell?: { column: number; row: number; columnSpan: number; rowSpan: number; anchored: boolean; alignH: CellAlign | null; alignV: CellAlign | null }
  constraints?: { horizontal: string; vertical: string }
  clip?: boolean
  devStatus?: string
  devChanged?: boolean
  devNote?: string
  sectionContentsHidden?: boolean
  isMask?: boolean
  maskType?: string
  grids?: LayoutGrid[]
  exports?: ExportSetting[]
  boundVariables?: Record<string, string>
  overrides?: string[]
  fills?: Array<Record<string, unknown>>
  arc?: { start: number; sweep: number; inner: number }
  pointCount?: number
  innerRadius?: number
  booleanOperation?: string
  mainComponent?: number
  mainComponentName?: string | null
  componentProperties?: Record<string, unknown>
  componentPropertyDefinitions?: Record<string, PropDef>
  description?: string
  slot?: string
  childCount?: number
  constrainProportions?: boolean
  /** The parent (null at the top of a page). */
  parent?: number | null
  parentAutoLayout?: boolean
  parentGrid?: boolean
  /** Inside an instance (not the instance itself): it takes its place from the main. */
  insideInstance?: boolean
  /** It can carry a handoff status. */
  hasStatus?: boolean
  /** Which component property controls `visible`, `characters` or `mainComponent`, or the slot property a frame is (`slot`). */
  propRefs?: Record<string, string>
  /** Own variable modes by collection (Auto modes are absent). */
  varModes?: Record<string, string>
}

export type Sizing = 'fixed' | 'hug' | 'fill'
export type CellAlign = 'min' | 'center' | 'max'

export type AutoLayout = {
  mode: 'horizontal' | 'vertical' | 'grid'
  wrap: boolean
  padding: [number, number, number, number]
  gap: number
  crossGap: number
  mainAlign: string
  crossAlign: string
  contentAlign: string
  strokesIncluded: boolean
  reverseZ: boolean
  gridColumns: number
  gridRows: number
  columnTracks: Array<{ sizing: Sizing; value: number }>
  rowTracks: Array<{ sizing: Sizing; value: number }>
}

export type Rgba = { r: number; g: number; b: number; a: number }

export type LayoutGrid = {
  kind: 'grid' | 'columns' | 'rows'
  size: number
  count: number
  gutter: number
  offset: number
  align: 'stretch' | 'min' | 'center' | 'max'
  color: Rgba
  visible: boolean
}

export type ExportSize = { by: 'scale' | 'width' | 'height'; value: number }
export type ExportSetting = {
  size: ExportSize
  suffix: string
  format: string
  print?: { bleed: number; cropMarks: boolean; bleedMarks: boolean; cmyk: boolean }
  contentsOnly?: boolean
  includeBoundingBox?: boolean
  svgOutlineText?: boolean
  svgIdAttribute?: boolean
  svgSimplifyStroke?: boolean
  vdCompat?: string
}

export type PropDef = { type: 'boolean' | 'text' | 'instanceSwap' | 'variant' | 'slot'; default?: unknown; preferred?: number[] }

/** `query('selection')` (bindings/wasm/src/query.rs `selection`). */
export type SelectionAnswer = {
  ids: number[]
  count: number
  kind: string | null
  types?: string[]
  nodes: NodeJson[]
  parentAutoLayout?: boolean
  canAlign?: boolean
  booleanOperation?: string | null
  component?: { owningComponent: number | null; owningInstance: number | null; variantSet: number | null; variantOptions: Array<[string, string[]]>; variantProps: Record<string, string> } | null
  page: { id: number; background: string; colorProfile: string; localStyles: number; localVariables: number } | null
  /** Each layer's parent (null at the top of a page). */
  parents?: Array<number | null>
  /** Every selected layer's parent has auto layout. */
  allParentsAutoLayout?: boolean
  /** Any selected layer is inside an instance. */
  insideInstance?: boolean
  /** Every selected layer can carry a handoff status. */
  hasStatus?: boolean
  /** A single component or component set "Add variant" works on. */
  canAddVariant?: boolean
  /** Two or more main components "Combine as variants" takes. */
  canCombineVariants?: boolean
}

export const FRAME_LIKE = new Set(['FRAME', 'COMPONENT', 'COMPONENT_SET', 'INSTANCE'])
export const CORNER_RADIUS = new Set(['RECTANGLE', 'FRAME', 'COMPONENT', 'COMPONENT_SET', 'INSTANCE', 'POLYGON', 'STAR', 'VECTOR', 'SECTION'])
const NO_OWN_CORNERS = new Set(['POLYGON', 'STAR', 'VECTOR'])
/** Kinds that draw through (pass-through blend by default). */
const GROUPING = new Set(['FRAME', 'GROUP', 'COMPONENT', 'COMPONENT_SET', 'INSTANCE', 'SECTION', 'BOOLEAN_OPERATION'])

export const isFrameLike = (n: NodeJson) => FRAME_LIKE.has(n.type)
export const hasCornerRadius = (n: NodeJson) => CORNER_RADIUS.has(n.type)
export const hasIndependentCorners = (n: NodeJson) => hasCornerRadius(n) && !NO_OWN_CORNERS.has(n.type)
export const hasCornerSmoothing = (n: NodeJson) => hasCornerRadius(n) && n.type !== 'VECTOR'

/** A node's own values with their defaults filled in. */
export const opacityOf = (n: NodeJson) => n.opacity ?? 1
export const visibleOf = (n: NodeJson) => n.visible !== false
export const blendOf = (n: NodeJson) => n.blend ?? (GROUPING.has(n.type) ? 'passThrough' : 'normal')
export const radiiOf = (n: NodeJson): [number, number, number, number] => {
  const r = n.cornerRadius
  if (Array.isArray(r)) return [r[0] ?? 0, r[1] ?? 0, r[2] ?? 0, r[3] ?? 0]
  const v = r ?? 0
  return [v, v, v, v]
}
export const uniformRadius = (n: NodeJson) => {
  const [a, b, c, d] = radiiOf(n)
  return a === b && b === c && c === d ? a : null
}

const round2 = (v: number) => Math.round(v * 100) / 100

/** Equal as JSON (values from the engine are plain data). */
function same(a: unknown, b: unknown) {
  return a === b || JSON.stringify(a) === JSON.stringify(b)
}

/** The value of `f` shared by every item, or null when they differ (or there are none). */
export function common<T, I>(items: readonly I[], f: (item: I) => T): T | null {
  if (!items.length) return null
  const first = f(items[0])
  return items.every((item) => same(f(item), first)) ? first : null
}

/** Like `common`, telling a shared `null` from a mixed value. */
export function shared<T, I>(items: readonly I[], f: (item: I) => T): { mixed: boolean; value: T | null } {
  if (!items.length) return { mixed: false, value: null }
  const first = f(items[0])
  return items.every((item) => same(f(item), first)) ? { mixed: false, value: first } : { mixed: true, value: null }
}

/** The layer is an instance or sits in one. */
export const inInstance = (n: NodeJson) => n.type === 'INSTANCE' || n.insideInstance === true

export { round2 }

/** `"multiply"` / `"Multiply"` / `"Color burn"` → one key to compare blend modes by. */
export const blendKey = (blend: string) => blend.replace(/[\s_-]/g, '').toLowerCase()

/** The blend modes as the menu lists them (labels are what `node.set {blend}` takes); null: a separator. */
export const BLEND_MENU: Array<string | null> = [
  'Pass through',
  'Normal',
  null,
  'Darken',
  'Multiply',
  'Plus darker',
  'Color burn',
  null,
  'Lighten',
  'Screen',
  'Plus lighter',
  'Color dodge',
  null,
  'Overlay',
  'Soft light',
  'Hard light',
  null,
  'Difference',
  'Exclusion',
  null,
  'Hue',
  'Saturation',
  'Color',
  'Luminosity',
]

export const blendLabel = (blend: string) => BLEND_MENU.find((label) => label && blendKey(label) === blendKey(blend)) ?? blend

/** `#RRGGBB` (upper case, no alpha) of an engine colour. */
export function hexOf(c: Rgba) {
  const h = (v: number) =>
    Math.round(Math.min(1, Math.max(0, v)) * 255)
      .toString(16)
      .padStart(2, '0')
  return `${h(c.r)}${h(c.g)}${h(c.b)}`.toUpperCase()
}

/** Parses `RGB`, `RRGGBB` or `RRGGBBAA` (with or without `#`); null otherwise. */
export function parseHex(text: string): Rgba | null {
  let s = text.trim().replace(/^#/, '')
  if (/^[0-9a-f]{3}$/i.test(s)) s = s.replace(/./g, (c) => c + c)
  if (!/^([0-9a-f]{6}|[0-9a-f]{8})$/i.test(s)) return null
  const v = (i: number) => parseInt(s.slice(i, i + 2), 16) / 255
  return { r: v(0), g: v(2), b: v(4), a: s.length === 8 ? v(6) : 1 }
}

/** A number as upstream prints it: up to two decimals, no trailing zeros. */
export const fmt = (v: number) => String(round2(v))
