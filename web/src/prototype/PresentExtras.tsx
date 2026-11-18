/**
 * The presenter's Inspect and Comments (upstream `present/chrome.rs` `inspect_panel`,
 * `present/player.rs` `inspect_hover`, `comments::presenter`). Inspect: the layer under the
 * pointer becomes the inspected one, outlined over the prototype, and a 280 px panel beside it
 * lists its properties as the Inspect tab does. Comments: the pins of the comments on the presented
 * frame, where the frame as designed maps onto the frame as presented; a pin opens its thread.
 *
 * The hit test and the properties come from the player (`player("inspect")`): the screen as it
 * shows (scrolled, laid out for the device) and the player's copy of the document, with what the
 * prototype changed (variables, Change to, a device's reflow).
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Note, SectionHeader } from '../ui'
import { InspectSections, type Section } from '../dev/DevPanel'
import { useDev } from '../dev/store'
import { openThread, PinFace, ThreadPopover, useOpenComment, type Pin, type Thread } from '../comments/CommentsLayer'
import type { PlayerState } from './Player'

type Box = { x: number; y: number; width: number; height: number }
/** The player's `inspect` answer (screen units). */
export type Inspected = { id: number | null; name?: string; bounds?: Box | null; clip?: Box; sections?: Section[] }

/**
 * Inspecting (upstream `inspect_hover`): `onPointer` takes the pointer over the screen and the
 * layer under it becomes the inspected one (it stays when the pointer leaves the layers); `outline`
 * draws it, clipped to the active screen (screen units → CSS pixels by the player's scale). Each
 * change of the player's state asks again, so the outline and properties follow scrolling and the
 * prototype's changes.
 */
export function usePresentInspect(on: boolean, s: PlayerState | null) {
  const { invoke } = useDesignKit()
  const dev = useDev()
  const [inspected, setInspected] = useState<Inspected | null>(null)
  const last = useRef<number | null>(null)
  const ask = useCallback(
    async (q: { x: number; y: number } | null) => {
      const params = { ...(q ?? {}), id: last.current, colorFormat: dev.colorFormat }
      const answer = JSON.parse(await invoke<string>('player', 'inspect', JSON.stringify(params))) as Inspected
      if (answer.id != null) {
        last.current = answer.id
        setInspected(answer)
      } else if (!q) {
        // The inspected layer left the screen: no outline; its properties stay.
        setInspected((prev) => (prev ? { ...prev, bounds: null } : prev))
      }
    },
    [dev.colorFormat, invoke],
  )
  const onPointer = useCallback(
    (q: { x: number; y: number } | null) => {
      if (on && q) void ask(q).catch(() => undefined)
    },
    [ask, on],
  )
  const key = s ? `${s.revision}:${s.current}:${s.scroll.join(',')}:${s.overlays.map((o) => o.id).join(',')}` : ''
  useEffect(() => {
    if (on && last.current != null) void ask(null).catch(() => undefined)
  }, [ask, key, on])
  useEffect(() => {
    if (on) return
    last.current = null
    setInspected(null)
  }, [on])
  const outline = (scale: number) => {
    const b = on ? inspected?.bounds : null
    const clip = inspected?.clip
    if (!b || !clip) return null
    return (
      <span className="design-proto-inspect-clip" style={{ left: clip.x * scale, top: clip.y * scale, width: clip.width * scale, height: clip.height * scale }}>
        <span className="design-proto-inspect-outline" style={{ left: (b.x - clip.x) * scale, top: (b.y - clip.y) * scale, width: b.width * scale, height: b.height * scale }} data-testid="design-proto-present-inspect-outline" data-id={inspected?.id} />
      </span>
    )
  }
  return { inspected, onPointer, outline }
}

/** The presenter's Inspect panel (upstream `inspect_panel`): the inspected layer's name and properties. */
export function PresentInspectPanel({ inspected }: { inspected: Inspected | null }) {
  const { t } = useDesignKit()
  return (
    <aside className="design-proto-present-inspect" aria-label={t('Inspect')} data-testid="design-proto-present-inspect">
      {inspected?.id == null ? (
        <>
          <SectionHeader title={t('Inspect')} />
          <Note>{t('Hover a layer to see its properties')}</Note>
        </>
      ) : (
        <>
          <SectionHeader title={inspected.name ?? ''} testId="design-proto-present-inspect-name" />
          <InspectSections sections={inspected.sections ?? []} testId="design-proto-present-inspect-section" />
        </>
      )}
    </aside>
  )
}

type Pins = { pins: Pin[] }

/** The comments on the presented frame: their pins over the screen, and the open thread. */
export function PresentComments({ s, scale }: { s: PlayerState; scale: number }) {
  const open = useOpenComment()
  const frame = s.current
  const list = useQuery<{ comments: Thread[] }>('comments', { frame, filter: 'open' })
  const node = useQuery<{ absoluteBounds?: Box | null }>('node', { id: frame, depth: 0 })
  // Each pin's face (initials, colour) as the canvas draws it; no clusters at this zoom.
  const faces = useQuery<Pins>('commentPins', { zoom: 4096, ...(open != null ? { open } : {}) })
  const people = useQuery<{ people: string[]; you: string }>('commentPeople', {}, { skip: open == null })
  const byId = useMemo(() => new Map((faces?.pins ?? []).map((pin) => [pin.id, pin])), [faces])
  const colors = useMemo(() => new Map((faces?.pins ?? []).map((pin) => [pin.author, pin.color])), [faces])
  const fb = node?.absoluteBounds
  if (!fb || !s.frame || !s.screen) return null
  const threads = [...(list?.comments ?? [])].reverse()
  return (
    <>
      {threads.map((thread) => {
        // The frame as designed maps onto the frame as presented.
        const x = s.frame!.x + thread.x - fb.x
        const y = s.frame!.y + thread.y - fb.y
        if (x < 0 || y < 0 || x > s.screen!.width || y > s.screen!.height) return null
        const face = byId.get(thread.id)
        return (
          <PresentPin
            key={thread.id}
            thread={thread}
            face={{ initials: face?.initials ?? thread.author.slice(0, 1).toUpperCase(), color: face?.color ?? 'var(--design-success)', resolved: thread.resolved, unread: thread.unread }}
            left={x * scale}
            top={y * scale}
            open={open === thread.id}
            people={people?.people ?? []}
            me={people?.you ?? 'You'}
            colors={colors}
          />
        )
      })}
    </>
  )
}

function PresentPin({ thread, face, left, top, open, people, me, colors }: { thread: Thread; face: Pick<Pin, 'initials' | 'color' | 'resolved' | 'unread'>; left: number; top: number; open: boolean; people: string[]; me: string; colors: ReadonlyMap<string, string> }) {
  const { t } = useDesignKit()
  const ref = useRef<HTMLButtonElement>(null)
  const [spot, setSpot] = useState<{ x: number; y: number } | null>(null)
  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect()
    if (open && r) setSpot({ x: r.left, y: r.bottom })
  }, [open, left, top])
  return (
    <>
      <button
        ref={ref}
        type="button"
        className="design-comment-pin"
        data-open={open || undefined}
        data-resolved={face.resolved || undefined}
        style={{ left, top: top - 28 }}
        aria-label={`${thread.author}: ${t('Comment')}`}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={() => openThread(open ? null : thread.id)}
        data-testid={`design-proto-present-pin-${thread.id}`}
      >
        <PinFace pin={face} />
      </button>
      {open && spot ? <ThreadPopover thread={thread} spot={spot} rect={new DOMRect(0, 0, window.innerWidth, window.innerHeight)} people={people} me={me} colors={colors} zIndex={950} /> : null}
    </>
  )
}
