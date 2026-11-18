/**
 * The Layers section (2.2, `layers.rs` `layers`): the header with Search layers, the filter row,
 * and the virtualised tree (rows 32 tall at a 38 pitch): icons per kind, disclosure (Alt: the whole
 * subtree), selection with Shift and Cmd ranges, rename by double-click (Tab goes on to the next
 * row), the eye and padlock (a press dragged across rows sweeps them, one step), drag to reorder
 * and reparent (25/50/75 % zones, hold to open, auto-scroll, drop on a page row; what takes a drop
 * and where it lands come from `query('layers.dropInfo')`), the context menu, keyboard navigation,
 * and the hover shared with the canvas (a hovered row outlines its layer through `setHover`; the
 * canvas's hover, `inspect().hover`, outlines its row).
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { useUiRequest } from '../kit/requests'
import { Icon } from '../icons/Icon'
import { ScrollBar } from '../ui/ScrollBar'
import { openContextMenu } from '../menus/contextMenuStore'
import { devRefuses } from './dev'
import { RenameInput, SearchRow } from './fields'
import { DragGhost } from './Pages'
import { leftState, setLeft, toggleExpanded, useLeft } from './store'
import { buildTree, kindLabel, listRows, matchingRows, type DropInfo, type DropRow, type DropSpot, type LayersAnswer, type Row, type Tree } from './tree'

const ROW = 32
const PITCH = 38
const EDGE = 28
const HOLD_MS = 600

type Target = { index: number; zone: -1 | 0 | 1; refused: boolean; spot: DropSpot | null } | null
type Sweep = { lock: boolean; on: boolean; ids: number[] }

/** x of a row's contents (the disclosure triangle) at `depth`, from the row's left edge. */
const rowX0 = (depth: number) => 8 + 4 + depth * 16

/**
 * The layer the canvas hovers (`inspect().hover`), read again after every tool answer (a hover
 * over the canvas answers new overlays but no new revision, so kit-editor's `inspect` keeps the
 * old one).
 */
function useCanvasHover(): number | null {
  const { invoke, tools, revision, doc } = useDesignKit()
  const [hover, setHover] = useState<number | null>(null)
  const asking = useRef({ busy: false, again: false })
  const ask = useCallback(() => {
    const state = asking.current
    if (state.busy) {
      state.again = true
      return
    }
    state.busy = true
    void invoke<string>('inspect')
      .then((text) => {
        const value = (JSON.parse(text) as { hover?: unknown }).hover
        setHover(typeof value === 'number' ? value : null)
      })
      .catch(() => undefined)
      .finally(() => {
        state.busy = false
        if (state.again) {
          state.again = false
          ask()
        }
      })
  }, [invoke])
  useEffect(() => {
    if (doc) ask()
  }, [ask, doc, revision, tools.decorate])
  return hover
}

export function Layers() {
  const designKit = useDesignKit()
  const { t, run, inspect } = designKit
  const left = useLeft()
  const filter = left.layerFilter.trim().toLowerCase()
  const filtering = filter !== ''
  const expandedList = useMemo(() => [...left.expanded].sort((a, b) => a - b), [left.expanded])
  const answer = useQuery<LayersAnswer>('layers', filtering ? { all: true } : { expanded: expandedList })
  // The last tree stays while the next answer is on its way.
  const last = useRef<{ answer: LayersAnswer; tree: Tree } | null>(null)
  if (answer && last.current?.answer !== answer) last.current = { answer, tree: buildTree(answer) }
  const tree = last.current?.answer.page === inspect.page || !answer ? last.current?.tree ?? null : null
  const rows = useMemo(() => (tree ? (filtering ? matchingRows(tree, filter) : listRows(tree, left.expanded)) : []), [tree, filtering, filter, left.expanded])
  // The listed rows now, for drags and sweeps that outlive a render (hold-to-open adds rows).
  const rowsNow = useRef(rows)
  rowsNow.current = rows
  const selection = inspect.selection
  const selected = useMemo(() => new Set(selection), [selection])
  const dev = designKit.ui.rightTab === 'inspect'
  const canvasHover = useCanvasHover()
  // The row under the pointer is the shared hover: the canvas outlines its layer.
  const hovering = useRef<number | null>(null)
  const shareHover = useCallback(
    (id: number | null) => {
      if (hovering.current === id) return
      hovering.current = id
      void designKit.invoke<boolean>('setHover', id == null ? '' : String(id)).then(
        (changed) => {
          if (changed) designKit.tools.refresh()
        },
        () => undefined,
      )
    },
    [designKit],
  )

  const scroller = useRef<HTMLDivElement>(null)
  const [view, setView] = useState({ top: 0, height: 400 })
  useLayoutEffect(() => {
    const el = scroller.current
    if (!el) return undefined
    const measure = () => setView({ top: el.scrollTop, height: el.clientHeight })
    const observer = new ResizeObserver(measure)
    observer.observe(el)
    measure()
    return () => observer.disconnect()
  }, [])

  // Reveal a new selection once: open its ancestors and scroll its row into view.
  const scrollTo = useRef<number | null>(null)
  const selectionKey = selection.join(',')
  useEffect(() => {
    if (!tree || selectionKey === leftState().seenSelection) return
    // Not listed yet: the answer for this selection (its ancestors open) is on its way.
    if (selection.some((id) => !tree.byId.has(id))) return
    const expanded = new Set(leftState().expanded)
    for (const id of selection) for (const a of tree.byId.get(id)?.ancestors ?? []) expanded.add(a)
    setLeft({ expanded, seenSelection: selectionKey })
    scrollTo.current = selection[0] ?? null
  }, [selection, selectionKey, tree])
  useLayoutEffect(() => {
    const id = scrollTo.current ?? leftState().rename?.id ?? null
    const el = scroller.current
    if (id == null || !el) return
    const index = rows.findIndex((row) => row.id === id)
    if (index < 0) return
    scrollTo.current = null
    const top = index * PITCH
    if (top < el.scrollTop) el.scrollTop = top
    else if (top + ROW > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW - el.clientHeight
  }, [rows])

  // Engine requests: rename the topmost listed of `ids` (Cmd+R, F2), collapse all (Alt+L).
  const startRename = useCallback(
    (ids: number[]) => {
      if (!tree || devRefuses(designKit, 'object.rename')) return
      const expanded = new Set(leftState().expanded)
      for (const id of ids) for (const a of tree.byId.get(id)?.ancestors ?? []) expanded.add(a)
      const listed = filtering ? matchingRows(tree, filter) : listRows(tree, expanded)
      const id = listed.map((row) => row.id).find((row) => ids.includes(row))
      if (id == null) return
      designKit.ui.setLeftTab('file')
      designKit.ui.setLeftOpen(true)
      setLeft({ expanded, rename: { id, start: true }, anchor: id, seenSelection: String(id) })
      if (selection.length !== 1 || selection[0] !== id) void run('select.set', { ids: [id] })
    },
    [designKit, filter, filtering, run, selection, tree],
  )
  // One layer renames inline here; two or more open the batch Rename layers dialog (MenusHost).
  useUiRequest('layers.rename', (data) => {
    const given = (data as { ids?: unknown } | null)?.ids
    const ids = Array.isArray(given) ? given.filter((id): id is number => typeof id === 'number') : selection
    if (ids.length === 1) startRename(ids)
  })
  useUiRequest('layers.collapseAll', () => {
    const expanded = new Set<number>()
    for (const id of selection) for (const a of tree?.byId.get(id)?.ancestors ?? []) expanded.add(a)
    setLeft({ expanded })
  })

  // Clicks: select (Shift: the listed rows from the anchor; Cmd or Shift: toggle).
  const clickRow = (row: Row, mods: { shift: boolean; command: boolean }) => {
    const order = rows.map((r) => r.id)
    const anchor = left.anchor != null && selected.has(left.anchor) ? order.indexOf(left.anchor) : -1
    const at = order.indexOf(row.id)
    let next: number[]
    let nextAnchor = row.id
    if (mods.shift && anchor >= 0 && at >= 0) {
      next = mods.command ? [...selection] : []
      for (const id of order.slice(Math.min(anchor, at), Math.max(anchor, at) + 1)) if (!next.includes(id)) next.push(id)
      nextAnchor = left.anchor ?? row.id
    } else if (mods.shift || mods.command) {
      next = selected.has(row.id) ? selection.filter((id) => id !== row.id) : [...selection, row.id]
    } else next = [row.id]
    // A click in the panel needs no reveal.
    setLeft({ anchor: nextAnchor, seenSelection: next.join(','), pagesFocus: false })
    void run('select.set', { ids: next })
  }

  // Drags: reorder and reparent, or a sweep of eyes or padlocks.
  const [target, setTarget] = useState<Target>(null)
  const [sweep, setSweep] = useState<Sweep | null>(null)
  const pointer = useRef<{ x: number; y: number } | null>(null)

  const rowAt = (clientY: number): { index: number; rel: number } | null => {
    const el = scroller.current
    if (!el) return null
    const rect = el.getBoundingClientRect()
    if (clientY < rect.top || clientY > rect.bottom) return null
    const y = clientY - rect.top + el.scrollTop
    const index = Math.floor(y / PITCH)
    const within = y - index * PITCH
    if (index < 0 || index >= rowsNow.current.length || within > ROW) return null
    return { index, rel: within / ROW }
  }

  const startDrag = (row: Row, event: ReactPointerEvent) => {
    const start = { x: event.clientX, y: event.clientY }
    const mods = { shift: event.shiftKey, command: event.metaKey || event.ctrlKey }
    let dragging = false
    let hold: { id: number; timer: number } | null = null
    let frame = 0
    let current: Target = null
    const ids = selected.has(row.id) ? [...selection] : [row.id]
    // What each listed row takes, asked when the drag starts and again when the rows change
    // (holding over a closed layer opens it).
    let drops: Map<number, DropRow> | null = null
    // The rows asked about (a render may list the same rows anew) and their ids.
    let askedRows: Row[] | null = null
    let askedKey = ''
    const askDrops = () => {
      const listed = rowsNow.current
      askedRows = listed
      const key = listed.map((r) => r.id).join(',')
      if (key === askedKey) return
      askedKey = key
      void designKit.query<DropInfo>('layers.dropInfo', { ids: listed.map((r) => r.id), moving: ids }).then(
        (answer) => {
          if (askedKey !== key) return
          drops = new Map(answer.rows.map((r) => [r.id, r]))
          if (pointer.current) locate(pointer.current.x, pointer.current.y)
        },
        () => undefined,
      )
    }
    const clearHold = () => {
      if (hold) window.clearTimeout(hold.timer)
      hold = null
    }
    const locate = (x: number, y: number) => {
      const el = scroller.current
      const pageEl = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-page-id]')
      const page = pageEl ? Number(pageEl.dataset.pageId) : null
      const hit = rowAt(y)
      const rect = el?.getBoundingClientRect()
      const inList = rect && x >= rect.left && x <= rect.right
      let next: Target = null
      const listed = rowsNow.current
      if (dragging && askedRows !== listed) askDrops()
      const info = hit && inList ? drops?.get(listed[hit.index].id) : undefined
      if (hit && info) {
        const over = listed[hit.index]
        if (info.blocked) {
          // The dragged layers themselves take no drop; what is inside them refuses it.
          if (!ids.includes(over.id)) next = { index: hit.index, zone: 0, refused: true, spot: null }
        } else {
          const zone = info.container && hit.rel >= 0.25 && hit.rel < 0.75 ? 0 : hit.rel < 0.5 ? -1 : 1
          const spot = (zone === 0 ? info.drop?.into : zone < 0 ? info.drop?.above : info.drop?.below) ?? null
          next = { index: hit.index, zone, refused: spot == null, spot }
        }
        // Holding over a closed group or frame opens it, to drop deeper inside.
        if (next && next.zone === 0 && !next.refused && over.hasKids && !leftState().expanded.has(over.id)) {
          if (hold?.id !== over.id) {
            clearHold()
            const id = over.id
            hold = { id, timer: window.setTimeout(() => toggleExpanded(id, true), HOLD_MS) }
          }
        } else clearHold()
      } else clearHold()
      current = next
      setTarget(next)
      setLeft((now) => ({ drag: now.drag ? { ...now.drag, x, y, page } : now.drag }))
    }
    const scroll = () => {
      const el = scroller.current
      const p = pointer.current
      if (el && p) {
        const rect = el.getBoundingClientRect()
        let dy = 0
        if (p.y < rect.top + EDGE) dy = Math.min(rect.top + EDGE - p.y, EDGE)
        else if (p.y > rect.bottom - EDGE) dy = -Math.min(p.y - (rect.bottom - EDGE), EDGE)
        if (dy !== 0 && p.x >= rect.left && p.x <= rect.right) {
          el.scrollTop -= dy * 0.5
          locate(p.x, p.y)
        }
      }
      frame = requestAnimationFrame(scroll)
    }
    const move = (e: PointerEvent) => {
      if (!dragging) {
        if (Math.hypot(e.clientX - start.x, e.clientY - start.y) < 4) return
        if (devRefuses(designKit, 'node.reparent')) {
          stop()
          return
        }
        dragging = true
        shareHover(null)
        askDrops()
        const label = ids.length === 1 ? row.name : t('{n} layers', { n: ids.length })
        setLeft({ drag: { from: row.id, ids, label, x: e.clientX, y: e.clientY, page: null } })
        frame = requestAnimationFrame(scroll)
      }
      pointer.current = { x: e.clientX, y: e.clientY }
      locate(e.clientX, e.clientY)
    }
    const finish = () => {
      stop()
      clearHold()
      cancelAnimationFrame(frame)
      setTarget(null)
      pointer.current = null
    }
    const up = (e: PointerEvent) => {
      if (!dragging) {
        stop()
        clickRow(row, mods)
        return
      }
      const drag = leftState().drag
      finish()
      setLeft({ drag: null })
      if (drag?.page != null) {
        const name = document.querySelector<HTMLElement>(`[data-page-id='${drag.page}']`)?.dataset.pageName ?? ''
        if (!devRefuses(designKit, 'object.moveToPage')) void run('object.moveToPage', { ids, page: drag.page }).then((done) => done != null && designKit.toast(t('Moved to {name}', { name })))
        return
      }
      if (current?.spot && rowAt(e.clientY)?.index === current.index) void run('node.reparent', { ids, ...current.spot })
    }
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || !dragging) return
      e.preventDefault()
      e.stopPropagation()
      finish()
      setLeft({ drag: null })
    }
    const stop = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('keydown', key, true)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    window.addEventListener('keydown', key, true)
  }

  const startSweep = (row: Row, lock: boolean, event: ReactPointerEvent) => {
    event.stopPropagation()
    if (event.button !== 0) return
    const command = lock ? 'object.toggleLock' : 'object.toggleVisible'
    if (devRefuses(designKit, command)) return
    const start = event.clientY
    // The state the first row switches to: every row the sweep passes gets it.
    let state: Sweep = { lock, on: lock ? !row.locked : row.visible, ids: [row.id] }
    let swept = false
    const move = (e: PointerEvent) => {
      if (!swept && Math.abs(e.clientY - start) < 4) return
      swept = true
      const hit = rowAt(e.clientY)
      const over = hit ? rowsNow.current[hit.index] : null
      if (over && !state.ids.includes(over.id)) state = { ...state, ids: [...state.ids, over.id] }
      setSweep(state)
    }
    const up = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      setSweep(null)
      if (!swept) void run(command, { ids: [row.id] })
      else void run(command, lock ? { ids: state.ids, locked: state.on } : { ids: state.ids, visible: !state.on })
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }

  const toggle = async (row: Row, deep: boolean) => {
    const open = !left.expanded.has(row.id)
    if (!deep) {
      toggleExpanded(row.id)
      return
    }
    // Alt: everything inside opens or closes with it.
    const all = await designKit.query<LayersAnswer>('layers', { all: true }).catch(() => null)
    const full = all ? buildTree(all) : tree
    if (!full) return
    const ids: number[] = []
    const walk = (id: number) => {
      ids.push(id)
      for (const kid of full.children.get(id) ?? []) walk(kid)
    }
    walk(row.id)
    setLeft((now) => {
      const expanded = new Set(now.expanded)
      for (const id of ids) {
        if (open) expanded.add(id)
        else expanded.delete(id)
      }
      return { expanded }
    })
  }

  // Keys while the tree has the keyboard.
  const keyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.target instanceof HTMLInputElement) return
    const key = event.key
    if (key === 'F10' && event.shiftKey) {
      const el = scroller.current?.querySelector<HTMLElement>(`[data-row-id='${selection[selection.length - 1]}']`) ?? (document.activeElement as HTMLElement | null)
      const rect = el?.getBoundingClientRect()
      if (rect) openContextMenu({ clientX: rect.left + 40, clientY: rect.bottom, source: 'layers' })
      event.preventDefault()
      event.stopPropagation()
      return
    }
    if (!['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(key) || event.metaKey || event.ctrlKey || event.altKey) return
    event.preventDefault()
    event.stopPropagation()
    const select = (ids: number[], anchor: number) => {
      setLeft({ anchor, seenSelection: '' })
      void run('select.set', { ids })
    }
    const last = selection[selection.length - 1]
    const i = rows.findIndex((row) => row.id === last)
    if (i < 0) {
      const first = key === 'ArrowUp' ? rows[rows.length - 1] : key === 'ArrowDown' ? rows[0] : undefined
      if (first) select([first.id], first.id)
      return
    }
    const row = rows[i]
    if (key === 'ArrowUp' || key === 'ArrowDown') {
      const j = key === 'ArrowUp' ? i - 1 : i + 1
      const next = rows[j]
      if (!next) return
      if (event.shiftKey) {
        const anchor = left.anchor != null && selected.has(left.anchor) ? left.anchor : row.id
        const a = Math.max(rows.findIndex((r) => r.id === anchor), 0)
        const ids = a <= j ? rows.slice(a, j + 1).map((r) => r.id) : rows.slice(j, a + 1).map((r) => r.id).reverse()
        select(ids, anchor)
      } else select([next.id], next.id)
    } else if (key === 'ArrowLeft') {
      if (row.hasKids && !filtering && left.expanded.has(row.id)) toggleExpanded(row.id, false)
      else if (row.parent !== tree?.page && !filtering) select([row.parent], row.parent)
    } else if (row.hasKids && !filtering) {
      if (!left.expanded.has(row.id)) toggleExpanded(row.id, true)
      else if (rows[i + 1]?.depth === row.depth + 1) select([rows[i + 1].id], rows[i + 1].id)
    }
  }

  // Keyboard focus follows the selection's last row while the tree has the keyboard.
  useLayoutEffect(() => {
    const el = scroller.current
    if (!el || !el.contains(document.activeElement) || document.activeElement instanceof HTMLInputElement) return
    el.querySelector<HTMLElement>(`[data-row-id='${selection[selection.length - 1]}']`)?.focus({ preventScroll: true })
  }, [selectionKey, rows, selection])

  const first = Math.max(0, Math.floor(view.top / PITCH))
  const lastRow = Math.min(rows.length, Math.ceil((view.top + view.height) / PITCH) + 1)
  const total = rows.length * PITCH
  const preferred = selection[selection.length - 1]
  const shownRows = rows.slice(first, lastRow)
  const renameId = left.rename?.id
  // A rename scrolled out of view keeps its field (unseen), so the rename goes on.
  const renameIndex = renameId != null ? rows.findIndex((row) => row.id === renameId) : -1
  if (renameIndex >= 0 && (renameIndex < first || renameIndex >= lastRow)) shownRows.push(rows[renameIndex])
  useEffect(() => {
    if (renameId != null && tree && renameIndex < 0 && rows.length) setLeft({ rename: null })
  }, [renameId, renameIndex, rows.length, tree])

  const drag = left.drag
  const targetRow = target ? rows[target.index] : null

  return (
    <>
      <div className="design-left-header" data-testid="design-layers-header">
        <span className="design-left-header-title">{t('Layers')}</span>
        <button type="button" className="design-left-header-button" style={{ right: 10 }} aria-label={t('Search layers')} data-tip={t('Search layers')} aria-pressed={left.layerSearch} data-testid="design-layers-search" onClick={() => setLeft((now) => ({ layerSearch: !now.layerSearch, layerFilter: '' }))}>
          <Icon name="Search" size={14} />
        </button>
      </div>
      {left.layerSearch ? <SearchRow value={left.layerFilter} placeholder={t('Find layers')} onChange={(value) => setLeft({ layerFilter: value })} onClose={() => setLeft({ layerSearch: false, layerFilter: '' })} testId="design-layers-filter" /> : null}
      <div
        ref={scroller}
        className="design-layers-scroll design-scroll-hidden"
        role="tree"
        aria-label={t('Layers')}
        aria-multiselectable
        data-testid="design-layers"
        data-dragging={drag ? (target?.refused ? 'refused' : 'true') : undefined}
        onScroll={(event) => setView({ top: event.currentTarget.scrollTop, height: event.currentTarget.clientHeight })}
        onKeyDown={keyDown}
        onPointerLeave={() => shareHover(null)}
      >
        <div
          className="design-layers-content"
          style={{ height: Math.max(total, view.height) }}
          onClick={(event) => {
            if (event.target === event.currentTarget && rows.length) void run('select.none', {})
          }}
        >
          {tree && rows.length === 0 ? <div className="design-layers-empty">{filtering ? t('No matching layers') : t('Add a frame (F) or a shape (R) to get started')}</div> : null}
          {shownRows.map((row) => {
            const index = rows.indexOf(row)
            const isSelected = selected.has(row.id)
            const inSelected = !isSelected && row.ancestors.some((a) => selected.has(a))
            const swept = sweep?.ids.includes(row.id) ? sweep : null
            const hidden = swept && !swept.lock ? swept.on : !row.visible
            const locked = swept?.lock ? swept.on : row.locked
            const shown = !row.dimmed
            const x0 = rowX0(row.depth)
            const open = left.expanded.has(row.id)
            const name = row.name || t(kindLabel(row.type, row.icon.toLowerCase()))
            const renaming = renameId === row.id
            return (
              <div
                key={row.id}
                className="design-layer-row"
                role="treeitem"
                tabIndex={row.id === preferred ? 0 : -1}
                aria-selected={isSelected}
                aria-expanded={row.hasKids && !filtering ? open : undefined}
                aria-level={row.depth + 1}
                aria-label={name}
                data-row-id={row.id}
                data-selected={isSelected || undefined}
                data-child={inSelected || undefined}
                data-component={row.component || row.icon === 'Slot' || undefined}
                data-dimmed={!shown || undefined}
                data-set={hidden || locked || row.lockedAbove || undefined}
                data-design-prefer={row.id === preferred || undefined}
                data-hovered={canvasHover === row.id || undefined}
                data-testid={`design-layer-${row.id}`}
                style={{ top: index * PITCH, visibility: index < first || index >= lastRow ? 'hidden' : undefined }}
                onPointerEnter={() => {
                  if (!leftState().drag) shareHover(row.id)
                }}
                onPointerDown={(event) => {
                  if (event.button === 0 && !(event.target instanceof HTMLInputElement)) startDrag(row, event)
                }}
                onDoubleClick={(event) => {
                  if (event.target instanceof HTMLInputElement || (event.target as Element).closest('button')) return
                  if (!devRefuses(designKit, 'object.rename')) setLeft({ rename: { id: row.id, start: true } })
                }}
                onContextMenu={(event) => {
                  event.preventDefault()
                  if (!isSelected) {
                    setLeft({ anchor: row.id, seenSelection: String(row.id) })
                    void run('select.set', { ids: [row.id] })
                  }
                  openContextMenu({ clientX: event.clientX, clientY: event.clientY, source: 'layers', ids: isSelected ? selection : [row.id] })
                }}
              >
                <span className="design-layer-row-box" />
                {row.hasKids && !filtering ? (
                  <button
                    type="button"
                    className="design-layer-disclosure"
                    style={{ left: x0 - 1 }}
                    aria-label={t(open ? 'Collapse' : 'Expand')}
                    tabIndex={-1}
                    data-testid={`design-layer-toggle-${row.id}`}
                    onPointerDown={(event) => event.stopPropagation()}
                    onClick={(event) => void toggle(row, event.altKey)}
                  >
                    <Icon name={open ? 'Chevron' : 'ChevronRight'} size={12} />
                  </button>
                ) : null}
                <span className="design-layer-icon" style={{ left: x0 + 15 }}>
                  <Icon name={row.icon} size={14} />
                </span>
                {renaming ? (
                  <RenameInput
                    value={row.name}
                    style={{ left: x0 + 30, right: 10, top: 2, bottom: 2 }}
                    testId="design-layer-rename"
                    onDone={(next) => {
                      setLeft({ rename: null })
                      if (next != null) void run('object.rename', { ids: [row.id], name: next })
                      requestAnimationFrame(() => scroller.current?.querySelector<HTMLElement>(`[data-row-id='${row.id}']`)?.focus({ preventScroll: true }))
                    }}
                    onTab={(back) => {
                      const next = rows[index + (back ? -1 : 1)]
                      if (!next) return
                      setTimeout(() => {
                        setLeft({ rename: { id: next.id, start: true }, anchor: next.id, seenSelection: String(next.id) })
                        void run('select.set', { ids: [next.id] })
                      })
                    }}
                  />
                ) : (
                  <span className={`design-layer-name ${row.bold ? 'design-s11' : 'design-r11'}`} style={{ left: x0 + 36 }}>
                    {name}
                  </span>
                )}
                {!dev || locked || row.lockedAbove ? (
                  <button
                    type="button"
                    className="design-layer-toggle"
                    data-on={locked || row.lockedAbove || undefined}
                    data-inherited={(!locked && row.lockedAbove) || undefined}
                    style={{ right: 34 }}
                    role="switch"
                    aria-checked={locked || row.lockedAbove}
                    aria-label={t('Locked')}
                    tabIndex={-1}
                    disabled={dev}
                    data-testid={`design-layer-lock-${row.id}`}
                    onPointerDown={(event) => startSweep(row, true, event)}
                  >
                    <Icon name={locked || row.lockedAbove ? 'Lock' : 'Unlock'} size={14} />
                  </button>
                ) : null}
                {!dev || hidden ? (
                  <button
                    type="button"
                    className="design-layer-toggle"
                    data-on={hidden || undefined}
                    style={{ right: 12 }}
                    role="switch"
                    aria-checked={hidden}
                    aria-label={t('Hidden')}
                    tabIndex={-1}
                    disabled={dev}
                    data-testid={`design-layer-eye-${row.id}`}
                    onPointerDown={(event) => startSweep(row, false, event)}
                  >
                    <Icon name={hidden ? 'EyeOff' : 'Eye'} size={14} />
                  </button>
                ) : null}
              </div>
            )
          })}
          {target && targetRow && !target.refused && drag && drag.page == null ? (
            target.zone === 0 ? (
              <span className="design-layer-drop-into" style={{ top: target.index * PITCH }} />
            ) : (
              <span className="design-insertion-line" style={{ top: target.index * PITCH + (target.zone < 0 ? 0 : ROW) - 1, left: rowX0(targetRow.depth), right: 8 }} />
            )
          ) : null}
        </div>
      </div>
      <ScrollBar target={scroller} testId="design-layers-scrollbar" />
      {drag ? <DragGhost x={drag.x} y={drag.y} label={drag.label} /> : null}
    </>
  )
}
