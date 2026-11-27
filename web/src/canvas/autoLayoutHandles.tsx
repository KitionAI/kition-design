/**
 * The auto layout padding and gap bands over one selected, unrotated auto layout frame (the
 * engine's `chrome.autoLayout`), with the Move tool. A band shows while hovered; dragging one sets
 * its value as one undo step (`node.set` in an interaction); a press the browser cancels puts it
 * back. With Alt or Shift a padding drag sets the opposite side too, with both
 * all four, and the sides it sets light up while hovered and dragged. A click on a band (a press
 * that does not move) types its value there: Enter sets it as one undo step, Escape keeps it, and
 * the keyboard is the canvas's again. A gap spaced by the alignment (Auto) shows the distance;
 * dragging or typing it makes the gap fixed at the new value.
 */
import { useEffect, useRef, useState } from 'react'
import type { KitCanvasView } from '@kition/sdk/editor'
import { useDesignKit } from '../kit/hooks'
import { returnKeyboardLater } from '../kit/focus'
import type { DesignChrome } from '../kit/types'
import { RenameInput } from '../left/fields'
import { parseEdit } from '../ui/math'
import type { ZoneKey } from './chrome'

type Rect = { x: number; y: number; width: number; height: number }

/** One band: a padding side (`side` 0–3: top, right, bottom, left) or a gap (`side` -1). */
export type Zone = {
  key: string
  prop: string
  value: number
  rect: Rect
  axis: 'x' | 'y'
  sign: 1 | -1
  gap: boolean
  auto: boolean
  side: number
}

type Mods = { alt: boolean; shift: boolean }

/** The bands of `al`, padding first, keyed as the canvas chrome draws them. */
export function zonesOf(al: NonNullable<DesignChrome['autoLayout']>): Zone[] {
  return [
    ...al.padding.map((zone) => ({
      key: `padding:${zone.side}`,
      prop: zone.key,
      value: zone.value,
      rect: zone.rect,
      axis: zone.side === 0 || zone.side === 2 ? ('y' as const) : ('x' as const),
      sign: zone.side === 0 || zone.side === 3 ? (1 as const) : (-1 as const),
      gap: false,
      auto: false,
      side: zone.side,
    })),
    ...al.gaps.map((zone, index) => ({
      key: `gap:${index}`,
      prop: zone.key,
      value: zone.value,
      rect: zone.rect,
      axis: zone.vertical ? ('y' as const) : ('x' as const),
      sign: 1 as const,
      gap: true,
      auto: zone.auto === true,
      side: -1,
    })),
  ]
}

/** The padding sides a change of `zone` sets: its own; with Alt or Shift the opposite one too; with both all four. */
export function sidesOf(zone: Zone, mods: Mods): number[] {
  if (zone.gap) return []
  if (mods.alt && mods.shift) return [0, 1, 2, 3]
  if (mods.alt || mods.shift) return [zone.side, (zone.side + 2) % 4]
  return [zone.side]
}

/** The band keys lit for `zone` with `mods` held. */
function litKeys(zone: Zone, mods: Mods): string {
  return zone.gap ? zone.key : sidesOf(zone, mods).map((side) => `padding:${side}`).join(' ')
}

/** The properties that set `zone` to `value` with `mods` held (an Auto gap becomes fixed). */
export function propsFor(zone: Zone, value: number, mods: Mods): Record<string, unknown> {
  if (zone.gap) {
    if (!zone.auto) return { [zone.prop]: value }
    return zone.prop === 'crossGap' ? { crossGap: value, contentAlign: 'auto' } : { gap: value, mainAlign: 'min' }
  }
  const sides = sidesOf(zone, mods)
  if (sides.length === 4) return { padding: value }
  if (sides.length === 2) return { [zone.axis === 'y' ? 'paddingVertical' : 'paddingHorizontal']: value }
  return { [zone.prop]: value }
}

/** How far (CSS pixels) a press moves before it is a drag rather than a click. */
const CLICK_SLOP = 3

const fmt = (value: number) => String(Math.round(value * 100) / 100)

export function AutoLayoutHandles({ chrome, view, hot, onHot }: { chrome: DesignChrome; view: KitCanvasView; hot: ZoneKey; onHot: (key: ZoneKey) => void }) {
  const { interaction, run, t, readOnly } = useDesignKit()
  const al = chrome.autoLayout!
  const zones = zonesOf(al)
  const lit = new Set(hot?.split(' ') ?? [])
  // The band under the pointer, and the one being dragged with its latest pointer event.
  const hovered = useRef<Zone | null>(null)
  const dragging = useRef<{ zone: Zone; again: (mods: Mods) => void } | null>(null)
  const [typing, setTyping] = useState<{ zone: Zone; mods: Mods } | null>(null)
  const latest = useRef({ onHot, typing })
  latest.current = { onHot, typing }

  // Alt and Shift pressed or let go with the pointer still: the lit sides follow, and a drag
  // sets what it now covers.
  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      if (event.key !== 'Alt' && event.key !== 'Shift') return
      const mods = { alt: event.altKey, shift: event.shiftKey }
      const drag = dragging.current
      const zone = drag?.zone ?? hovered.current
      if (!zone || latest.current.typing) return
      latest.current.onHot(litKeys(zone, mods))
      drag?.again(mods)
    }
    window.addEventListener('keydown', keys, true)
    window.addEventListener('keyup', keys, true)
    return () => {
      window.removeEventListener('keydown', keys, true)
      window.removeEventListener('keyup', keys, true)
    }
  }, [])

  const commitTyped = (text: string | null) => {
    const open = typing
    setTyping(null)
    onHot(null)
    returnKeyboardLater()
    if (!open || text === null) return
    // Arithmetic as the panel's fields take it (`+8`, `*2`), on the value shown.
    const edit = parseEdit(text, open.zone.value)
    const value = edit?.kind === 'set' ? edit.value : edit ? edit.apply(open.zone.value) : null
    if (value === null || !Number.isFinite(value)) return
    void run('node.set', { ids: [al.id], props: propsFor(open.zone, Math.max(0, value), open.mods) })
  }

  return (
    <>
      {zones.map((zone) => {
        const left = (zone.rect.x - view.x) * view.zoom
        const top = (zone.rect.y - view.y) * view.zoom
        return (
          <div
            key={zone.key}
            className="design-al-zone"
            data-testid={`design-al-${zone.key.replace(':', '-')}`}
            data-lit={lit.has(zone.key) || undefined}
            style={{ left, top, width: zone.rect.width * view.zoom, height: zone.rect.height * view.zoom, cursor: zone.axis === 'y' ? 'ns-resize' : 'ew-resize' }}
            onPointerEnter={(event) => {
              hovered.current = zone
              if (!dragging.current && !typing) onHot(litKeys(zone, { alt: event.altKey, shift: event.shiftKey }))
            }}
            onPointerLeave={() => {
              if (hovered.current?.key === zone.key) hovered.current = null
              if (dragging.current?.zone.key !== zone.key && !typing) onHot(null)
            }}
            onPointerDown={(event) => {
              if (event.button !== 0 || readOnly) return
              event.preventDefault()
              event.stopPropagation()
              if (typing) return
              const target = event.currentTarget
              target.setPointerCapture(event.pointerId)
              const start = { x: event.clientX, y: event.clientY }
              let at = { x: event.clientX, y: event.clientY }
              let mods = { alt: event.altKey, shift: event.shiftKey }
              let moved = false
              let drag: ReturnType<typeof interaction> | null = null
              let last: string | null = null
              const apply = () => {
                const delta = ((zone.axis === 'x' ? at.x - start.x : at.y - start.y) * zone.sign) / view.zoom
                const next = Math.max(0, Math.round(zone.value + delta))
                const props = propsFor(zone, next, mods)
                const said = JSON.stringify(props)
                if (said === last) return
                last = said
                drag ??= interaction(t(zone.gap ? 'interaction.gap' : 'interaction.padding'))
                drag.update('node.set', { ids: [al.id], props })
              }
              dragging.current = {
                zone,
                again: (next) => {
                  mods = next
                  if (moved) apply()
                },
              }
              onHot(litKeys(zone, mods))
              const move = (e: PointerEvent) => {
                at = { x: e.clientX, y: e.clientY }
                mods = { alt: e.altKey, shift: e.shiftKey }
                if (!moved && Math.hypot(at.x - start.x, at.y - start.y) < CLICK_SLOP) return
                moved = true
                onHot(litKeys(zone, mods))
                apply()
              }
              // A release commits (a press that never moved types the value instead); a press the
              // browser cancels puts the value back. Losing the capture without either (the band
              // went away) keeps what the drag did.
              const end = (how: 'commit' | 'cancel') => () => {
                target.removeEventListener('pointermove', move)
                target.removeEventListener('pointerup', up)
                target.removeEventListener('pointercancel', cancel)
                target.removeEventListener('lostpointercapture', up)
                dragging.current = null
                if (drag) void (how === 'commit' ? drag.commit() : drag.cancel())
                if (how === 'commit' && !moved) {
                  setTyping({ zone, mods })
                  return
                }
                onHot(hovered.current ? litKeys(hovered.current, mods) : null)
              }
              const up = end('commit')
              const cancel = end('cancel')
              target.addEventListener('pointermove', move)
              target.addEventListener('pointerup', up)
              target.addEventListener('pointercancel', cancel)
              target.addEventListener('lostpointercapture', up)
            }}
          />
        )
      })}
      {typing ? <ZoneField zone={typing.zone} view={view} label={t(typing.zone.gap ? 'interaction.gap' : 'interaction.padding')} onDone={commitTyped} /> : null}
    </>
  )
}

/** The field a click on a band opens over it, its value selected. */
function ZoneField({ zone, view, label, onDone }: { zone: Zone; view: KitCanvasView; label: string; onDone: (text: string | null) => void }) {
  const width = 48
  const height = 20
  const cx = (zone.rect.x + zone.rect.width / 2 - view.x) * view.zoom
  const cy = (zone.rect.y + zone.rect.height / 2 - view.y) * view.zoom
  return (
    <div onPointerDown={(event) => event.stopPropagation()} onDoubleClick={(event) => event.stopPropagation()} aria-label={label}>
      <RenameInput value={fmt(zone.value)} style={{ left: cx - width / 2, top: cy - height / 2, width, height }} testId="design-al-field" onDone={(text) => onDone(text)} />
    </div>
  )
}
