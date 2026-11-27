/**
 * Comments on the canvas (2.5 §17): pins and clusters from `query('commentPins')`, the thread
 * popover (header with Resolve, More and Close; messages with reactions and their own menus; a
 * reply composer with @mentions), and the draft composer. Pins show with the Comment tool, View ▸
 * Comments, or while a thread or draft is open. A click toggles a thread, dragging your own pin
 * moves it, a cluster zooms in; the Comment tool's press asks for a draft (`comment.new`).
 */
import { useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type KeyboardEvent as ReactKeyboardEvent } from 'react'
import { createPortal } from 'react-dom'
import { useDesignKit, useQuery, useView } from '../kit/hooks'
import type { DesignKit } from '../kit/context'
import { useUiRequest } from '../kit/requests'
import { useKeyboardReturn } from '../kit/focus'
import { Icon } from '../icons/Icon'
import { Menu, type MenuEntry } from '../ui/Menu'
import { useProtoState } from '../prototype/store'
import './comments.css'

export type Pin = { id: number; author: string; initials: string; color: string; resolved: boolean; unread: boolean; mine: boolean; open: boolean; x: number; y: number }
type Pins = { visible: boolean; pins: Pin[]; clusters: Array<{ ids: number[]; x: number; y: number }>; pinSize: number }
type Reaction = { emoji: string; users?: string[]; count?: number }
type Message = { author: string; created: number; edited: number; text: string; reactions: Reaction[] }
export type Thread = Message & { id: number; x: number; y: number; resolved: boolean; unread: boolean; replies: Message[]; link: string; frameName?: string; pageName?: string }

// The open thread or draft (page coordinates), shared with the UI requests. `posted`: the open
// thread is the one the person just posted, whose reply field does not take the keyboard (the next
// key is the canvas's: V, R or Escape leave the Comment tool, C12).
type State = { open: number | null; draft: { x: number; y: number } | null; posted?: boolean }
let state: State = { open: null, draft: null }
const listeners = new Set<() => void>()
function setState(next: Partial<State>) {
  state = { ...state, ...next }
  listeners.forEach((listener) => listener())
}
function useCommentState() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => state,
  )
}

/** The open thread's id, or null. */
export function useOpenComment() {
  return useCommentState().open
}

/** Opens thread `id` (`null` closes it). */
export function openThread(id: number | null) {
  setState({ open: id, draft: null, posted: false })
}

/**
 * Closes thread `id` if it is still the open one: for an answer that arrives after the person may
 * have opened another thread or started a draft, which it must leave alone.
 */
function closeThread(id: number) {
  if (state.open === id) openThread(null)
}

const EMOJI = ['👍', '❤️', '😂', '🎉', '👀', '🙏', '🔥', '✅']
const PIN = 28
const POPOVER_W = 300

/** "now", "5m", "3h", "2d", "1w" (upstream `ago`). */
function ago(designKit: DesignKit, created: number) {
  const d = Math.max(0, Math.floor(Date.now() / 1000) - created)
  if (d < 60) return designKit.t('now')
  if (d < 3600) return designKit.t('{n}m', { n: Math.floor(d / 60) })
  if (d < 86_400) return designKit.t('{n}h', { n: Math.floor(d / 3600) })
  if (d < 604_800) return designKit.t('{n}d', { n: Math.floor(d / 86_400) })
  return designKit.t('{n}w', { n: Math.floor(d / 604_800) })
}

/** The canvas surface's box in the page, and how a document pixel maps onto it. */
function useSurface() {
  const view = useView()
  const [rect, setRect] = useState<DOMRect | null>(null)
  useLayoutEffect(() => {
    const surface = document.querySelector('[data-testid="kit-canvas-surface"]')
    if (!surface) return undefined
    const update = () => setRect(surface.getBoundingClientRect())
    update()
    const observer = new ResizeObserver(update)
    observer.observe(surface)
    window.addEventListener('resize', update)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', update)
    }
  }, [view?.width, view?.height])
  return { view, rect }
}

/** A message's text with @mentions in semibold `accent_text`. */
function Body({ text, people }: { text: string; people: string[] }) {
  const parts = useMemo(() => {
    const names = [...people].sort((a, b) => b.length - a.length).map((name) => name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
    if (!names.length) return [text]
    return text.split(new RegExp(`(@(?:${names.join('|')}))`, 'g'))
  }, [people, text])
  return (
    <div className="design-comment-body">
      {parts.map((part, index) => (part.startsWith('@') && people.includes(part.slice(1)) ? <b key={index}>{part}</b> : <span key={index}>{part}</span>))}
    </div>
  )
}

function Avatar({ name, color, size }: { name: string; color?: string; size: number }) {
  return (
    <span className="design-comment-avatar" style={{ width: size, height: size, background: color ?? 'var(--design-success)', fontSize: size <= 18 ? 8 : 9 }}>
      {name.slice(0, 1).toUpperCase()}
    </span>
  )
}

/**
 * The composer (frame `field`, r6; text regular 12; a 24 px round send button, ACCENT with text):
 * Enter posts, Shift+Enter breaks the line; typing `@` offers people (arrows, Enter or Tab inserts).
 */
function Composer({ placeholder, people, me, onPost, initial = '', testId, onEscape, focus = true }: { placeholder: string; people: string[]; me: string; onPost: (text: string) => void; initial?: string; testId?: string; onEscape?: () => void; focus?: boolean }) {
  const { t } = useDesignKit()
  const [text, setText] = useState(initial)
  const [pick, setPick] = useState(0)
  const area = useRef<HTMLTextAreaElement>(null)
  useEffect(() => {
    if (focus) area.current?.focus()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once, when it opens
  }, [])
  // Posted or closed, the keyboard goes back to the canvas.
  useKeyboardReturn()
  const mention = /@([^\s@]*)$/.exec(text)
  const matches = mention ? people.filter((name) => name !== me && name.toLowerCase().startsWith(mention[1].toLowerCase())).slice(0, 6) : []
  const insert = (name: string) => {
    if (!mention) return
    setText(`${text.slice(0, mention.index)}@${name} `)
    setPick(0)
    area.current?.focus()
  }
  const post = () => {
    const value = text.trim()
    if (!value) return
    onPost(value)
    setText('')
  }
  const keyDown = (event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (matches.length) {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        setPick((now) => (now + (event.key === 'ArrowDown' ? 1 : matches.length - 1)) % matches.length)
        event.preventDefault()
        return
      }
      if (event.key === 'Enter' || event.key === 'Tab') {
        insert(matches[Math.min(pick, matches.length - 1)])
        event.preventDefault()
        return
      }
      if (event.key === 'Escape') {
        setText(`${text} `)
        event.preventDefault()
        event.stopPropagation()
        return
      }
    }
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      post()
    } else if (event.key === 'Escape' && onEscape) {
      event.preventDefault()
      event.stopPropagation()
      onEscape()
    }
  }
  return (
    <div className="design-comment-composer">
      <textarea ref={area} rows={1} value={text} placeholder={placeholder} aria-label={placeholder} onChange={(event) => setText(event.target.value)} onKeyDown={keyDown} data-testid={testId} />
      <button type="button" className="design-comment-send" data-on={text.trim() ? true : undefined} aria-label={t('Post')} data-tip={t('Post')} onClick={post} data-testid={testId ? `${testId}-send` : undefined}>
        <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
          <path d="M6 10.5V1.8M2.4 5.2 6 1.6l3.6 3.6" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {matches.length ? (
        <div className="design-comment-mentions" role="listbox">
          {matches.map((name, index) => (
            <div key={name} role="option" aria-selected={index === pick} data-active={index === pick || undefined} onPointerDown={(event) => event.preventDefault()} onClick={() => insert(name)}>
              <Avatar name={name} size={18} />
              <span>{name}</span>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}

/** One message: avatar, author, age; on hover Add reaction and (own) More; reactions; edit mode. */
function MessageView({ thread, message, index, people, me, colors }: { thread: Thread; message: Message; index: number; people: string[]; me: string; colors: ReadonlyMap<string, string> }) {
  const designKit = useDesignKit()
  const { t, run } = designKit
  const [editing, setEditing] = useState(false)
  const [menu, setMenu] = useState<HTMLElement | null>(null)
  const [react, setReact] = useState<HTMLElement | null>(null)
  const reply = index === 0 ? {} : { reply: index - 1 }
  const when = message.edited > 0 ? t('{when} · edited', { when: ago(designKit, message.created) }) : ago(designKit, message.created)
  const mine = message.author === me
  const items: MenuEntry[] = [
    { label: t('Edit'), onSelect: () => setEditing(true) },
    { label: t(index === 0 ? 'Delete thread' : 'Delete'), onSelect: () => void run('comment.delete', { id: thread.id, ...reply }).then(() => index === 0 && closeThread(thread.id)) },
  ]
  return (
    <div className="design-comment-message" data-testid="design-comment-message">
      <div className="design-comment-meta">
        <Avatar name={message.author} color={colors.get(message.author)} size={24} />
        <span className="design-comment-author">{message.author}</span>
        <span className="design-comment-age">{when}</span>
        <span className="design-comment-tools">
          <button type="button" className="design-icon-button" style={{ width: 20, height: 20 }} aria-label={t('Add reaction')} data-tip={t('Add reaction')} onClick={(event) => setReact(react ? null : event.currentTarget)}>
            <Icon name="Reaction" size={14} />
          </button>
          {mine ? (
            <button type="button" className="design-icon-button" style={{ width: 20, height: 20 }} aria-label={t('More')} data-tip={t('More')} onClick={(event) => setMenu(menu ? null : event.currentTarget)}>
              <Icon name="Dots" size={14} />
            </button>
          ) : null}
        </span>
      </div>
      {editing ? (
        <div style={{ marginLeft: 36 }}>
          <Composer placeholder={t('Edit comment')} people={people} me={me} initial={message.text} onPost={(text) => void run('comment.edit', { id: thread.id, text, ...reply }).then(() => setEditing(false))} onEscape={() => setEditing(false)} />
          <div className="design-comment-hint">{t('Enter to save · Esc to cancel')}</div>
        </div>
      ) : (
        <Body text={message.text} people={people} />
      )}
      {message.reactions.length ? (
        <div className="design-comment-reactions">
          {message.reactions.map((reaction) => {
            const users = reaction.users ?? []
            const on = users.includes(me)
            return (
              <button key={reaction.emoji} type="button" className="design-comment-chip" data-on={on || undefined} data-tip={users.join(', ')} onClick={() => void run('comment.react', { id: thread.id, emoji: reaction.emoji, ...reply })}>
                <span>{reaction.emoji}</span>
                <span>{reaction.count ?? users.length}</span>
              </button>
            )
          })}
        </div>
      ) : null}
      {menu ? <Menu tone="light" items={items} anchor={menu} placement="below-end" minWidth={160} onClose={() => setMenu(null)} /> : null}
      {react ? (
        <ReactionPicker
          anchor={react}
          mine={message.reactions.filter((r) => (r.users ?? []).includes(me)).map((r) => r.emoji)}
          onPick={(emoji) => {
            setReact(null)
            void run('comment.react', { id: thread.id, emoji, ...reply })
          }}
          onClose={() => setReact(null)}
        />
      ) : null}
    </div>
  )
}

function ReactionPicker({ anchor, mine, onPick, onClose }: { anchor: HTMLElement; mine: string[]; onPick: (emoji: string) => void; onClose: () => void }) {
  const items: MenuEntry[] = [
    {
      type: 'custom',
      render: () => (
        <div className="design-comment-emoji">
          {EMOJI.map((emoji) => (
            <button key={emoji} type="button" data-on={mine.includes(emoji) || undefined} onClick={() => onPick(emoji)}>
              {emoji}
            </button>
          ))}
        </div>
      ),
    },
  ]
  return <Menu tone="light" items={items} anchor={anchor} placement="below-start" onClose={onClose} />
}

/** The thread or draft popover: width 300 at the spot + (36, −28), flipped left when it would leave the canvas. */
function Popover({ spot, rect, children, testId, zIndex }: { spot: { x: number; y: number }; rect: DOMRect; children: React.ReactNode; testId: string; zIndex?: number }) {
  let left = spot.x + 36
  if (left + POPOVER_W > rect.right) left = spot.x - 308
  const top = Math.max(rect.top + 8, Math.min(spot.y - 28, rect.bottom - 200))
  return createPortal(
    <div className="design-comment-popover" style={{ left, top, width: POPOVER_W, zIndex }} data-testid={testId} onPointerDown={(event) => event.stopPropagation()}>
      {children}
    </div>,
    document.body,
  )
}

/** A pin's face: the author's initials on their colour, the unread dot, the resolved check. */
export function PinFace({ pin }: { pin: Pick<Pin, 'initials' | 'color' | 'resolved' | 'unread'> }) {
  return (
    <>
      <span className="design-comment-pin-avatar" style={{ background: pin.resolved ? '#969696' : pin.color }}>
        {pin.initials}
      </span>
      {pin.unread ? <span className="design-comment-pin-unread" /> : null}
      {pin.resolved ? (
        <span className="design-comment-pin-check">
          <Icon name="Check" size={10} />
        </span>
      ) : null}
    </>
  )
}

/**
 * An open thread's popover at `spot` (page pixels; it flips left inside `rect`): the header with
 * Resolve, More and Close, the messages and the reply composer. The canvas and the presenter show it.
 */
export function ThreadPopover({ thread, spot, rect, people, me, colors, zIndex, focusReply = true }: { thread: Thread; spot: { x: number; y: number }; rect: DOMRect; people: string[]; me: string; colors: ReadonlyMap<string, string>; zIndex?: number; focusReply?: boolean }) {
  const designKit = useDesignKit()
  const { t, run, refresh } = designKit
  const [moreAt, setMoreAt] = useState<HTMLElement | null>(null)
  const after = async (promise: Promise<unknown>) => {
    await promise
    await refresh()
  }
  const moreItems: MenuEntry[] = [
    { label: t('Mark as unread'), onSelect: () => void after(run('comment.markRead', { id: thread.id, read: false })).then(() => closeThread(thread.id)) },
    {
      label: t('Copy link'),
      onSelect: () =>
        void navigator.clipboard
          .writeText(thread.link)
          .then(() => designKit.toast(t('Link copied')))
          .catch(() => undefined),
    },
    { label: t(thread.resolved ? 'Mark as unresolved' : 'Mark as resolved'), onSelect: () => void after(run('comment.resolve', { id: thread.id, resolved: !thread.resolved })) },
    ...(thread.author === me ? [{ label: t('Delete thread'), onSelect: () => void after(run('comment.delete', { id: thread.id })).then(() => closeThread(thread.id)) }] : []),
  ]
  return (
    <Popover spot={spot} rect={rect} testId="design-comment-thread" zIndex={zIndex}>
      <div className="design-comment-head">
        <span>{t('Comment')}</span>
        <button type="button" className="design-icon-button" aria-label={t(thread.resolved ? 'Unresolve' : 'Resolve')} data-tip={t(thread.resolved ? 'Unresolve' : 'Resolve')} aria-pressed={thread.resolved} onClick={() => void after(run('comment.resolve', { id: thread.id, resolved: !thread.resolved }))} data-testid="design-comment-resolve">
          <Icon name="Check" size={14} />
        </button>
        <button type="button" className="design-icon-button" aria-label={t('More')} data-tip={t('More')} onClick={(event) => setMoreAt(moreAt ? null : event.currentTarget)} data-testid="design-comment-more">
          <Icon name="Dots" size={14} />
        </button>
        <button type="button" className="design-icon-button" aria-label={t('Close')} data-tip={t('Close')} onClick={() => openThread(null)} data-testid="design-comment-close">
          <Icon name="Close" size={14} />
        </button>
      </div>
      <div className="design-comment-messages">
        {[thread, ...thread.replies].map((message, index) => (
          <MessageView key={`${index}:${message.created}`} thread={thread} message={message} index={index} people={people} me={me} colors={colors} />
        ))}
      </div>
      <Composer key={thread.id} placeholder={t('Reply')} people={people} me={me} onPost={(text) => void after(run('comment.reply', { id: thread.id, text }))} testId="design-comment-composer" focus={focusReply} />
      {moreAt ? <Menu tone="light" items={moreItems} anchor={moreAt} placement="below-end" minWidth={180} onClose={() => setMoreAt(null)} /> : null}
    </Popover>
  )
}

export function CommentsLayer() {
  const designKit = useDesignKit()
  const { t, run, inspect, refresh } = designKit
  const { open, draft, posted } = useCommentState()
  const { view, rect } = useSurface()
  const zoom = view ? Math.round(view.zoom * 1000) / 1000 : 1
  const pins = useQuery<Pins>('commentPins', { zoom, ...(open !== null ? { open } : {}) })
  const comments = useQuery<{ comments: Thread[] }>('comments', {}, { skip: open === null })
  const people = useQuery<{ people: string[]; you: string }>('commentPeople', {}, { skip: open === null && draft === null })
  const presenting = useProtoState((p) => p.player?.mode === 'present')
  const drag = useRef<{ id: number; x: number; y: number; moved: boolean } | null>(null)
  const [dragAt, setDragAt] = useState<{ id: number; dx: number; dy: number } | null>(null)

  // The engine asks: open a thread (a link, `comment.open`), or start a draft (the Comment tool's press).
  useUiRequest('comment.open', (data) => {
    const id = (data as { id?: unknown })?.id
    if (typeof id === 'number') openThread(id)
  })
  useUiRequest('comment.new', (data) => {
    const at = data as { x?: number; y?: number }
    if (typeof at?.x === 'number' && typeof at.y === 'number') setState({ draft: { x: at.x, y: at.y }, open: null })
  })

  // A press elsewhere on the canvas closes the open thread (the Comment tool then asks for a draft).
  useEffect(() => {
    if (open === null && draft === null) return undefined
    const down = (event: PointerEvent) => {
      if (event.target instanceof Element && event.target.closest('[data-testid="kit-canvas-surface"]')) setState({ open: null, draft: null })
    }
    document.addEventListener('pointerdown', down, true)
    return () => document.removeEventListener('pointerdown', down, true)
  }, [draft, open])

  // Escape closes the thread or the draft (a composer's mention list and an edit take it first).
  useEffect(() => {
    if (open === null && draft === null) return undefined
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented || document.querySelector('.design-layer')) return
      event.preventDefault()
      event.stopPropagation()
      setState({ open: null, draft: null })
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [draft, open])

  // Opening an unread thread reads it.
  const thread = comments?.comments.find((c) => c.id === open) ?? null
  useEffect(() => {
    if (thread?.unread) void run('comment.markRead', { id: thread.id })
  }, [run, thread?.id, thread?.unread])

  if (!view || !rect || !pins) return null
  const shown = pins.visible || inspect.tool === 'comment' || open !== null || draft !== null
  const area = inspect.area
  const toScreen = (x: number, y: number) => ({ x: rect.left + (x - view.x) * view.zoom, y: rect.top + (y - view.y) * view.zoom })
  const me = people?.you ?? 'You'
  const names = people?.people ?? []
  const after = async (promise: Promise<unknown>) => {
    await promise
    await refresh()
  }

  const clustered = new Set(pins.clusters.flatMap((c) => c.ids))
  // Each author's colour, as the pins draw it.
  const colors = new Map(pins.pins.map((pin) => [pin.author, pin.color]))
  const openPin = pins.pins.find((pin) => pin.id === open)
  const spotOf = (pin: Pin) => {
    const at = toScreen(pin.x, pin.y)
    return dragAt?.id === pin.id ? { x: at.x + dragAt.dx, y: at.y + dragAt.dy } : at
  }

  const pinDown = (pin: Pin) => (event: React.PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return
    event.stopPropagation()
    event.preventDefault()
    const target = event.currentTarget
    target.setPointerCapture(event.pointerId)
    drag.current = { id: pin.id, x: event.clientX, y: event.clientY, moved: false }
    const move = (e: PointerEvent) => {
      const now = drag.current
      if (!now || !pin.mine) return
      const dx = e.clientX - now.x
      const dy = e.clientY - now.y
      if (!now.moved && Math.hypot(dx, dy) < 3) return
      now.moved = true
      setDragAt({ id: pin.id, dx, dy })
    }
    const stop = () => {
      target.removeEventListener('pointermove', move)
      target.removeEventListener('pointerup', up)
      target.removeEventListener('pointercancel', cancel)
      target.removeEventListener('lostpointercapture', cancel)
    }
    // A press the browser cancels, or a capture lost without a release, moves nothing and opens nothing.
    const cancel = () => {
      stop()
      drag.current = null
      setDragAt(null)
    }
    const up = (e: PointerEvent) => {
      stop()
      const now = drag.current
      drag.current = null
      setDragAt(null)
      if (now?.moved) {
        const x = pin.x + (e.clientX - now.x) / view.zoom + area.x
        const y = pin.y + (e.clientY - now.y) / view.zoom + area.y
        void after(run('comment.move', { id: pin.id, x, y }))
      } else openThread(open === pin.id ? null : pin.id)
    }
    target.addEventListener('pointermove', move)
    target.addEventListener('pointerup', up)
    target.addEventListener('pointercancel', cancel)
    target.addEventListener('lostpointercapture', cancel)
  }

  const zoomTo = (ids: number[]) => {
    const handle = designKit.canvas.current
    const chosen = pins.pins.filter((pin) => ids.includes(pin.id))
    if (!handle || !chosen.length) return
    const xs = chosen.map((pin) => pin.x)
    const ys = chosen.map((pin) => pin.y)
    const cx = (Math.min(...xs) + Math.max(...xs)) / 2
    const cy = (Math.min(...ys) + Math.max(...ys)) / 2
    const next = Math.min(view.zoom * 2, 256)
    handle.zoomTo(next)
    handle.panTo(cx - view.width / next / 2, cy - view.height / next / 2)
  }

  return (
    <>
      {shown
        ? createPortal(
            <div className="design-comment-layer" style={{ left: rect.left, top: rect.top, width: rect.width, height: rect.height }} data-testid="design-comment-layer">
              {pins.pins
                .filter((pin) => !clustered.has(pin.id) || pin.id === open)
                .sort((a, b) => Number(a.id === open) - Number(b.id === open))
                .map((pin) => {
                  const spot = spotOf(pin)
                  return (
                    <button
                      key={pin.id}
                      type="button"
                      className="design-comment-pin"
                      data-open={pin.id === open || undefined}
                      data-resolved={pin.resolved || undefined}
                      style={{ left: spot.x - rect.left, top: spot.y - rect.top - PIN, cursor: dragAt?.id === pin.id ? 'grabbing' : 'pointer' }}
                      aria-label={`${pin.author}: ${t('Comment')}`}
                      onPointerDown={pinDown(pin)}
                      data-testid={`design-comment-pin-${pin.id}`}
                    >
                      <PinFace pin={pin} />
                    </button>
                  )
                })}
              {pins.clusters.map((cluster) => {
                const at = toScreen(cluster.x, cluster.y)
                const count = String(cluster.ids.filter((id) => id !== open).length)
                const width = PIN + 7 * (count.length - 1)
                return (
                  <button
                    key={cluster.ids.join(',')}
                    type="button"
                    className="design-comment-cluster"
                    style={{ left: at.x - rect.left, top: at.y - rect.top - PIN, width }}
                    aria-label={t('{n} comments', { n: cluster.ids.length })}
                    onPointerDown={(event) => event.stopPropagation()}
                    onClick={() => zoomTo(cluster.ids)}
                    data-testid="design-comment-cluster"
                  >
                    <span className="design-comment-cluster-back" />
                    <span className="design-comment-cluster-front">
                      <span>{count}</span>
                    </span>
                  </button>
                )
              })}
              {draft ? <span className="design-comment-pin" data-open="true" style={{ left: toScreen(draft.x - area.x, draft.y - area.y).x - rect.left, top: toScreen(draft.x - area.x, draft.y - area.y).y - rect.top - PIN }}><span className="design-comment-draft-dot" /></span> : null}
            </div>,
            document.body,
          )
        : null}
      {thread && openPin && !presenting ? <ThreadPopover thread={thread} spot={spotOf(openPin)} rect={rect} people={names} me={me} colors={colors} focusReply={!posted} /> : null}
      {draft ? (
        <Popover spot={toScreen(draft.x - area.x, draft.y - area.y)} rect={rect} testId="design-comment-draft">
          <div className="design-comment-head">
            <span>{t('Comment')}</span>
            <button type="button" className="design-icon-button" aria-label={t('Close')} data-tip={t('Close')} onClick={() => setState({ draft: null })}>
              <Icon name="Close" size={14} />
            </button>
          </div>
          <Composer
            placeholder={t('Add a comment')}
            people={names}
            me={me}
            onPost={(text) => {
              // The post closes its draft and opens its thread only if the person has not acted
              // since: every press on the canvas, pin click, Escape, Close, opened thread or new
              // draft replaces `state.draft`, so this draft's object still being there means
              // nothing happened (no timing involved).
              const posting = draft
              void run<{ id?: number }>('comment.add', { x: posting.x, y: posting.y, text, anchor: true }).then(async (answer) => {
                await refresh()
                if (state.draft === posting) setState({ draft: null, open: typeof answer?.id === 'number' ? answer.id : null, posted: true })
              })
            }}
            testId="design-comment-draft-text"
          />
        </Popover>
      ) : null}
    </>
  )
}
