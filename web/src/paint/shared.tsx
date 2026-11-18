/**
 * Pieces the paint sections share: the document's styles and variables, the style chip (a linked
 * style, with Detach), the Styles menu (apply a local style of the slot), a bound variable's chip,
 * a context menu at the pointer, and list rows that drag to reorder.
 */
import { useRef, useState, type MouseEvent, type ReactNode } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { IconButton, Menu, Row, type MenuEntry } from '../ui'
import type { AnchorRect } from '../ui/position'
import { dispatchUiRequests } from '../kit/requests'
import { readPaints } from './paint'
import { PaintSwatch } from './Swatch'

export type StyleJson = { id: string; name: string; description?: string; type: 'paint' | 'text' | 'effect' | 'grid'; paints?: unknown; style?: Record<string, unknown>; effects?: unknown }
export type VariableJson = { id: string; name: string; resolvedType: string; collection: string }

export function useStyles(): StyleJson[] {
  const answer = useQuery<StyleJson[]>('styles')
  return Array.isArray(answer) ? answer : []
}

export function useVariables(): VariableJson[] {
  const answer = useQuery<{ variables: VariableJson[] }>('variables')
  return answer?.variables ?? []
}

/** The style slot a section links (`node.styles.<slot>`). */
export type StyleSlot = 'fill' | 'stroke' | 'effect' | 'text'

const SLOT_TYPE: Record<StyleSlot, StyleJson['type']> = { fill: 'paint', stroke: 'paint', effect: 'effect', text: 'text' }

/** The style every selected layer links in `slot`: its id, `null` for none, `'mixed'` when they differ. */
export function linkedStyle(nodes: Array<Record<string, unknown>>, slot: StyleSlot): string | null | 'mixed' {
  const ids = nodes.map((n) => ((n.styles as Record<string, string | null> | undefined)?.[slot] ?? null))
  if (!ids.length) return null
  return ids.every((id) => id === ids[0]) ? ids[0] : 'mixed'
}

/** What the engine's clipboard holds for the paste items (`query('clipboard.paintState')`). */
export type ClipboardState = { fill: boolean; stroke: boolean; effect: boolean; properties: boolean; layers: boolean }
let clipboard: ClipboardState = { fill: false, stroke: false, effect: false, properties: false, layers: false }

/** The clipboard as last read (when a paint menu opened). */
export function clipboardState(): ClipboardState {
  return clipboard
}

/**
 * A context menu at the pointer: `open(event)` from `onContextMenu`, render `menu` somewhere.
 * The engine's clipboard is read first (a copy moves no revision, so a cached query would not see
 * it): `items` can offer Paste from `clipboardState()`.
 */
export function useContextMenu(items: () => MenuEntry[], testId?: string) {
  const { invoke } = useDesignKit()
  const [at, setAt] = useState<AnchorRect | null>(null)
  const open = (event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    const where = { left: event.clientX, top: event.clientY, right: event.clientX, bottom: event.clientY }
    void invoke<string>('query', 'clipboard.paintState', '{}')
      .then((text) => {
        clipboard = { ...clipboard, ...(JSON.parse(text) as Partial<ClipboardState>) }
      })
      .catch(() => undefined)
      .then(() => {
        if (items().length) setAt(where)
      })
  }
  const menu = at ? <Menu tone="light" items={items()} anchor={at} placement="below-start" gap={0} minWidth={48} onClose={() => setAt(null)} testId={testId} /> : null
  return { open, menu }
}

/**
 * The style chip (`style_picker::chip`): the linked style's name in a 32 px row (`Icon::Styles`
 * at +12, name at +26, hover fill), with Detach at the right.
 */
export function StyleChip({ ids, slot, nodes, testId }: { ids: number[]; slot: StyleSlot; nodes: Array<Record<string, unknown>>; testId?: string }) {
  const { t, run } = useDesignKit()
  const styles = useStyles()
  const linked = linkedStyle(nodes, slot)
  if (linked === null) return null
  const style = linked === 'mixed' ? null : styles.find((s) => s.id === linked)
  const name = linked === 'mixed' ? t('Mixed') : (style?.name ?? t('Missing style'))
  const paints = style && (slot === 'fill' || slot === 'stroke') ? readPaints(style.paints) : []
  return (
    <Row testId={testId}>
      <div className="design-style-chip" data-tip={t('Change style')} data-mixed={linked === 'mixed' || undefined} onClick={(event) => openStyles(ids, slot, event)}>
        {paints[0] ? <PaintSwatch paint={paints[0]} size={14} /> : <Icon name={slot === 'text' ? 'Text' : 'Styles'} size={14} />}
        <span>{name}</span>
      </div>
      <IconButton icon="Detach" label={t('Detach style')} onClick={() => void run('style.detach', { ids, which: slot })} testId={testId ? `${testId}-detach` : undefined} />
    </Row>
  )
}

/**
 * The Styles header button: the style picker (the left panel's package) opens for the slot of the
 * layers (`stylePicker.open` with `{ ids, slot, y }`, `y` the pointer's, as upstream places it).
 */
export function openStyles(ids: number[], slot: StyleSlot, at?: { clientY: number } | string) {
  const y = typeof at === 'string' ? (document.querySelector(`[data-testid="${at}"]`)?.getBoundingClientRect().top ?? 200) + 12 : (at?.clientY ?? 200)
  dispatchUiRequests([{ kind: 'stylePicker.open', data: { ids, slot, y } }])
}

/**
 * A bound variable's chip (`bound_field` in paint rows): the variable's last path segment in a
 * `hover` chip with a 1 px `border` (the full name on hover), and Detach right of it.
 */
export function BoundChip({ name, onDetach, testId }: { name: string; onDetach: () => void; testId?: string }) {
  const { t } = useDesignKit()
  const short = name.split('/').pop() ?? name
  return (
    <span className="design-bound" data-testid={testId}>
      <span className="design-bound-chip" data-tip={name}>
        {short}
      </span>
      <button type="button" className="design-bound-detach" aria-label={t('Detach variable')} data-tip={t('Detach variable')} onClick={onDetach} data-testid={testId ? `${testId}-detach` : undefined}>
        <Icon name="Detach" size={12} />
      </button>
    </span>
  )
}

/**
 * Rows that drag to reorder (`ListDrag`): a grip shows left of a hovered row; dropping between rows
 * moves the item (document indices, bottom first, while the rows show the top first).
 */
export function useListDrag(count: number, onMove: (from: number, to: number) => void) {
  const [drag, setDrag] = useState<{ from: number; over: number } | null>(null)
  const rows = useRef(new Map<number, HTMLElement>())
  const start = (index: number, event: React.PointerEvent) => {
    if (event.button !== 0) return
    event.preventDefault()
    const startY = event.clientY
    let moved = false
    let over = index
    const move = (e: PointerEvent) => {
      if (!moved && Math.abs(e.clientY - startY) < 3) return
      moved = true
      // The shown row under the pointer (rows show the last item first).
      let best = index
      let bestDistance = Infinity
      for (const [i, el] of rows.current) {
        const r = el.getBoundingClientRect()
        const d = Math.abs(e.clientY - (r.top + r.height / 2))
        if (d < bestDistance) {
          bestDistance = d
          best = i
        }
      }
      over = best
      setDrag({ from: index, over })
    }
    const up = () => {
      document.removeEventListener('pointermove', move, true)
      document.removeEventListener('pointerup', up, true)
      setDrag(null)
      if (moved && over !== index && over >= 0 && over < count) onMove(index, over)
    }
    document.addEventListener('pointermove', move, true)
    document.addEventListener('pointerup', up, true)
  }
  /** Props for the row of item `index`. */
  const row = (index: number) => ({
    ref: (el: HTMLElement | null) => {
      if (el) rows.current.set(index, el)
      else rows.current.delete(index)
    },
    'data-drag-over': drag && drag.over === index && drag.from !== index ? (drag.from > index ? 'below' : 'above') : undefined,
    'data-dragging': drag?.from === index || undefined,
  })
  const grip = (index: number, label: string): ReactNode => (
    <span className="design-list-grip" aria-hidden="true" data-tip={label} onPointerDown={(event) => start(index, event)}>
      <Icon name="DragHandle" size={10} />
    </span>
  )
  return { row, grip }
}
