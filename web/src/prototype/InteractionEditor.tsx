/**
 * The interaction editor popover (2.6, upstream `editor_popover`, `trigger_rows`,
 * `animation_rows`, `custom_easing_fields`, `curve`): floating left of the right panel, its top
 * at the interaction's row; the trigger, the actions (`ActionList`), and the animation with its
 * curve preview. Every edit runs `proto.interaction.set` (scrubs as one undo step).
 */
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useDesignKit } from '../kit/hooks'
import { Dropdown, IconButton, NumberField, Row, Segmented, Separator, SubLabels, type ScrubHandlers } from '../ui'
import { ActionList } from './ActionList'
import type { ProtoAnswer, Tree } from './data'
import { CUSTOM_BEZIER, CUSTOM_SPRING, EASINGS, GAMEPAD_BUTTONS, TRANSITIONS, TRIGGERS, animates, easingEval, easingKind, easingLabel, easingSeconds, hasDirection, isSpring, keyCombo, transitionOf, triggerKind, triggerOf, walkActions, type Direction, type Easing, type Interaction, type TriggerLabel, type TransitionType, type Variables } from './model'
import { numberOf } from './PrototypePanel'
import { RichDropdown, type RichItem } from './RichMenu'
import { openEditor, setCapturing, useProtoState } from './store'
import { targetsFor } from './targets'

export const EDITOR_WIDTH = 280

type Props = { node: number; index: number; interaction: Interaction; anchor: HTMLElement | null; proto: ProtoAnswer | null; tree: Tree | null; variables: Variables | null }

export type Patch = (fields: Record<string, unknown>) => void

export function InteractionEditor({ node, index, interaction, anchor, proto, tree, variables }: Props) {
  const { run, t } = useDesignKit()
  const box = useRef<HTMLDivElement>(null)
  const [at, setAt] = useState<{ left: number; top: number; maxHeight: number } | null>(null)
  const ids = targetsFor(proto, node)
  const patch: Patch = (fields) => void run('proto.interaction.set', { ids, index, ...fields })

  // Top-right corner at (the panel's left − 8, the row's top), clamped to the window.
  useLayoutEffect(() => {
    const place = () => {
      const panel = document.querySelector('.design-right')?.getBoundingClientRect()
      const row = anchor?.getBoundingClientRect()
      if (!panel) return
      const y = row ? row.top : panel.top + 80
      const top = Math.max(48, Math.min(y, window.innerHeight - 420))
      // The body scrolls past the window's bottom − 64 (upstream `max_h`; the header is above it).
      setAt({ left: panel.left - 8 - EDITOR_WIDTH, top, maxHeight: Math.max(160, window.innerHeight - top - 64) })
    }
    place()
    window.addEventListener('resize', place)
    const scroller = anchor?.closest('.design-panel-scroll')
    scroller?.addEventListener('scroll', place)
    return () => {
      window.removeEventListener('resize', place)
      scroller?.removeEventListener('scroll', place)
    }
  }, [anchor])

  const animated = walkActions(interaction.actions).some(animates)
  return createPortal(
    <div ref={box} className="design-proto-editor design-layer" style={at ? { left: at.left, top: at.top } : { left: -9999, top: -9999 }} role="dialog" aria-label={t('Interaction')} data-testid="design-proto-editor">
      <div className="design-proto-editor-header">
        <span>{t('Interaction')}</span>
        <IconButton icon="Close" label={t('Close')} iconSize={12} onClick={() => openEditor(null)} testId="design-proto-editor-close" />
      </div>
      <Separator />
      <div className="design-proto-editor-body" style={{ maxHeight: at?.maxHeight }}>
        <TriggerRows interaction={interaction} patch={patch} node={node} index={index} />
        <Separator />
        <SubLabels labels={[interaction.actions.length > 1 ? t('Actions') : t('Action')]} />
        <ActionList node={node} index={index} ids={ids} actions={interaction.actions} prefix={[]} proto={proto} tree={tree} variables={variables} />
        {animated ? (
          <>
            <Separator />
            <AnimationRows interaction={interaction} patch={patch} ids={ids} index={index} />
          </>
        ) : null}
        <div style={{ height: 8 }} />
      </div>
    </div>,
    document.body,
  )
}

/** The trigger, its delay, and the key or controller button (`trigger_rows`). */
function TriggerRows({ interaction, patch, node, index }: { interaction: Interaction; patch: Patch; node: number; index: number }) {
  const { t } = useDesignKit()
  const trigger = interaction.trigger
  const capturing = useProtoState((s) => s.capturing)
  const options = TRIGGERS.flatMap((label, i) => [...(i === 4 || i === 5 || i === 9 ? [{ separator: true as const }] : []), { value: label, label: t(label) }])
  const delayField = (value: number, write: (v: number) => void) => (
    <Row>
      <span className="design-proto-row-label">{t('Delay')}</span>
      <NumberField value={value} suffix="ms" width={96} step={10} min={0} max={600000} decimals={0} onCommit={(edit) => write(Math.round(Math.min(600000, Math.max(0, numberOf(edit, value)))))} testId="design-proto-delay" />
    </Row>
  )
  // Key capture: the next key press (with held modifiers) is the trigger's key; Escape cancels unless the key is empty.
  useEffect(() => {
    if (!capturing || trigger.type !== 'keyPress') return undefined
    const key = (event: KeyboardEvent) => {
      const combo = keyCombo(event)
      if (!combo) return
      event.preventDefault()
      event.stopPropagation()
      setCapturing(false)
      if (event.key !== 'Escape' || !trigger.key) patch({ trigger: { type: 'keyPress', key: combo } })
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [capturing, patch, trigger])
  // A new editor, or another trigger, stops recording.
  useEffect(() => () => setCapturing(false), [node, index])
  return (
    <>
      <SubLabels labels={[t('Trigger')]} />
      <Row>
        <Dropdown<TriggerLabel>
          value={triggerKind(trigger)}
          width="100%"
          ariaLabel={t('Trigger')}
          testId="design-proto-trigger"
          options={options}
          onChange={(label) => {
            const next = triggerOf(label, trigger)
            patch({ trigger: next })
            if (next.type === 'keyPress' && !next.key) setCapturing(true)
          }}
        />
      </Row>
      {trigger.type === 'mouseEnter' || trigger.type === 'mouseLeave' || trigger.type === 'mouseDown' || trigger.type === 'mouseUp' ? delayField(interaction.delay_ms ?? 0, (v) => patch({ delayMs: v })) : null}
      {trigger.type === 'afterDelay' ? delayField(trigger.ms, (v) => patch({ trigger: { type: 'afterDelay', ms: v } })) : null}
      {trigger.type === 'keyPress' ? (
        <Row>
          <button type="button" className="design-proto-key" data-capturing={capturing || undefined} data-empty={!trigger.key || undefined} onClick={() => setCapturing(true)} data-testid="design-proto-key">
            {capturing ? t('Press a key…') : trigger.key || t('Click to record a key')}
          </button>
          <Dropdown
            value={null}
            mixedLabel={t('Controller')}
            width={92}
            menuWidth={160}
            ariaLabel={t('Controller')}
            testId="design-proto-controller"
            options={GAMEPAD_BUTTONS.map((b) => ({ value: b, label: trigger.key.toLowerCase() === b.toLowerCase() ? `✓ ${b}` : b }))}
            onChange={(b) => {
              setCapturing(false)
              patch({ trigger: { type: 'keyPress', key: b } })
            }}
          />
        </Row>
      ) : null}
    </>
  )
}

const DIRECTIONS: Array<{ value: Direction; label: string; icon: 'ChevronLeft' | 'ChevronRight' | 'ChevronUp' | 'Chevron' }> = [
  { value: 'left', label: 'Left', icon: 'ChevronLeft' },
  { value: 'right', label: 'Right', icon: 'ChevronRight' },
  { value: 'top', label: 'Up', icon: 'ChevronUp' },
  { value: 'bottom', label: 'Down', icon: 'Chevron' },
]

/** One undo step for a scrub of `proto.interaction.set` (upstream `begin_interaction("Edit")`). */
function useScrub(ids: number[], index: number, fields: (value: number) => Record<string, unknown>): ScrubHandlers {
  const { interaction } = useDesignKit()
  const live = useRef<ReturnType<typeof interaction> | null>(null)
  const latest = useRef(fields)
  latest.current = fields
  return {
    begin: () => {
      live.current = interaction('Edit')
    },
    move: (value) => live.current?.update('proto.interaction.set', { ids, index, ...latest.current(value) }),
    end: (cancelled) => {
      const now = live.current
      live.current = null
      void (cancelled ? now?.cancel() : now?.commit())
    },
  }
}

/** The transition, its direction, the easing and duration, custom curve fields and the curve preview (`animation_rows`). */
function AnimationRows({ interaction, patch, ids, index }: { interaction: Interaction; patch: Patch; ids: number[]; index: number }) {
  const { t } = useDesignKit()
  const { transition, easing } = interaction
  const duration = interaction.duration_ms
  const clampDuration = (v: number) => Math.round(Math.min(10000, Math.max(1, v)))
  const durationScrub = useScrub(ids, index, (v) => ({ durationMs: clampDuration(v) }))
  const transitionOptions = TRANSITIONS.flatMap(([value, label], i) => [...(i === 3 ? [{ separator: true as const }] : []), { value, label: t(label) }])
  const easingItems = (): RichItem[] => {
    const all: Easing[] = [...EASINGS.map(([id]) => id), CUSTOM_BEZIER, CUSTOM_SPRING]
    return all.flatMap((e, i) => {
      const kind = easingKind(e)
      const row: RichItem = {
        key: String(kind),
        height: 26,
        testId: `design-proto-easing-${kind}`,
        onSelect: () => patch({ easing: easingKind(easing) === kind ? easing : e }),
        render: (active) => (
          <>
            <CurveThumb easing={e} color={active ? '#ffffff' : 'var(--design-text)'} />
            <span className="design-menu-label design-proto-easing-label">{t(easingLabel(e))}</span>
          </>
        ),
      }
      return i === 7 || i === EASINGS.length ? [{ key: `sep${i}`, separator: true as const }, row] : [row]
    })
  }
  return (
    <>
      <SubLabels labels={[t('Animation')]} />
      <Row>
        <Dropdown<TransitionType> value={transition.type} width="100%" ariaLabel={t('Animation')} testId="design-proto-transition" options={transitionOptions} onChange={(type) => patch({ transition: transitionOf(type, transition) })} />
      </Row>
      {hasDirection(transition.type) ? (
        <Row>
          <Segmented
            value={transition.direction ?? 'left'}
            cell={32}
            ariaLabel={t('Direction')}
            testId="design-proto-direction"
            segments={DIRECTIONS.map((d) => ({ value: d.value, label: t(d.label), icon: d.icon }))}
            onChange={(direction) => patch({ transition: { ...transition, direction } })}
          />
        </Row>
      ) : null}
      {transition.type !== 'instant' ? (
        <>
          <Row>
            <RichDropdown label={t(easingLabel(easing))} width="calc(100% - 80px)" menuWidth={220} ariaLabel={t('Easing')} testId="design-proto-easing" items={easingItems} />
            {isSpring(easing) ? (
              <span className="design-proto-spring-ms" data-tip={t('A spring plays until it settles: its mass, stiffness and damping set how long')} data-testid="design-proto-spring-ms">
                {`${Math.round(easingSeconds(easing, duration) * 1000)} ms`}
              </span>
            ) : (
              <NumberField value={duration} suffix="ms" width={72} step={10} min={1} max={10000} decimals={0} onCommit={(edit) => patch({ durationMs: clampDuration(numberOf(edit, duration)) })} scrub={durationScrub} testId="design-proto-duration" />
            )}
          </Row>
          <CustomEasingFields easing={easing} patch={patch} ids={ids} index={index} />
          <CurvePreview easing={easing} duration={duration} />
        </>
      ) : null}
    </>
  )
}

/** Fields for a custom bezier (X1, Y1, X2, Y2) or spring (mass, stiffness, damping) (`custom_easing_fields`). */
function CustomEasingFields({ easing, patch, ids, index }: { easing: Easing; patch: Patch; ids: number[]; index: number }) {
  const { t } = useDesignKit()
  const bezier = typeof easing === 'object' && 'customBezier' in easing ? easing.customBezier : null
  const springValues = typeof easing === 'object' && 'customSpring' in easing ? easing.customSpring : null
  const values = bezier ? [bezier.x1, bezier.y1, bezier.x2, bezier.y2] : springValues ? [springValues.mass, springValues.stiffness, springValues.damping] : []
  const make = (next: number[]): Easing =>
    bezier
      ? { customBezier: { x1: clamp(next[0]!, 0, 1), y1: clamp(next[1]!, -5, 5), x2: clamp(next[2]!, 0, 1), y2: clamp(next[3]!, -5, 5) } }
      : { customSpring: { mass: clamp(next[0]!, 0.01, 100), stiffness: clamp(next[1]!, 1, 10000), damping: clamp(next[2]!, 0, 1000) } }
  const scrubbers = [0, 1, 2, 3].map((i) =>
    // eslint-disable-next-line react-hooks/rules-of-hooks
    useScrub(ids, index, (v) => ({ easing: make(values.map((x, j) => (j === i ? v : x))) })),
  )
  if (!values.length) return null
  const names = bezier ? ['X1', 'Y1', 'X2', 'Y2'] : ['Mass', 'Stiffness', 'Damping']
  const steps = bezier ? [0.01, 0.01, 0.01, 0.01] : [0.1, 1, 0.5]
  return (
    <>
      <Row>
        <div className="design-proto-easing-fields" data-count={values.length}>
          {values.map((value, i) => (
            <NumberField
              key={names[i]}
              label={bezier ? names[i] : undefined}
              ariaLabel={t(names[i]!)}
              value={Math.round(value * 100) / 100}
              step={steps[i]}
              decimals={2}
              width="100%"
              onCommit={(edit) => patch({ easing: make(values.map((x, j) => (j === i ? numberOf(edit, x) : x))) })}
              scrub={scrubbers[i]}
              testId={`design-proto-easing-field-${i}`}
            />
          ))}
        </div>
      </Row>
      {springValues ? <SubLabels labels={[t('Mass · Stiffness · Damping')]} /> : null}
    </>
  )
}

const clamp = (v: number, lo: number, hi: number) => (Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : lo)

/** The curve as an SVG path in a `w` × `h` box (x = time, y = progress up). */
function curvePath(easing: Easing, w: number, h: number) {
  const n = 48
  const points: string[] = []
  for (let i = 0; i <= n; i += 1) {
    const x = i / n
    points.push(`${(w * x).toFixed(2)},${(h - h * easingEval(easing, x)).toFixed(2)}`)
  }
  return `M${points.join('L')}`
}

/** The easing menu's 28 × 18 curve thumbnail. */
function CurveThumb({ easing, color }: { easing: Easing; color: string }) {
  return (
    <svg className="design-proto-curve-thumb" width="28" height="18" viewBox="0 0 28 18" overflow="visible" aria-hidden="true">
      <path d={curvePath(easing, 28, 18)} fill="none" stroke={color} strokeWidth="1.5" />
    </svg>
  )
}

/** How long the dot rests at the end each time round (s), and how often it plays when shown or changed. */
const CURVE_HOLD = 0.6
const CURVE_PLAYS = 2

/** The curve preview (72 tall): a dot runs along the curve twice when shown or changed, and loops while hovered. */
function CurvePreview({ easing, duration }: { easing: Easing; duration: number }) {
  const box = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState({ w: 216, h: 40 })
  const [phase, setPhase] = useState(1)
  const [hovered, setHovered] = useState(false)
  const key = `${JSON.stringify(easing)}:${duration}`
  useLayoutEffect(() => {
    const el = box.current
    if (el) setSize({ w: el.clientWidth - 20, h: el.clientHeight - 24 })
  }, [])
  useEffect(() => {
    const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches
    if (reduce) {
      setPhase(1)
      return undefined
    }
    const period = Math.max(0.05, easingSeconds(easing, duration)) + CURVE_HOLD
    const start = performance.now()
    let frame = 0
    const tick = (now: number) => {
      const elapsed = (now - start) / 1000
      if (!hovered && elapsed >= period * CURVE_PLAYS) {
        setPhase(1)
        return
      }
      setPhase(Math.min(1, (elapsed % period) / (period - CURVE_HOLD)))
      frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frame)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, hovered])
  const { w, h } = size
  const x = w * phase
  const y = h - h * easingEval(easing, phase)
  return (
    <div className="design-proto-curve-row">
      <div ref={box} className="design-proto-curve" onPointerEnter={() => setHovered(true)} onPointerLeave={() => setHovered(false)} data-testid="design-proto-curve">
        <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} overflow="visible" aria-hidden="true">
          <path d={curvePath(easing, w, h)} fill="none" stroke="var(--design-accent-line)" strokeWidth="1.5" />
          <circle cx={x} cy={y} r="3" fill="var(--design-accent-line)" />
        </svg>
      </div>
    </div>
  )
}
