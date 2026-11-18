/**
 * The Comments panel (2.5 §17), in place of the Design panel while the Comment tool is on: the
 * header with Filter and sort (Open, Resolved, Only your threads, Unread, All comments; Sort by
 * date or page; Mark all as read), the search field, and one row per thread (unread dot, avatar,
 * author, age, up to three lines, frame · page · replies · Resolved). A row opens its thread and
 * centres its pin.
 */
import { useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { Menu, type MenuEntry } from '../ui/Menu'
import { openThread, useOpenComment } from './CommentsLayer'
import './comments.css'

type Filter = 'open' | 'resolved' | 'mine' | 'unread' | 'all'
type Sort = 'date' | 'page'
type Message = { author: string; created: number; text: string }
type Thread = Message & { id: number; x: number; y: number; page: number; pageName?: string; frameName?: string; resolved: boolean; unread: boolean; replies: Message[] }

const FILTERS: Array<{ id: Filter; label: string }> = [
  { id: 'open', label: 'Open|comments' },
  { id: 'resolved', label: 'Resolved' },
  { id: 'mine', label: 'Only your threads' },
  { id: 'unread', label: 'Unread' },
  { id: 'all', label: 'All comments' },
]

// The panel's filter, sort and search outlive the panel (the tool comes and goes).
let view: { filter: Filter; sort: Sort; search: string } = { filter: 'open', sort: 'date', search: '' }
const listeners = new Set<() => void>()
const setView = (next: Partial<typeof view>) => {
  view = { ...view, ...next }
  listeners.forEach((listener) => listener())
}

/** An upstream label with a context suffix ("Open|comments") without it when untranslated. */
const shown = (text: string) => text.split('|')[0]

function ago(t: (key: string, values?: Record<string, string | number>) => string, created: number) {
  const d = Math.max(0, Math.floor(Date.now() / 1000) - created)
  if (d < 60) return t('now')
  if (d < 3600) return t('{n}m', { n: Math.floor(d / 60) })
  if (d < 86_400) return t('{n}h', { n: Math.floor(d / 3600) })
  if (d < 604_800) return t('{n}d', { n: Math.floor(d / 86_400) })
  return t('{n}w', { n: Math.floor(d / 604_800) })
}

export function CommentsPanel() {
  const designKit = useDesignKit()
  const { t, run, refresh, inspect } = designKit
  const state = useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => view,
  )
  const open = useOpenComment()
  const list = useQuery<{ comments: Thread[] }>('comments', { page: 'all', filter: state.filter, sort: state.sort, ...(state.search ? { query: state.search } : {}) })
  const any = useQuery<{ comments: Thread[] }>('comments', { page: 'all' })
  const pins = useQuery<{ pins: Array<{ author: string; color: string }> }>('commentPins', {})
  const colors = useMemo(() => new Map((pins?.pins ?? []).map((pin) => [pin.author, pin.color])), [pins])
  const [menuAt, setMenuAt] = useState<HTMLElement | null>(null)
  const filterButton = useRef<HTMLButtonElement>(null)
  const threads = list?.comments ?? []

  const items: MenuEntry[] = [
    ...FILTERS.map((filter) => ({ label: shown(t(filter.label)), checked: state.filter === filter.id, onSelect: () => setView({ filter: filter.id }) })),
    { type: 'separator' as const },
    { label: t('Sort by date'), checked: state.sort === 'date', onSelect: () => setView({ sort: 'date' }) },
    { label: t('Sort by page'), checked: state.sort === 'page', onSelect: () => setView({ sort: 'page' }) },
    { type: 'separator' as const },
    { label: t('Mark all as read'), onSelect: () => void run('comment.markRead', {}).then(() => refresh()) },
  ]

  const go = (thread: Thread) => {
    openThread(thread.id)
    // Centre its pin (upstream `comment.open {view: "center"}`; the canvas owns the view here).
    const handle = designKit.canvas.current
    const now = handle?.view()
    if (handle && now && thread.page === inspect.page) handle.panTo(thread.x - inspect.area.x - now.width / now.zoom / 2, thread.y - inspect.area.y - now.height / now.zoom / 2)
    else if (thread.page !== inspect.page) void run('page.select', { id: thread.page })
  }

  let lastPage: number | null = null
  return (
    <div className="design-comments-panel" data-testid="design-comments-panel">
      <div className="design-comments-head">
        <span>{t('Comments')}</span>
        <button ref={filterButton} type="button" className="design-icon-button" aria-label={t('Filter and sort')} data-tip={t('Filter and sort')} aria-pressed={state.filter !== 'open'} onClick={() => setMenuAt(menuAt ? null : filterButton.current)} data-testid="design-comments-filter">
          <Icon name="Adjust" size={14} />
        </button>
      </div>
      <div className="design-comments-search">
        <div className="design-field design-field-text" style={{ width: '100%' }}>
          <input value={state.search} placeholder={t('Search comments')} aria-label={t('Search comments')} spellCheck={false} autoComplete="off" onChange={(event) => setView({ search: event.target.value })} data-testid="design-comments-search" />
        </div>
      </div>
      {state.filter !== 'open' ? (
        <div className="design-comments-filter">
          <span>{shown(t(FILTERS.find((f) => f.id === state.filter)?.label ?? ''))}</span>
          <button type="button" onClick={() => setView({ filter: 'open' })}>
            {t('Show open')}
          </button>
        </div>
      ) : null}
      {!threads.length ? (
        <div className="design-comments-empty">
          <strong>{t(any?.comments.length ? 'No matching comments' : 'No comments yet')}</strong>
          <span>{t(any?.comments.length ? 'Try another search or filter.' : 'Click anywhere on the canvas to leave one.')}</span>
        </div>
      ) : (
        <div className="design-comments-list">
          {threads.map((thread) => {
            const pageHeader = state.sort === 'page' && lastPage !== thread.page ? thread.pageName ?? '' : null
            lastPage = thread.page
            const meta = [thread.frameName, state.sort !== 'page' && thread.page !== inspect.page ? thread.pageName : null, thread.replies.length ? t('{n} replies', { n: thread.replies.length }) : null, thread.resolved ? t('Resolved') : null].filter(Boolean)
            const last = Math.max(thread.created, ...thread.replies.map((reply) => reply.created))
            return (
              <div key={thread.id}>
                {pageHeader !== null ? <div className="design-comments-page">{pageHeader}</div> : null}
                <button type="button" className="design-comments-row" data-open={open === thread.id || undefined} onClick={() => go(thread)} data-testid={`design-comments-row-${thread.id}`}>
                  {thread.unread ? <span className="design-comments-unread" /> : null}
                  <span className="design-comment-avatar" style={{ width: 24, height: 24, background: colors.get(thread.author) ?? 'var(--design-success)', fontSize: 9, position: 'absolute', left: 16, top: 10 }}>
                    {thread.author.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="design-comments-author" data-unread={thread.unread || undefined}>
                    {thread.author}
                  </span>
                  <span className="design-comments-age">{ago(t, last)}</span>
                  <span className="design-comments-text">{thread.text}</span>
                  {meta.length ? <span className="design-comments-meta">{meta.join(' · ')}</span> : null}
                </button>
              </div>
            )
          })}
        </div>
      )}
      {menuAt ? <Menu tone="light" items={items} anchor={menuAt} placement="below-end" minWidth={200} onClose={() => setMenuAt(null)} /> : null}
    </div>
  )
}
