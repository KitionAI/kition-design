/**
 * The number field (2.1 §7 `number_field*`): a lead (a short label such as `X`, or an icon) that
 * scrubs the value when dragged (1 step per pixel, Shift ×10, Alt ×0.1, on with Pointer Lock at the
 * screen's edge), and a value that takes typed math (`10+5`, `*2`, `+10` per layer on a mixed
 * value), Up and Down steps, Enter, Tab or blur to commit and Escape to cancel. Mixed values show
 * "Mixed", an empty one its placeholder ("Auto").
 */
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { Icon, type IconName } from '../icons/Icon'
import { formatNumber, parseEdit, type NumberEdit } from './math'

export type ScrubHandlers = {
  /** The drag began (start an interaction). */
  begin: () => void
  /** The value under the drag: `start + delta` (start is the shown value, 0 when mixed), and the delta alone. */
  move: (value: number, delta: number) => void
  /** The drag ended (commit), or Escape cancelled it. */
  end: (cancelled: boolean) => void
}

/**
 * The horizontal movement of each pointer event of a scrub that began at `startX`. Without Pointer
 * Lock the position tells: WebKit leaves `movementX` at 0 for some pointer sources, and Chromium
 * on Linux has given one event of a scrub a `movementX` of about minus the pointer's x (−1230 on
 * a lead at x 1230), which threw the value to its minimum. Under Pointer Lock only the movement
 * tells (the position stays put), except on the event the lock began or ended on: its movement
 * is measured from somewhere else, so it moves nothing.
 */
export function scrubDeltas(startX: number): (event: PointerEvent) => number {
  let lastX = startX
  let locked = document.pointerLockElement !== null
  return (event) => {
    const nowLocked = document.pointerLockElement !== null
    const changed = nowLocked !== locked
    locked = nowLocked
    const dx = changed ? 0 : nowLocked ? event.movementX || event.clientX - lastX : event.clientX - lastX
    lastX = event.clientX
    return dx
  }
}

export type NumberFieldProps = {
  /** The value, or null for none (the placeholder shows). */
  value: number | null
  /** The selection's values differ: "Mixed" shows and typed edits may be relative. */
  mixed?: boolean
  /** A short text lead (`X`, `W`), drawn in `text_secondary`. */
  label?: string
  /** An icon lead (14 px in a 24 px box). */
  icon?: IconName
  /** Drawn after the value (`%`, `°`). */
  suffix?: string
  step?: number
  /** The Up/Down step and Shift's; `step` and `step × 10` by default. */
  nudge?: { small: number; big: number }
  min?: number
  max?: number
  decimals?: number
  placeholder?: string
  mixedLabel?: string
  width?: CSSProperties['width']
  disabled?: boolean
  /** A typed or stepped value. */
  onCommit: (edit: NumberEdit) => void
  /** Scrubbing; without it the lead does not scrub. */
  scrub?: ScrubHandlers
  /** Shown at the right while hovered (the variable button). */
  addon?: ReactNode
  /** The lead's tooltip ("X position"). */
  tip?: string
  /** The input's accessible name (the tooltip, else the label). */
  ariaLabel?: string
  testId?: string
}

const clamp = (value: number, min?: number, max?: number) => Math.min(max ?? Infinity, Math.max(min ?? -Infinity, value))

export function NumberField({
  value,
  mixed = false,
  label,
  icon,
  suffix = '',
  step = 1,
  nudge,
  min,
  max,
  decimals = 2,
  placeholder = '',
  mixedLabel = 'Mixed',
  width,
  disabled = false,
  onCommit,
  scrub,
  addon,
  tip,
  ariaLabel,
  testId,
}: NumberFieldProps) {
  const input = useRef<HTMLInputElement>(null)
  const lead = useRef<HTMLSpanElement>(null)
  const [focused, setFocused] = useState(false)
  const [text, setText] = useState('')
  const [scrubbing, setScrubbing] = useState<number | null>(null)
  const cancelled = useRef(false)
  // Enter commits before it blurs: the blur must not commit the same text again.
  const committed = useRef(false)
  // Focusing selects the whole value once the focused text is in the input (a layout effect, so
  // keys typed straight after the focus replace it; a frame later they could append in WebKit), and
  // the press that focused keeps that selection rather than placing the caret.
  const selectAll = useRef(false)
  const pressFocus = useRef(false)
  useLayoutEffect(() => {
    if (!focused || !selectAll.current) return
    selectAll.current = false
    input.current?.select()
  }, [focused, text])
  const current = mixed ? null : value
  const shown = scrubbing ?? current
  const display = shown === null ? '' : `${formatNumber(shown, decimals)}${suffix}`

  useEffect(() => {
    if (!focused) setText(display)
  }, [display, focused])

  const commitText = () => {
    const raw = text.trim()
    const stripped = suffix && raw.endsWith(suffix) ? raw.slice(0, -suffix.length) : raw
    if (raw === display || stripped === (shown === null ? '' : formatNumber(shown, decimals))) return
    const edit = parseEdit(stripped, current)
    if (!edit) return
    onCommit(edit.kind === 'set' ? { kind: 'set', value: clamp(edit.value, min, max) } : { ...edit, apply: (v) => clamp(edit.apply(v), min, max) })
  }

  const stepBy = (delta: number) => {
    if (current === null) onCommit({ kind: 'relative', apply: (v) => clamp(v + delta, min, max), text: `${delta >= 0 ? '+' : ''}${delta}` })
    else {
      const next = clamp(current + delta, min, max)
      setText(formatNumber(next, decimals))
      onCommit({ kind: 'set', value: next })
    }
  }

  // Scrubbing the lead: pointer lock keeps it going at the screen's edge.
  const pointerDown = (event: React.PointerEvent<HTMLSpanElement>) => {
    if (disabled || event.button !== 0) return
    event.preventDefault()
    if (!scrub) {
      input.current?.focus()
      return
    }
    const target = event.currentTarget
    const start = current ?? 0
    let moved = 0
    let total = 0
    let began = false
    let last = start
    cancelled.current = false
    const quantum = (alt: boolean) => (alt ? step / 10 : step)
    const deltaOf = scrubDeltas(event.clientX)
    let ended = false
    // WebKit can grant the lock after the drag ended: give it back then, or the next clicks go to the lead.
    const lockChanged = () => {
      if (ended && document.pointerLockElement === target) document.exitPointerLock?.()
    }
    document.addEventListener('pointerlockchange', lockChanged)
    const move = (e: PointerEvent) => {
      const dx = deltaOf(e)
      if (!began) {
        moved += Math.abs(dx)
        if (moved < 2) return
        began = true
        scrub.begin()
        try {
          void (target.requestPointerLock?.() as Promise<void> | undefined)?.catch?.(() => undefined)
        } catch {
          // No pointer lock (a frame that does not allow it): the drag still scrubs.
        }
      }
      total += dx * step * (e.shiftKey ? 10 : e.altKey ? 0.1 : 1)
      const q = quantum(e.altKey)
      const next = clamp(Math.round((start + total) / q) * q, min, max)
      if (next !== last) {
        last = next
        setScrubbing(next)
        scrub.move(next, next - start)
      }
    }
    const up = () => {
      ended = true
      setTimeout(() => document.removeEventListener('pointerlockchange', lockChanged), 1000)
      document.removeEventListener('pointermove', move, true)
      document.removeEventListener('pointerup', up, true)
      window.removeEventListener('keydown', key, true)
      if (document.pointerLockElement) document.exitPointerLock?.()
      setScrubbing(null)
      const wasCancelled = cancelled.current
      // Escape cancelled this scrub only: the next typed value commits.
      cancelled.current = false
      if (began) scrub.end(wasCancelled)
      else input.current?.focus()
    }
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return
      e.preventDefault()
      e.stopPropagation()
      cancelled.current = true
      up()
    }
    document.addEventListener('pointermove', move, true)
    document.addEventListener('pointerup', up, true)
    window.addEventListener('keydown', key, true)
  }

  const leadText = label ? (
    <span ref={lead} className="design-field-lead" data-scrub={Boolean(scrub)} data-tip={tip} style={{ minWidth: 24, padding: '0 6px', justifyContent: 'center' }} onPointerDown={pointerDown}>
      {label}
    </span>
  ) : icon ? (
    <span ref={lead} className="design-field-lead" data-icon="true" data-scrub={Boolean(scrub)} data-tip={tip} onPointerDown={pointerDown}>
      <Icon name={icon} size={14} />
    </span>
  ) : (
    <span style={{ width: 6, flex: 'none' }} />
  )

  return (
    <div className="design-field" style={{ width }} data-mixed={mixed || undefined} data-empty={value === null || undefined} data-disabled={disabled || undefined} data-testid={testId}>
      {leadText}
      <input
        ref={input}
        type="text"
        inputMode="decimal"
        spellCheck={false}
        autoComplete="off"
        disabled={disabled}
        aria-label={ariaLabel ?? tip ?? label}
        value={focused ? text : display}
        placeholder={mixed ? mixedLabel : placeholder}
        onChange={(event) => setText(event.target.value)}
        onMouseDown={(event) => {
          pressFocus.current = document.activeElement !== event.currentTarget
        }}
        onMouseUp={(event) => {
          if (!pressFocus.current) return
          pressFocus.current = false
          event.preventDefault()
        }}
        onFocus={() => {
          committed.current = false
          cancelled.current = false
          selectAll.current = true
          setFocused(true)
          setText(shown === null ? '' : formatNumber(shown, decimals))
        }}
        onBlur={() => {
          if (!cancelled.current && !committed.current) commitText()
          cancelled.current = false
          committed.current = false
          setFocused(false)
        }}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            commitText()
            committed.current = true
            setText(display)
            event.currentTarget.blur()
          } else if (event.key === 'Escape') {
            cancelled.current = true
            event.currentTarget.blur()
            event.stopPropagation()
          } else if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
            event.preventDefault()
            const amount = event.shiftKey ? (nudge?.big ?? step * 10) : (nudge?.small ?? step)
            stepBy(event.key === 'ArrowUp' ? amount : -amount)
          } else return
          event.stopPropagation()
        }}
      />
      {addon}
    </div>
  )
}
