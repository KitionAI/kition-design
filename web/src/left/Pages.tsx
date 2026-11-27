/**
 * The Pages section (2.2, `layers.rs` `pages`): the header (a toggle, Search pages, Add new page),
 * the search row, the page list (go to a page, rename by double-click, reorder by drag, dividers,
 * the page context menu, layer drops), and the splitter that sizes the list.
 */
import { useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { ScrollBar } from '../ui/ScrollBar'
import { Menu } from '../ui/Menu'
import { RenameInput, SearchRow } from './fields'
import { rememberedPageView } from '../canvas/view'
import { devRefuses } from './dev'
import { setLeft, useLeft } from './store'

type PageRow = { id: number; name: string; current: boolean; divider: boolean }

const ROW = 32
const PITCH = 38

/** The list's height in `avail` below the Pages header: asked (40 % by default), at least three rows, leaving the layers their header and three rows. */
export function pagesListHeight(asked: number | null, avail: number) {
  const least = 3 * ROW
  const most = Math.max(avail - 40 - 3 * ROW, least)
  const want = asked != null && Number.isFinite(asked) ? asked : avail * 0.4
  return Math.min(Math.max(want, least), most)
}

/** Where a page dragged from list position `from` lands with the pointer at `y` (list coordinates): its new index and the line's y. */
function dropSlot(count: number, from: number, y: number): { to: number; y: number } | null {
  const slack = ROW / 2
  if (count === 0 || y < -slack || y > (count - 1) * PITCH + ROW + slack) return null
  let gap = 0
  for (let i = 0; i < count; i += 1) if (i * PITCH + ROW / 2 < y) gap = i + 1
  if (gap === from || gap === from + 1) return null
  const line = gap === 0 ? 0 : gap === count ? (count - 1) * PITCH + ROW : (gap - 1) * PITCH + ROW + (PITCH - ROW) / 2
  return { to: gap > from ? gap - 1 : gap, y: line }
}

export function Pages({ room }: { room: number }) {
  const designKit = useDesignKit()
  const { t, run, inspect } = designKit
  const left = useLeft()
  const answer = useQuery<{ current: number; pages: PageRow[] }>('pages')
  const pages: PageRow[] = answer?.pages ?? inspect.pages.map((page) => ({ id: page.id, name: page.name, current: page.id === inspect.page, divider: false }))
  const current = answer?.current ?? inspect.page
  const filter = left.pageFilter.trim().toLowerCase()
  const filtering = filter !== ''
  const shown = pages.filter((page) => !filtering || (!page.divider && page.name.toLowerCase().includes(filter)))
  const list = useRef<HTMLDivElement>(null)
  const [menu, setMenu] = useState<{ x: number; y: number; page: PageRow } | null>(null)
  const [reorder, setReorder] = useState<{ from: number; label: string; x: number; y: number; slot: { to: number; y: number } | null } | null>(null)

  // The current page's row scrolls into view when the page changes.
  useLayoutEffect(() => {
    const el = list.current
    const index = shown.findIndex((page) => page.id === current)
    if (!el || index < 0) return
    const top = index * PITCH
    if (top < el.scrollTop) el.scrollTop = top
    else if (top + ROW > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW - el.clientHeight
    // Only on a page change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current])

  const select = (page: PageRow) => {
    setLeft({ pagesFocus: true })
    if (page.divider || page.id === current) return
    // A page shown before goes back to its own view (the canvas restores it); a new one fits.
    const fits = rememberedPageView(designKit.doc, page.id) == null
    void run('page.select', { id: page.id }).then(() => (fits ? designKit.viewTo('view.zoomFit') : undefined))
  }

  const deletePage = async (id: number) => {
    if (devRefuses(designKit, 'page.delete')) return
    if ((await run('page.delete', { id })) != null) designKit.toast(t('Page deleted'))
  }

  const rename = (id: number) => {
    if (devRefuses(designKit, 'page.rename')) return
    setLeft({ renamePage: id })
  }

  const rowDown = (event: ReactPointerEvent<HTMLDivElement>, page: PageRow, index: number) => {
    if (event.button !== 0 || filtering) return
    const start = { x: event.clientX, y: event.clientY }
    let dragging = false
    const move = (e: PointerEvent) => {
      if (!dragging && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 4) return
      if (!dragging) {
        if (devRefuses(designKit, 'page.move')) {
          stop()
          return
        }
        dragging = true
      }
      const top = list.current?.getBoundingClientRect().top ?? 0
      const y = e.clientY - top + (list.current?.scrollTop ?? 0)
      setReorder({ from: index, label: page.divider ? t('Divider') : page.name, x: e.clientX, y: e.clientY, slot: dropSlot(shown.length, index, y) })
    }
    const up = (e: PointerEvent) => {
      stop()
      if (!dragging) return
      const top = list.current?.getBoundingClientRect().top ?? 0
      const rect = list.current?.getBoundingClientRect()
      const inside = rect && e.clientY >= rect.top - 3 && e.clientY <= rect.bottom + 3
      const slot = inside ? dropSlot(shown.length, index, e.clientY - top + (list.current?.scrollTop ?? 0)) : null
      setReorder(null)
      if (slot) void run('page.move', { id: page.id, index: slot.to })
    }
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.preventDefault()
      e.stopPropagation()
      stop()
      setReorder(null)
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

  const height = Math.min(pagesListHeight(left.pagesHeight, room), shown.length ? shown.length * PITCH - (PITCH - ROW) : 0)

  return (
    <>
      <div className="design-left-header" data-testid="design-pages-header">
        <button type="button" className="design-left-header-toggle" aria-expanded={left.pagesOpen} onClick={() => setLeft((now) => ({ pagesOpen: !now.pagesOpen }))} data-testid="design-pages-toggle">
          {t('Pages')}
        </button>
        <button type="button" className="design-left-header-button" style={{ right: 38 }} aria-label={t('Search pages')} data-tip={t('Search pages')} aria-pressed={left.pageSearch} data-testid="design-pages-search" onClick={() => setLeft((now) => ({ pageSearch: !now.pageSearch, pageFilter: '', pagesOpen: now.pageSearch ? now.pagesOpen : true }))}>
          <Icon name="Search" size={14} />
        </button>
        <button
          type="button"
          className="design-left-header-button"
          style={{ right: 10 }}
          aria-label={t('Add new page')}
          data-tip={t('Add new page')}
          data-testid="design-pages-add"
          onClick={() => {
            setLeft({ pagesFocus: true })
            if (devRefuses(designKit, 'page.add')) return
            // The new page's name is open for typing (Enter keeps it), so a name typed right
            // away names it instead of pressing the + button again.
            void run<{ id?: number; divider?: boolean }>('page.add', {}).then((added) => {
              if (typeof added?.id === 'number' && !added.divider) setLeft({ renamePage: added.id })
            })
          }}
        >
          <Icon name="Plus" size={14} />
        </button>
      </div>
      {left.pagesOpen && left.pageSearch ? <SearchRow value={left.pageFilter} placeholder={t('Search pages')} onChange={(value) => setLeft({ pageFilter: value })} onClose={() => setLeft({ pageSearch: false, pageFilter: '' })} testId="design-pages-search-field" /> : null}
      {left.pagesOpen ? (
        <>
          <div
            ref={list}
            className="design-pages-list design-scroll-hidden"
            role="listbox"
            aria-label={t('Pages')}
            style={{ height }}
            data-testid="design-pages-list"
            onKeyDown={(event) => {
              if ((event.key === 'Delete' || event.key === 'Backspace') && inspect.selection.length === 0 && left.pagesFocus && !left.renamePage) {
                event.preventDefault()
                event.stopPropagation()
                void deletePage(current)
              }
            }}
          >
            {shown.map((page, index) => {
              const renaming = left.renamePage === page.id
              const dropTarget = left.drag?.page === page.id
              return (
                <div
                  key={page.id}
                  className="design-page-row"
                  role="option"
                  tabIndex={page.id === current ? 0 : -1}
                  aria-selected={page.id === current}
                  aria-label={page.divider ? t('Divider') : page.name}
                  data-current={page.id === current || undefined}
                  data-divider={(page.divider && !renaming) || undefined}
                  data-drop={dropTarget || undefined}
                  data-page-id={page.divider || page.id === current ? undefined : page.id}
                  data-page-name={page.name}
                  data-design-prefer={page.id === current && left.pagesFocus ? true : undefined}
                  data-testid={`design-page-${page.id}`}
                  style={{ top: index * PITCH }}
                  onPointerDown={(event) => rowDown(event, page, index)}
                  onClick={() => select(page)}
                  onDoubleClick={() => rename(page.id)}
                  onContextMenu={(event) => {
                    event.preventDefault()
                    setMenu({ x: event.clientX, y: event.clientY, page })
                  }}
                >
                  <span className="design-page-row-box" />
                  {renaming ? (
                    <RenameInput
                      value={page.name}
                      bold={page.id === current}
                      style={{ left: 14, right: 10, top: 2, bottom: 2 }}
                      testId="design-page-rename"
                      onDone={(name) => {
                        setLeft({ renamePage: null })
                        if (name != null) void run('page.rename', { id: page.id, name })
                      }}
                    />
                  ) : page.divider ? null : (
                    <span className={`design-page-row-name ${page.id === current ? 'design-s11' : 'design-r11'}`}>{page.name}</span>
                  )}
                </div>
              )
            })}
            {reorder?.slot ? <span className="design-insertion-line" style={{ top: reorder.slot.y - 1, left: 16, right: 16 }} /> : null}
          </div>
          <ScrollBar target={list} />
          {reorder ? <DragGhost x={reorder.x} y={reorder.y} label={reorder.label} /> : null}
          <PagesSplitter shown={height} />
        </>
      ) : null}
      {menu ? (
        <Menu
          tone="light"
          variant="buttons"
          anchor={{ left: menu.x, top: menu.y, right: menu.x, bottom: menu.y }}
          gap={0}
          onClose={() => setMenu(null)}
          testId="design-page-menu"
          items={[
            { label: t('Rename page'), onSelect: () => rename(menu.page.id), testId: 'design-page-menu-rename' },
            { label: t('Duplicate page'), onSelect: () => !devRefuses(designKit, 'page.duplicate') && void run('page.duplicate', { id: menu.page.id }), testId: 'design-page-menu-duplicate' },
            { label: t('Delete page'), onSelect: () => void deletePage(menu.page.id), testId: 'design-page-menu-delete' },
          ]}
        />
      ) : null}
    </>
  )
}

/** What a drag carries, at the pointer: the label in a small outlined box (`drag_ghost`). */
export function DragGhost({ x, y, label }: { x: number; y: number; label: string }) {
  return (
    <span className="design-drag-ghost design-r11" style={{ left: x + 14, top: y + 10 }}>
      {label}
    </span>
  )
}

/** The line between the pages and the layers: drag to size the list, double-click for the default. */
function PagesSplitter({ shown }: { shown: number }) {
  const [active, setActive] = useState(false)
  return (
    <div
      className="design-pages-splitter"
      data-active={active || undefined}
      data-testid="design-pages-splitter"
      onDoubleClick={() => setLeft({ pagesHeight: null })}
      onPointerDown={(event) => {
        if (event.button !== 0) return
        event.preventDefault()
        const target = event.currentTarget
        target.setPointerCapture(event.pointerId)
        setActive(true)
        const start = event.clientY
        const move = (e: PointerEvent) => setLeft({ pagesHeight: shown + e.clientY - start })
        const up = () => {
          target.removeEventListener('pointermove', move)
          target.removeEventListener('pointerup', up)
          setActive(false)
        }
        target.addEventListener('pointermove', move)
        target.addEventListener('pointerup', up)
      }}
    />
  )
}
