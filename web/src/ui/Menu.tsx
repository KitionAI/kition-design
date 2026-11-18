/**
 * Menus (2.7 §0.1 and 2.1 §7): `tone="dark"` is the main menu, context menus and toolbar flyouts
 * (#1E1E1E in both themes, rows 28 tall, label at +28, check at +12, green hover with white text);
 * `tone="light"` is a dropdown (the popup frame, rows 26 tall at a 32 pitch). Submenus open after
 * 150 ms of hover (at once by click or key), with a safe triangle while the pointer aims at an
 * open submenu. Keys: arrows, Home, End, Enter, Space, Escape.
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { Icon, type IconName } from '../icons/Icon'
import { anchorRect, place, type AnchorRect, type Placement } from './position'

export type MenuItem = {
  type?: 'item'
  label: string
  /** Shown right-aligned, already in display form (`prettyShortcut`). */
  shortcut?: string
  /** A check mark (dark: drawn at +12; light: the label is prefixed "✓ " or four spaces). */
  checked?: boolean
  disabled?: boolean
  /** A flyout row's 16 px icon at +32. */
  icon?: IconName
  /** Extra left inset of the label (Select layer ▸ indents 12 px per depth). */
  indent?: number
  onSelect: () => void
  testId?: string
  /** Art drawn at the right of the row, before the shortcut (stroke caps, width profiles). */
  art?: ReactNode
  /** The pointer or the arrow keys are on the row (a live preview). */
  onHover?: () => void
}

export type MenuEntry =
  | MenuItem
  | { type: 'separator' }
  | { type: 'submenu'; label: string; items: MenuEntry[]; disabled?: boolean; testId?: string }
  /** The main menu's "Actions…" search row. */
  | { type: 'search'; label: string; shortcut?: string; onSelect: () => void; testId?: string }
  /** Any content (a note row), not focusable. */
  | { type: 'custom'; render: () => ReactNode }

export type MenuTone = 'dark' | 'light'

export type MenuProps = {
  tone: MenuTone
  /** `flyout`: the toolbar's tool menus (rows 32 tall, an icon at +32, label at +48); `buttons`:
   * egui's plain buttons in a context popup (rows 24 tall at a 30 pitch, 11 px, 2 px padding, as
   * wide as the longest label). */
  variant?: 'menu' | 'flyout' | 'buttons'
  items: MenuEntry[]
  anchor: Element | AnchorRect | null
  placement?: Placement
  gap?: number
  /** Outer width at least (the anchor's width for dropdowns, 256 for dark menus, 220 for flyouts). */
  minWidth?: number
  /** The gap between a row and its submenu (12 for the main menu's top level, 4 below). */
  submenuGap?: number
  /** A shadow under dark menus (main and context menus; flyouts have none). */
  shadow?: boolean
  onClose: () => void
  label?: string
  testId?: string
  /** The panel scrolls past this height. */
  maxHeight?: number
}

const HOVER_DELAY = 150
const AIM_REST = 300

const focusable = (entry: MenuEntry) => entry.type !== 'separator' && entry.type !== 'custom' && !('disabled' in entry && entry.disabled)

type Tree = { id: string; close: () => void; panels: Set<HTMLElement> }

/** A menu with its submenus; closes on a press outside all of its panels, Escape at the top, or a chosen item. */
export function Menu(props: MenuProps) {
  const id = useMemo(() => `design-menu-${Math.random().toString(36).slice(2)}`, [])
  const latest = useRef(props.onClose)
  latest.current = props.onClose
  const tree = useMemo<Tree>(() => ({ id, close: () => latest.current(), panels: new Set() }), [id])
  useEffect(() => {
    const anchor = props.anchor instanceof Element ? props.anchor : null
    const down = (event: PointerEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      for (const panel of tree.panels) if (panel.contains(target)) return
      if (anchor?.contains(target)) return
      // A layer opened from inside this menu (a dialog) keeps it.
      const other = target instanceof Element ? target.closest('.design-layer') : null
      if (other && [...tree.panels].some((panel) => panel.compareDocumentPosition(other) & Node.DOCUMENT_POSITION_FOLLOWING)) return
      tree.close()
    }
    document.addEventListener('pointerdown', down, true)
    return () => document.removeEventListener('pointerdown', down, true)
  }, [props.anchor, tree])
  return <MenuPanel {...props} tree={tree} depth={0} />
}

type PanelProps = MenuProps & { tree: Tree; depth: number; onBack?: () => void; autoFocus?: boolean }

function MenuPanel({ tone, variant = 'menu', items, anchor, placement = 'below-start', gap = 4, minWidth, submenuGap = 4, shadow = false, tree, depth, onBack, autoFocus = true, label, testId, maxHeight }: PanelProps) {
  const panel = useRef<HTMLDivElement>(null)
  const [at, setAt] = useState<{ left: number; top: number } | null>(null)
  const [active, setActive] = useState(-1)
  const [open, setOpen] = useState<{ index: number; rect: AnchorRect; focus: boolean } | null>(null)
  const timer = useRef<number | undefined>(undefined)
  const rows = useRef<Array<HTMLElement | null>>([])
  // The safe triangle: the pointer's last position, and when it last moved.
  const aim = useRef<{ x: number; y: number; t: number } | null>(null)

  useLayoutEffect(() => {
    const el = panel.current
    const rect = anchorRect(anchor)
    if (!el || !rect) return
    tree.panels.add(el)
    setAt(place(rect, { width: el.offsetWidth, height: el.offsetHeight }, placement, gap))
    return () => {
      tree.panels.delete(el)
    }
  }, [anchor, gap, placement, tree, items])

  useEffect(() => {
    if (autoFocus) panel.current?.focus({ preventScroll: true })
  }, [autoFocus])

  useEffect(() => () => window.clearTimeout(timer.current), [])

  // A row the pointer or the keys are on previews (`onHover`).
  useEffect(() => {
    const entry = items[active]
    if (entry && (entry.type === undefined || entry.type === 'item')) entry.onHover?.()
    // Only a change of row previews.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active])

  const openSub = useCallback((index: number, focus: boolean) => {
    const row = rows.current[index]
    if (!row) return
    const r = row.getBoundingClientRect()
    // The submenu's first row lines up with this row: its panel starts 8 px (its padding) higher.
    setOpen({ index, rect: { left: r.left, top: r.top - 8, right: r.right + submenuGap, bottom: r.top - 8 }, focus })
  }, [submenuGap])

  const choose = (entry: MenuEntry, index: number) => {
    if (!focusable(entry)) return
    if (entry.type === 'submenu') {
      window.clearTimeout(timer.current)
      openSub(index, true)
      return
    }
    if (entry.type === 'separator' || entry.type === 'custom') return
    tree.close()
    entry.onSelect()
  }

  /** Whether the pointer, moving from its last point to (`x`, `y`), heads into the open submenu. */
  const aiming = (x: number, y: number) => {
    const last = aim.current
    const sub = open ? document.querySelector<HTMLElement>(`[data-menu-tree='${tree.id}'][data-depth='${depth + 1}']`) : null
    if (!last || !sub || performance.now() - last.t > AIM_REST) return false
    const r = sub.getBoundingClientRect()
    const edge = r.left >= last.x ? r.left : r.right
    const dx = edge - last.x
    if (Math.abs(dx) < 1) return false
    const t = (x - last.x) / dx
    if (t <= 0) return false
    const top = last.y + t * (r.top - 8 - last.y)
    const bottom = last.y + t * (r.bottom + 8 - last.y)
    return y >= top && y <= bottom
  }

  const enter = (entry: MenuEntry, index: number, event: React.PointerEvent) => {
    if (open && open.index !== index && aiming(event.clientX, event.clientY)) {
      // Rows the pointer crosses on its way to the submenu do not take over until it rests.
      window.clearTimeout(timer.current)
      timer.current = window.setTimeout(() => {
        setActive(index)
        setOpen(null)
        if (entry.type === 'submenu' && !entry.disabled) openSub(index, false)
      }, AIM_REST)
      return
    }
    window.clearTimeout(timer.current)
    if (!focusable(entry)) return
    setActive(index)
    if (open && open.index !== index) setOpen(null)
    if (entry.type === 'submenu' && open?.index !== index) timer.current = window.setTimeout(() => openSub(index, false), HOVER_DELAY)
  }

  const step = (from: number, by: number) => {
    const n = items.length
    for (let i = 1; i <= n; i += 1) {
      const next = (((from + by * i) % n) + n) % n
      if (focusable(items[next])) return next
    }
    return -1
  }

  const keyDown = (event: React.KeyboardEvent) => {
    const entry = items[active]
    switch (event.key) {
      case 'ArrowDown':
        setActive(step(active, 1))
        break
      case 'ArrowUp':
        setActive(step(active < 0 ? 0 : active, -1))
        break
      case 'Home':
        setActive(step(-1, 1))
        break
      case 'End':
        setActive(step(0, -1))
        break
      case 'ArrowRight':
        if (entry?.type === 'submenu' && !entry.disabled) openSub(active, true)
        else return
        break
      case 'ArrowLeft':
        if (onBack) onBack()
        else return
        break
      case 'Enter':
      case ' ':
        if (entry) choose(entry, active)
        break
      case 'Escape':
        if (onBack) onBack()
        else tree.close()
        break
      case 'Tab':
        tree.close()
        return
      default:
        return
    }
    event.preventDefault()
    event.stopPropagation()
  }

  const flyout = variant === 'flyout'
  const sub = open ? items[open.index] : null
  return (
    <>
      {createPortal(
        <div
          ref={panel}
          className="design-layer design-menu"
          data-tone={tone}
          data-variant={variant}
          data-shadow={shadow || undefined}
          data-menu-tree={tree.id}
          data-depth={depth}
          role="menu"
          aria-label={label}
          tabIndex={-1}
          data-testid={testId}
          style={{ ...(at ?? { left: -9999, top: -9999 }), minWidth, ...(maxHeight ? { maxHeight, overflowY: 'auto' as const } : null) }}
          onKeyDown={keyDown}
          onPointerMove={(event) => {
            aim.current = { x: event.clientX, y: event.clientY, t: performance.now() }
          }}
          onPointerLeave={() => {
            // Leaving every menu keeps the last submenu open; the hover highlight follows it.
            window.clearTimeout(timer.current)
            setActive(open ? open.index : -1)
          }}
          onContextMenu={(event) => event.preventDefault()}
        >
          {items.map((entry, index) => {
            if (entry.type === 'separator') {
              const previous = items[index - 1]
              if (index === 0 || index === items.length - 1 || previous?.type === 'separator') return null
              return <div key={index} className="design-menu-separator" role="separator" />
            }
            if (entry.type === 'custom') return <div key={index}>{entry.render()}</div>
            if (entry.type === 'search') {
              return (
                <div key={index} ref={(el) => void (rows.current[index] = el)} className="design-menu-search" role="menuitem" data-active={active === index || undefined} data-testid={entry.testId} onPointerEnter={(event) => enter(entry, index, event)} onClick={() => choose(entry, index)}>
                  <Icon name="Search" size={14} />
                  <span>{entry.label}</span>
                  {entry.shortcut ? <span style={{ flex: 'none' }}>{entry.shortcut}</span> : null}
                </div>
              )
            }
            const disabled = 'disabled' in entry && entry.disabled === true
            const highlighted = !disabled && (active === index || open?.index === index)
            const isSub = entry.type === 'submenu'
            const item = isSub ? null : (entry as MenuItem)
            const lightCheck = tone === 'light' && item?.checked !== undefined
            return (
              <div
                key={index}
                ref={(el) => void (rows.current[index] = el)}
                className="design-menu-row"
                role={item?.checked !== undefined ? 'menuitemcheckbox' : 'menuitem'}
                aria-checked={item?.checked}
                aria-disabled={disabled || undefined}
                aria-haspopup={isSub ? 'menu' : undefined}
                aria-expanded={isSub ? open?.index === index : undefined}
                data-active={highlighted || undefined}
                data-testid={entry.testId}
                style={item?.indent ? { paddingLeft: (tone === 'dark' ? 28 : 8) + item.indent } : undefined}
                onPointerEnter={(event) => enter(entry, index, event)}
                onClick={() => choose(entry, index)}
              >
                {tone === 'dark' && item?.checked ? (
                  <span className="design-menu-check">
                    <Icon name="Check" size={12} />
                  </span>
                ) : null}
                {flyout && item?.icon ? (
                  <span className="design-menu-icon">
                    <Icon name={item.icon} size={16} />
                  </span>
                ) : null}
                <span className="design-menu-label" style={lightCheck ? { whiteSpace: 'pre' } : undefined}>
                  {lightCheck ? (item?.checked ? '✓ ' : <span className="design-menu-check-space" />) : ''}
                  {entry.label}
                </span>
                {item?.art ? <span className="design-menu-art">{item.art}</span> : null}
                {item?.shortcut ? <span className="design-menu-keys">{item.shortcut}</span> : null}
                {isSub ? (
                  <span className="design-menu-sub">
                    <Icon name="ChevronRight" size={12} />
                  </span>
                ) : null}
              </div>
            )
          })}
        </div>,
        document.body,
      )}
      {open && sub?.type === 'submenu' ? (
        <MenuPanel
          tone={tone}
          items={sub.items}
          anchor={open.rect}
          placement="right-start"
          gap={0}
          minWidth={minWidth}
          submenuGap={4}
          shadow={shadow}
          tree={tree}
          depth={depth + 1}
          autoFocus={open.focus}
          onBack={() => {
            setOpen(null)
            panel.current?.focus({ preventScroll: true })
          }}
          onClose={tree.close}
          label={sub.label}
          testId={sub.testId ? `${sub.testId}-menu` : undefined}
        />
      ) : null}
    </>
  )
}
