/**
 * The layers tree as the panel lists it (`layers.rs` `listed_rows`): the engine's nested
 * `query('layers')` answer flattened into rows, top of the stack first, with what the panel needs
 * for icons and fonts. Drops come from the engine (`query('layers.dropInfo')`, `DropInfo`).
 */
import type { IconName } from '../icons/Icon'

/** One row of `query('layers')` (bindings/wasm/src/query.rs). */
export type EngineRow = {
  id: number
  name: string
  type: string
  icon: string
  visible: boolean
  locked: boolean
  lockedAbove: boolean
  dimmed: boolean
  component: boolean
  mask: boolean
  selected: boolean
  hasChildren: boolean
  expanded: boolean
  children?: EngineRow[]
}

export type LayersAnswer = { page: number; rows: EngineRow[]; truncated?: boolean }

export type Row = {
  id: number
  name: string
  type: string
  icon: IconName
  depth: number
  hasKids: boolean
  /** Not shown on the canvas (hidden, or under a hidden layer or a section hiding its contents). */
  dimmed: boolean
  visible: boolean
  locked: boolean
  lockedAbove: boolean
  component: boolean
  /** A top-level frame, component, instance or section: its name is semibold. */
  bold: boolean
  /** The parent's id (the page for top-level layers). */
  parent: number
  /** Ancestors, nearest first (the page excluded). */
  ancestors: number[]
}

export type Tree = {
  page: number
  /** Every layer the answer lists, by id. */
  byId: Map<number, Row>
  /** Children ids per parent (the page included), in the engine's order (bottom first). */
  children: Map<number, number[]>
}

const ICONS: Record<string, IconName> = {
  page: 'Page',
  slot: 'Slot',
  layoutVertical: 'LayoutV',
  layoutGrid: 'LayoutGrid',
  layoutHorizontal: 'LayoutH',
  frame: 'Frame',
  group: 'Group',
  section: 'Section',
  image: 'Image',
  rectangle: 'Rectangle',
  ellipse: 'Ellipse',
  polygon: 'Polygon',
  star: 'Star',
  arrow: 'Arrow',
  line: 'Line',
  vector: 'Vector',
  text: 'Text',
  union: 'Union',
  subtract: 'Subtract',
  intersect: 'Intersect',
  exclude: 'Exclude',
  component: 'Component',
  componentSet: 'ComponentSet',
  instance: 'Instance',
  slice: 'Slice',
}

export const iconOf = (icon: string): IconName => ICONS[icon] ?? 'Frame'

/** The kind label upstream shows for a layer without a name (`NodeKind::label`). */
const LABELS: Record<string, string> = {
  PAGE: 'Page',
  FRAME: 'Frame',
  GROUP: 'Group',
  SECTION: 'Section',
  RECTANGLE: 'Rectangle',
  ELLIPSE: 'Ellipse',
  POLYGON: 'Polygon',
  STAR: 'Star',
  LINE: 'Line',
  VECTOR: 'Vector',
  TEXT: 'Text',
  COMPONENT: 'Component',
  COMPONENT_SET: 'Component set',
  INSTANCE: 'Instance',
  SLICE: 'Slice',
}

const OPS: Record<string, string> = { union: 'Union', subtract: 'Subtract', intersect: 'Intersect', exclude: 'Exclude' }

export function kindLabel(type: string, icon: string): string {
  if (type === 'BOOLEAN_OPERATION') return OPS[icon] ?? 'Union'
  return LABELS[type] ?? 'Layer'
}

const FRAME_LIKE = new Set(['FRAME', 'COMPONENT', 'COMPONENT_SET', 'INSTANCE'])

/** Where `node.reparent` puts the dragged layers for one drop zone (`layers.dropInfo`). */
export type DropSpot = { parent: number; index?: number }

/** One row of `query('layers.dropInfo', { ids, moving })` (bindings/wasm/src/lists.rs `drop_info`). */
export type DropRow = {
  id: number
  parent: number
  /** Among its parent's children, bottom of the stack first. */
  index: number
  /** Takes a drop into it (the middle half of the row). */
  container: boolean
  /** A drop above or below it is refused (its parent's structure is locked: inside an instance). */
  structureLocked: boolean
  childrenLocked: boolean
  /** One of the dragged layers, or inside one. */
  blocked?: boolean
  drop?: { above: DropSpot | null; below: DropSpot | null; into: DropSpot | null }
}

export type DropInfo = { page: number; rows: DropRow[]; truncated: boolean }

/** Reads the engine's answer into a tree. */
export function buildTree(answer: LayersAnswer): Tree {
  const byId = new Map<number, Row>()
  const children = new Map<number, number[]>()
  const walk = (list: EngineRow[], parent: number, depth: number, ancestors: number[]) => {
    // The answer lists the top of the stack first; the engine's order is bottom first.
    children.set(
      parent,
      list.map((row) => row.id).reverse(),
    )
    list.forEach((row) => {
      const shown = byId.get(row.id)
      if (shown) return
      byId.set(row.id, {
        id: row.id,
        name: row.name,
        type: row.type,
        icon: iconOf(row.icon),
        depth,
        hasKids: row.hasChildren || row.type === 'BOOLEAN_OPERATION',
        dimmed: row.dimmed,
        visible: row.visible,
        locked: row.locked,
        lockedAbove: row.lockedAbove,
        component: row.component,
        bold: depth === 0 && (FRAME_LIKE.has(row.type) || row.type === 'SECTION'),
        parent,
        ancestors,
      })
      if (row.children) walk(row.children, row.id, depth + 1, [row.id, ...ancestors])
    })
  }
  walk(answer.rows, answer.page, 0, [])
  return { page: answer.page, byId, children }
}

/** The rows to list: the tree with the rows in `expanded` open. */
export function listRows(tree: Tree, expanded: ReadonlySet<number>): Row[] {
  const out: Row[] = []
  const walk = (parent: number) => {
    const kids = tree.children.get(parent)
    if (!kids) return
    for (let i = kids.length - 1; i >= 0; i -= 1) {
      const row = tree.byId.get(kids[i])
      if (!row) continue
      out.push(row)
      if (expanded.has(row.id) && row.depth < 64) walk(row.id)
    }
  }
  walk(tree.page)
  return out
}

/** The layers whose name (or kind label) contains `query` (lowercase), as a flat list. */
export function matchingRows(tree: Tree, query: string): Row[] {
  const out: Row[] = []
  const walk = (parent: number) => {
    const kids = tree.children.get(parent)
    if (!kids) return
    for (let i = kids.length - 1; i >= 0 && out.length < 5000; i -= 1) {
      const row = tree.byId.get(kids[i])
      if (!row) continue
      const name = row.name || kindLabel(row.type, '')
      if (name.toLowerCase().includes(query)) out.push({ ...row, depth: 0, hasKids: false })
      walk(row.id)
    }
  }
  walk(tree.page)
  return out
}
