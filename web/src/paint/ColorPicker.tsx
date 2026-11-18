/**
 * The colour picker popover (2.4 "Colour picker popover", upstream `widgets/colorpicker.rs`): paint
 * type (solid, linear, radial, angular, diamond gradient, image, pattern), the paint's blend mode,
 * the gradient stop bar, image and pattern controls, the saturation/brightness square, the hue and
 * alpha sliders with the eyedropper, the colour model fields (Hex, RGB, HSL, HSB, LCH, OKLCH, CSS)
 * and alpha, the gamut note, and the page's and recent colours.
 *
 * It is a controlled component other packages import: it shows `paint` and calls
 * `onChange(paint, phase)` for every edit, `phase` `live` while a drag goes on, `done` when an
 * edit is complete (a click, a typed value, the end of a drag), and `cancel` when Escape cancels a
 * scrub (with the paint from before it). `TargetColorPicker` (in
 * `PaintPicker.tsx`) connects it to the engine's paints.
 */
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { useDesignKit } from '../kit/hooks'
import { Icon, type IconName } from '../icons/Icon'
import { Button, Dropdown, IconButton, NumberField, Segmented, TextField, showToast, type DropdownOption, type NumberEdit, type ScrubHandlers } from '../ui'
import {
  type Color,
  cssColor,
  fromHexOrName,
  fromHsb,
  fromHsl,
  fromLch,
  fromOklch,
  type Hsb,
  parseCss,
  rgba8,
  toCss,
  toCssHex,
  toHex,
  toHsb,
  toHsl,
  toLch,
  toOklch,
  withAlpha,
  color as makeColor,
} from './color'
import { BLEND_BREAKS, BLEND_MODES, barStripes, defaultGradient, editColor, type GradientKind, type Paint, previewColor, sample } from './paint'
import { PaintSwatch } from './Swatch'

export type ChangePhase = 'live' | 'done' | 'cancel'

export type PatternSource = { id: number; name: string }

export type ImagePreset = { name: string; filters: Record<string, number> }

export type ColorPickerProps = {
  paint: Paint
  onChange: (paint: Paint, phase: ChangePhase) => void
  onClose: () => void
  /** Shows the paint type and blend rows (not for effect, page, variable targets). */
  kinds?: boolean
  /** The header's title ("Custom"). */
  title?: string
  /** Where the window's top-left goes (client px); beside the right panel by default. */
  position?: { left: number; top: number }
  /** The page's colours ("On this page"). */
  documentColors?: Color[]
  /** Recent colours. */
  recent?: Color[]
  /** The selected gradient stop (shared with the canvas handles). */
  stop?: number
  onStop?: (stop: number) => void
  /** The eyedropper button (`Pick color (I)`); without it the button does not show. */
  onEyedropper?: () => void
  /** The document's colours are Display P3: warn for colours sRGB can't show. */
  displayP3?: boolean
  /** Choosing the Image type: pick an image (the picker writes nothing itself). */
  onChooseImage?: () => void
  /** Layers a pattern can tile (the first is used when Pattern is chosen). */
  patternSources?: PatternSource[]
  /** Saved image adjustment presets, and what to do with presets. */
  imagePresets?: ImagePreset[]
  onImagePreset?: (action: { apply: string } | { save: string; filters: Record<string, number> } | { remove: string }) => void
  /** The image's colour profile line (`Color profile: …`), when known. */
  imageProfile?: string
  /** Extra rows under the gradient stop buttons (the stop's variable chip). */
  stopExtra?: ReactNode
  testId?: string
}

const KINDS: Array<{ value: string; label: string; icon: IconName }> = [
  { value: 'solid', label: 'Solid', icon: 'Fill' },
  { value: 'linear', label: 'Linear', icon: 'GradientLinear' },
  { value: 'radial', label: 'Radial', icon: 'GradientRadial' },
  { value: 'angular', label: 'Angular', icon: 'GradientAngular' },
  { value: 'diamond', label: 'Diamond', icon: 'GradientDiamond' },
  { value: 'image', label: 'Image', icon: 'Image' },
  { value: 'pattern', label: 'Pattern', icon: 'ViewGrid' },
]

const MODELS = ['Hex', 'RGB', 'HSL', 'HSB', 'LCH', 'OKLCH', 'CSS'] as const
type Model = (typeof MODELS)[number]
/** The model is UI state shared by every picker (upstream's `cp_model`). */
let lastModel: Model = 'Hex'

const FILTERS = ['Exposure', 'Contrast', 'Saturation', 'Temperature', 'Tint', 'Highlights', 'Shadows'] as const
const BUILT_IN_PRESETS: Array<[string, string, Record<string, number>]> = [
  ['vivid', 'Vivid', { contrast: 0.2, saturation: 0.45, highlights: -0.1, shadows: 0.1 }],
  ['muted', 'Muted', { contrast: -0.15, saturation: -0.45 }],
  ['warm', 'Warm', { temperature: 0.5, tint: 0.1, saturation: 0.1 }],
  ['cool', 'Cool', { temperature: -0.5, tint: -0.05 }],
  ['blackAndWhite', 'Black & white', { saturation: -1, contrast: 0.15 }],
  ['fade', 'Fade', { contrast: -0.35, saturation: -0.2, shadows: 0.35, highlights: -0.1 }],
]

const filterKey = (label: string) => label.toLowerCase()
const sameFilters = (a: Record<string, number>, b: Record<string, number>) => FILTERS.every((f) => Math.abs((a[filterKey(f)] ?? 0) - (b[filterKey(f)] ?? 0)) < 1e-4)

/** The default position: left of the Design panel, 100 px under its top (upstream: 140 in a window with a 40 px tab strip). */
export function besideRightPanel(): { left: number; top: number } {
  const right = document.querySelector('.design-right')?.getBoundingClientRect()
  if (!right) return { left: Math.max(8, window.innerWidth - 500), top: 100 }
  return { left: Math.max(8, right.left - 260), top: right.top + 100 }
}

/** Calls `move` with the pointer's position in `el` (0..1 each way) while dragged, then `end`. */
function dragIn(event: ReactPointerEvent<HTMLElement>, move: (x: number, y: number, first: boolean) => void, end: () => void) {
  if (event.button !== 0) return
  event.preventDefault()
  const el = event.currentTarget
  const rect = el.getBoundingClientRect()
  const at = (e: { clientX: number; clientY: number }, first: boolean) =>
    move(Math.min(1, Math.max(0, (e.clientX - rect.left) / Math.max(1, rect.width))), Math.min(1, Math.max(0, (e.clientY - rect.top) / Math.max(1, rect.height))), first)
  try {
    el.setPointerCapture(event.pointerId)
  } catch {
    // A synthetic pointer: the document listeners still follow it.
  }
  at(event, true)
  const onMove = (e: PointerEvent) => at(e, false)
  const onUp = () => {
    el.removeEventListener('pointermove', onMove)
    el.removeEventListener('pointerup', onUp)
    el.removeEventListener('pointercancel', onUp)
    end()
  }
  el.addEventListener('pointermove', onMove)
  el.addEventListener('pointerup', onUp)
  el.addEventListener('pointercancel', onUp)
}

export function ColorPicker(props: ColorPickerProps) {
  const { t } = useDesignKit()
  const { paint, onChange, onClose, kinds = true, title, documentColors = [], recent = [], testId = 'design-color-picker' } = props
  const ref = useRef<HTMLDivElement>(null)
  const [at, setAt] = useState(props.position ?? null)
  const [internalStop, setInternalStop] = useState(0)
  const stops = paint.gradient?.stops ?? []
  const stop = Math.min(props.stop ?? internalStop, Math.max(0, stops.length - 1))
  const setStop = (i: number) => (props.onStop ? props.onStop(i) : setInternalStop(i))

  useLayoutEffect(() => {
    setAt(props.position ?? besideRightPanel())
  }, [props.position])

  // Escape closes (unless a menu or field inside handles it first).
  const latestClose = useRef(onClose)
  latestClose.current = onClose
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      const layers = document.querySelectorAll('.design-layer')
      if (layers[layers.length - 1] !== ref.current) return
      event.preventDefault()
      event.stopPropagation()
      latestClose.current()
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [])

  // The colour the square, sliders and fields edit: a solid's, or the selected stop's.
  const edited: Color | null = paint.type === 'solid' ? editColor(paint) : paint.type === 'gradient' ? (stops[stop]?.color ?? null) : null

  /** `paint` with `c` as the edited colour. */
  const withColor = (c: Color): Paint => {
    if (paint.type === 'solid') return { ...paint, color: withAlpha(c, 1), opacity: c.a }
    if (paint.type === 'gradient' && paint.gradient) {
      const next = paint.gradient.stops.map((s, i) => (i === stop ? { ...s, color: c } : s))
      return { ...paint, gradient: { ...paint.gradient, stops: next } }
    }
    return paint
  }

  const chooseKind = (kind: string) => {
    const base = previewColor(paint)
    if (kind === 'solid') return onChange({ ...paint, type: 'solid', color: withAlpha(base, 1), gradient: null, image: null, pattern: null }, 'done')
    if (kind === 'image') return props.onChooseImage?.()
    if (kind === 'pattern') {
      const source = props.patternSources?.[0]
      if (!source) {
        showToast(t('Add a layer or component to tile first'))
        return
      }
      return onChange({ ...paint, type: 'pattern', gradient: null, image: null, pattern: { source: source.id, tile: 'grid', scale: 1, spacing: [0, 0], align: 'start' } }, 'done')
    }
    const k = kind as GradientKind
    const fresh = defaultGradient(k, base)
    const gradient = paint.gradient && paint.gradient.stops.length >= 2 ? { ...fresh, stops: paint.gradient.stops } : fresh
    onChange({ ...paint, type: 'gradient', gradient, image: null, pattern: null }, 'done')
  }

  const active = paint.type === 'gradient' ? (paint.gradient?.kind ?? 'linear') : paint.type
  const blendOptions: DropdownOption<string>[] = []
  for (const [id, label] of BLEND_MODES) {
    if (id === 'passThrough') continue
    blendOptions.push({ value: id, label: t(label) })
    if (BLEND_BREAKS.has(id)) blendOptions.push({ separator: true })
  }

  if (!at) return null
  return createPortal(
    <div ref={ref} className="design-layer design-cp" style={{ left: at.left, top: at.top }} role="dialog" aria-label={title ?? t('Custom')} data-testid={testId} onPointerDown={(event) => event.stopPropagation()}>
      <div className="design-cp-header">
        <span className="design-s11">{title ?? t('Custom')}</span>
        <button type="button" className="design-cp-close" aria-label={t('Close')} data-testid={`${testId}-close`} onClick={onClose}>
          <Icon name="Close" size={12} />
        </button>
      </div>
      {kinds ? (
        <>
          <div className="design-cp-row" style={{ paddingLeft: 12 }}>
            <Segmented value={active} cell={30} segments={KINDS.map((k) => ({ value: k.value, label: t(k.label), icon: k.icon }))} onChange={chooseKind} testId={`${testId}-kind`} ariaLabel={t('Paint type')} />
          </div>
          <div className="design-cp-row" data-tip={t('Blend mode')}>
            <Dropdown checks={false} value={paint.blend} options={blendOptions} icon="Blend" width={208} menuWidth={200} onChange={(blend) => onChange({ ...paint, blend }, 'done')} ariaLabel={t('Blend mode')} testId={`${testId}-blend`} />
          </div>
        </>
      ) : null}
      {paint.type === 'gradient' && paint.gradient ? (
        <GradientBar
          paint={paint}
          stop={stop}
          onStop={setStop}
          onChange={onChange}
          extra={props.stopExtra}
          testId={testId}
        />
      ) : null}
      {paint.type === 'image' && paint.image ? <ImageControls paint={paint} onChange={onChange} presets={props.imagePresets ?? []} onPreset={props.onImagePreset} profile={props.imageProfile} testId={testId} /> : null}
      {paint.type === 'pattern' && paint.pattern ? <PatternControls paint={paint} onChange={onChange} sources={props.patternSources ?? []} testId={testId} /> : null}
      {edited ? <ColorArea color={edited} onColor={(c, phase) => onChange(withColor(c), phase)} onEyedropper={props.onEyedropper} displayP3={props.displayP3} testId={testId} /> : null}
      {documentColors.length ? (
        <>
          <div className="design-cp-label">{t('On this page')}</div>
          <Swatches colors={documentColors.slice(0, 21)} onPick={(c) => edited && onChange(withColor(c), 'done')} testId={`${testId}-page`} />
        </>
      ) : null}
      {recent.length ? (
        <>
          <div className="design-cp-label">{t('Recent')}</div>
          <Swatches colors={recent.slice(0, 14)} onPick={(c) => edited && onChange(withColor(c), 'done')} testId={`${testId}-recent`} />
        </>
      ) : null}
    </div>,
    document.body,
  )
}

function Swatches({ colors, onPick, testId }: { colors: Color[]; onPick: (c: Color) => void; testId: string }) {
  return (
    <div className="design-cp-swatches" data-testid={testId}>
      {colors.map((c, i) => (
        <button key={`${toHex(c)}-${i}`} type="button" className="design-cp-swatch" data-tip={toHex(c)} aria-label={toHex(c)} data-color={toCssHex(c)} onClick={() => onPick(c)}>
          <PaintSwatch color={c} size={14} />
        </button>
      ))}
    </div>
  )
}

/** The saturation/brightness square, hue and alpha sliders, the model fields and the gamut note. */
function ColorArea({ color, onColor, onEyedropper, displayP3, testId }: { color: Color; onColor: (c: Color, phase: ChangePhase) => void; onEyedropper?: () => void; displayP3?: boolean; testId: string }) {
  const { t } = useDesignKit()
  // The hue and saturation stay while the colour is theirs (a grey keeps its hue).
  const kept = useRef<{ hsb: Hsb; key: string } | null>(null)
  const key = rgba8(color).join()
  const hsb = kept.current && kept.current.key === key ? kept.current.hsb : toHsb(color)
  const latest = useRef({ hsb, alpha: color.a })
  latest.current = { hsb, alpha: color.a }
  const emit = (next: Hsb, alpha: number, phase: ChangePhase) => {
    const c = fromHsb(next, alpha)
    kept.current = { hsb: next, key: rgba8(c).join() }
    latest.current = { hsb: next, alpha }
    onColor(c, phase)
  }
  const hue = cssColor(fromHsb({ h: hsb.h, s: 1, b: 1 }))
  return (
    <>
      <div
        className="design-cp-sv"
        style={{ background: `linear-gradient(to bottom, transparent, #000), linear-gradient(to right, #fff, ${hue})` }}
        data-testid={`${testId}-sv`}
        onPointerDown={(event) =>
          dragIn(
            event,
            (x, y) => emit({ h: latest.current.hsb.h, s: x, b: 1 - y }, latest.current.alpha, 'live'),
            () => emit(latest.current.hsb, latest.current.alpha, 'done'),
          )
        }
      >
        <span className="design-cp-knob" style={{ left: `${hsb.s * 100}%`, top: `${(1 - hsb.b) * 100}%` }} />
      </div>
      <div className="design-cp-slider-row" style={{ height: 28 }}>
        {onEyedropper ? (
          <span className="design-cp-eyedropper">
            <IconButton icon="Eyedropper" label={t('Pick color (I)')} onClick={onEyedropper} testId={`${testId}-eyedropper`} />
          </span>
        ) : null}
        <div
          className="design-cp-slider design-cp-hue"
          data-testid={`${testId}-hue`}
          onPointerDown={(event) =>
            dragIn(
              event,
              (x) => emit({ ...latest.current.hsb, h: Math.min(359.9, x * 360) }, latest.current.alpha, 'live'),
              () => emit(latest.current.hsb, latest.current.alpha, 'done'),
            )
          }
        >
          <span className="design-cp-ring" style={{ left: `${(hsb.h / 360) * 100}%` }} />
        </div>
      </div>
      <div className="design-cp-slider-row design-cp-alpha-row" style={{ height: 20 }}>
        <div
          className="design-cp-slider design-cp-alpha"
          data-testid={`${testId}-alpha`}
          onPointerDown={(event) =>
            dragIn(
              event,
              (x) => emit(latest.current.hsb, x, 'live'),
              () => emit(latest.current.hsb, latest.current.alpha, 'done'),
            )
          }
        >
          <span className="design-cp-alpha-fill" style={{ background: `linear-gradient(to right, transparent, ${cssColor(color, 1)})` }} />
          <span className="design-cp-ring" style={{ left: `${color.a * 100}%` }} />
        </div>
      </div>
      <ModelFields
        color={color}
        onColor={(c, phase) => {
          kept.current = null
          onColor(c, phase)
        }}
        testId={testId}
      />
      {displayP3 && outsideSrgb(color) ? (
        <div className="design-cp-row design-cp-gamut" data-tip="sRGB screens and files can't show this Display P3 color: the canvas shows the nearest sRGB color, P3 exports keep it">
          <Icon name="GamutWarning" size={14} />
          <span>Outside sRGB</span>
        </div>
      ) : null}
    </>
  )
}

/** A Display P3 colour sRGB can't show (P3 → sRGB leaves 0..1). */
function outsideSrgb(c: Color): boolean {
  const lin = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)
  const [r, g, b] = [lin(c.r), lin(c.g), lin(c.b)]
  const out = [
    1.2249401 * r - 0.2249404 * g + 0 * b,
    -0.0420569 * r + 1.0420571 * g + 0 * b,
    -0.0196376 * r - 0.0786361 * g + 1.0982735 * b,
  ]
  return out.some((v) => v < -1e-4 || v > 1 + 1e-4)
}

type Field = { id: string; decimals: number; step: number; max: number; wraps: boolean }
const field = (id: string, decimals: number, step: number, max: number): Field => ({ id, decimals, step, max, wraps: false })
const hueField = (id: string): Field => ({ id, decimals: 0, step: 1, max: 360, wraps: true })
const POLAR: Record<'HSL' | 'HSB' | 'LCH' | 'OKLCH', { fields: Field[]; values: (c: Color) => number[]; color: (v: number[]) => Color }> = {
  HSL: {
    fields: [hueField('h'), field('s', 0, 1, 100), field('l', 0, 1, 100)],
    values: (c) => {
      const x = toHsl(c)
      return [x.h, x.s * 100, x.l * 100]
    },
    color: ([h, s, l]) => fromHsl({ h, s: s / 100, l: l / 100 }),
  },
  HSB: {
    fields: [hueField('h'), field('s', 0, 1, 100), field('b', 0, 1, 100)],
    values: (c) => {
      const x = toHsb(c)
      return [x.h, x.s * 100, x.b * 100]
    },
    color: ([h, s, b]) => fromHsb({ h, s: s / 100, b: b / 100 }),
  },
  LCH: {
    fields: [field('l', 0, 1, 100), field('c', 0, 1, 230), hueField('h')],
    values: (c) => toLch(c),
    color: ([l, c, h]) => fromLch([l, c, h]),
  },
  OKLCH: {
    fields: [field('l', 0, 1, 100), field('c', 2, 0.01, 0.5), hueField('h')],
    values: (c) => {
      const [l, ch, h] = toOklch(c)
      return [l * 100, ch, h]
    },
    color: ([l, c, h]) => fromOklch([l / 100, c, h]),
  },
}

/** A number field of the model row: typed values and scrubs (`null` ends a scrub, or cancels it) as colour edits. */
function ModelNumber({ value, decimals, step, width, label, suffix, onValue, testId }: { value: number; decimals: number; step: number; width: number; label: string; suffix?: string; onValue: (v: number | null, phase: ChangePhase) => void; testId?: string }) {
  const p = 10 ** decimals
  const shown = Math.round(value * p) / p
  const latest = useRef(onValue)
  latest.current = onValue
  const scrub = useMemo<ScrubHandlers>(() => ({ begin: () => undefined, move: (v) => latest.current(v, 'live'), end: (cancelled) => latest.current(null, cancelled ? 'cancel' : 'done') }), [])
  return <NumberField value={shown} decimals={decimals} step={step} width={width} suffix={suffix} onCommit={(edit: NumberEdit) => onValue(edit.kind === 'set' ? edit.value : edit.apply(shown), 'done')} scrub={scrub} ariaLabel={label} testId={testId} />
}

function ModelFields({ color, onColor, testId }: { color: Color; onColor: (c: Color, phase: ChangePhase) => void; testId: string }) {
  const { t } = useDesignKit()
  const [model, setModel] = useState<Model>(lastModel)
  const wide = model === 'LCH' || model === 'OKLCH'
  // Polar values typed are kept while the colour is theirs (a clipped chroma or a grey's hue).
  const kept = useRef<{ model: Model; values: number[]; key: string } | null>(null)
  const key = rgba8(color).join()
  const last = useRef<Color>(color)
  last.current = color
  const scrubbed = useRef<Color | null>(null)
  // The colour before the scrub, which a cancelled one puts back.
  const before = useRef<Color | null>(null)
  /** A new colour; null ends a scrub with the last one it showed (or, cancelled, the one before it). */
  const emit = (c: Color | null, phase: ChangePhase) => {
    if (phase === 'live' && c) {
      if (!scrubbed.current) before.current = last.current
      scrubbed.current = c
      onColor(c, 'live')
    } else if (phase === 'cancel') {
      const start = before.current ?? last.current
      scrubbed.current = null
      before.current = null
      onColor(start, 'cancel')
    } else {
      before.current = null
      const final = c ?? scrubbed.current ?? last.current
      scrubbed.current = null
      onColor(final, 'done')
    }
  }
  let fields: ReactNode
  if (model === 'RGB') {
    const [r, g, b] = rgba8(color)
    const vals = [r, g, b]
    fields = vals.map((v, k) => (
      <ModelNumber
        key={k}
        value={v}
        decimals={0}
        step={1}
        width={31}
        label={['R', 'G', 'B'][k]}
        testId={`${testId}-rgb-${k}`}
        onValue={(x, phase) => {
          if (x === null) return emit(null, phase)
          const next = [...vals]
          next[k] = Math.min(255, Math.max(0, x))
          emit(makeColor(next[0] / 255, next[1] / 255, next[2] / 255, color.a), phase)
        }}
      />
    ))
  } else if (model === 'HSL' || model === 'HSB' || model === 'LCH' || model === 'OKLCH') {
    const spec = POLAR[model]
    const vals = kept.current && kept.current.model === model && kept.current.key === key ? kept.current.values : spec.values(color)
    fields = spec.fields.map((f, k) => (
      <ModelNumber
        key={`${model}-${f.id}`}
        value={vals[k]}
        decimals={f.decimals}
        step={f.step}
        width={!wide ? 31 : f.decimals > 0 ? 30 : 27}
        label={f.id.toUpperCase()}
        testId={`${testId}-${model.toLowerCase()}-${f.id}`}
        onValue={(x, phase) => {
          if (x === null) return emit(null, phase)
          const next = [...vals]
          next[k] = f.wraps ? ((x % 360) + 360) % 360 : Math.min(f.max, Math.max(0, x))
          const c = withAlpha(spec.color(next), color.a)
          kept.current = { model, values: next, key: rgba8(c).join() }
          emit(c, phase)
        }}
      />
    ))
  } else if (model === 'CSS') {
    fields = (
      <TextField
        value={toCss(color)}
        width={152}
        ariaLabel="CSS"
        testId={`${testId}-css`}
        onCommit={(text) => {
          const c = parseCss(text)
          if (c) emit(c, 'done')
        }}
      />
    )
  } else {
    fields = (
      <TextField
        value={toHex(color)}
        width={101}
        ariaLabel={t('Hex')}
        testId={`${testId}-hex`}
        onCommit={(text) => {
          const c = fromHexOrName(text)
          if (c) emit(withAlpha(c, c.a < 1 ? c.a : color.a), 'done')
        }}
      />
    )
  }
  return (
    <div className="design-cp-row design-cp-models">
      <Dropdown checks={false}
        value={model}
        options={MODELS.map((m) => ({ value: m, label: m }))}
        width={wide ? 66 : 52}
        onChange={(m) => {
          lastModel = m
          setModel(m)
        }}
        ariaLabel={t('Color model')}
        testId={`${testId}-model`}
      />
      {fields}
      {model !== 'CSS' ? (
        <ModelNumber
          value={Math.round(color.a * 100)}
          decimals={0}
          step={1}
          width={wide ? 40 : 44}
          label={t('Opacity')}
          suffix="%"
          testId={`${testId}-alpha-field`}
          onValue={(x, phase) => (x === null ? emit(null, phase) : emit(withAlpha(color, Math.min(100, Math.max(0, x)) / 100), phase))}
        />
      ) : null}
    </div>
  )
}

/** The gradient stop bar, its buttons, and the selected stop's chip. */
function GradientBar({ paint, stop, onStop, onChange, extra, testId }: { paint: Paint; stop: number; onStop: (i: number) => void; onChange: (p: Paint, phase: ChangePhase) => void; extra?: ReactNode; testId: string }) {
  const { t } = useDesignKit()
  const g = paint.gradient!
  const bar = useRef<HTMLDivElement>(null)
  const set = (stops = g.stops, more: Partial<typeof g> = {}, phase: ChangePhase = 'done') => onChange({ ...paint, gradient: { ...g, ...more, stops } }, phase)
  const selected = g.stops[stop]
  return (
    <>
      <div className="design-cp-bar-row">
        <div
          ref={bar}
          className="design-cp-bar"
          data-testid={`${testId}-bar`}
          onPointerDown={(event) => {
            if (event.button !== 0 || event.target !== event.currentTarget) return
            const r = event.currentTarget.getBoundingClientRect()
            const pos = Math.min(1, Math.max(0, (event.clientX - r.left) / r.width))
            const stops = [...g.stops, { position: pos, color: sample(g, pos) }]
            onStop(stops.length - 1)
            set(stops)
          }}
        >
          <span className="design-cp-bar-fill" style={{ background: barStripes(g, 24) }} />
          {g.stops.map((s, i) => (
            <span
              key={i}
              className="design-cp-stop"
              data-selected={i === stop || undefined}
              data-testid={`${testId}-stop-${i}`}
              style={{ left: `${s.position * 100}%`, background: cssColor(s.color, 1) }}
              onPointerDown={(event) => {
                event.stopPropagation()
                if (event.button !== 0) return
                onStop(i)
                const r = bar.current!.getBoundingClientRect()
                const el = event.currentTarget
                const start = event.clientX
                let moved = false
                let stops = g.stops
                try {
                  el.setPointerCapture(event.pointerId)
                } catch {
                  // Synthetic pointer.
                }
                const move = (e: PointerEvent) => {
                  if (!moved && Math.abs(e.clientX - start) < 2) return
                  moved = true
                  const pos = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width))
                  stops = stops.map((x, k) => (k === i ? { ...x, position: pos } : x))
                  set(stops, {}, 'live')
                }
                const up = () => {
                  el.removeEventListener('pointermove', move)
                  el.removeEventListener('pointerup', up)
                  el.removeEventListener('pointercancel', up)
                  if (moved) set(stops, {}, 'done')
                }
                el.addEventListener('pointermove', move)
                el.addEventListener('pointerup', up)
                el.addEventListener('pointercancel', up)
              }}
            />
          ))}
        </div>
      </div>
      <div className="design-cp-row design-cp-small-buttons">
        <button
          type="button"
          className="design-cp-small"
          data-testid={`${testId}-remove-stop`}
          onClick={() => {
            if (g.stops.length <= 2) return
            onStop(0)
            set(g.stops.filter((_, i) => i !== stop))
          }}
        >
          {t('Remove stop')}
        </button>
        <button type="button" className="design-cp-small" data-testid={`${testId}-flip`} onClick={() => set(g.stops.map((s) => ({ ...s, position: 1 - s.position })))}>
          {t('Flip')}
        </button>
        <button
          type="button"
          className="design-cp-small"
          data-testid={`${testId}-rotate`}
          onClick={() => {
            const [fx, fy] = g.from
            const rot = ([x, y]: [number, number]): [number, number] => [fx - (y - fy), fy + (x - fx)]
            set(g.stops, { to: rot(g.to), width: rot(g.width) })
          }}
        >
          {t('Rotate 90°')}
        </button>
      </div>
      {selected ? (
        <div className="design-cp-row">
          {extra ?? (
            <div className="design-cp-stop-chip" data-testid={`${testId}-stop-chip`}>
              <PaintSwatch color={selected.color} size={14} />
              <span>{t('Stop {n}  {hex}', { n: stop + 1, hex: toHex(selected.color) })}</span>
            </div>
          )}
        </div>
      ) : null}
    </>
  )
}

/** The image paint's scale mode, rotation, profile, presets and adjustment sliders. */
function ImageControls({ paint, onChange, presets, onPreset, profile, testId }: { paint: Paint; onChange: (p: Paint, phase: ChangePhase) => void; presets: ImagePreset[]; onPreset?: ColorPickerProps['onImagePreset']; profile?: string; testId: string }) {
  const { t } = useDesignKit()
  const img = paint.image!
  const filters = (img.filters ?? {}) as Record<string, number>
  const mode = String(img.scale_mode ?? 'fill')
  const setImage = (next: Record<string, unknown>, phase: ChangePhase = 'done') => onChange({ ...paint, image: { ...img, ...next } }, phase)
  const identity = FILTERS.every((f) => Math.abs(filters[filterKey(f)] ?? 0) < 1e-6)
  const current =
    BUILT_IN_PRESETS.map(([, label, f]) => [t(label), f] as const)
      .concat(presets.map((p) => [p.name, p.filters] as const))
      .find(([, f]) => sameFilters(f, filters))?.[0] ?? (identity ? t('No preset') : t('Custom'))
  const [naming, setNaming] = useState<string | null>(null)
  const apply = (name: string) => {
    if (onPreset) onPreset({ apply: name })
    else {
      const preset = BUILT_IN_PRESETS.find(([id]) => id === name)?.[2] ?? presets.find((p) => p.name === name)?.filters ?? {}
      setImage({ filters: Object.fromEntries(FILTERS.map((f) => [filterKey(f), name === 'reset' ? 0 : (preset[filterKey(f)] ?? 0)])) })
    }
  }
  const presetOptions: DropdownOption<string>[] = [
    ...BUILT_IN_PRESETS.map(([id, label]) => ({ value: `built:${id}`, label: t(label) })),
    { separator: true as const },
    ...presets.map((p) => ({ value: `saved:${p.name}`, label: p.name })),
    ...(presets.length ? [{ separator: true as const }] : []),
    { value: 'save', label: t('Save as preset…') },
  ]
  return (
    <>
      <div className="design-cp-row">
        <Dropdown checks={false}
          value={mode}
          options={[
            { value: 'fill', label: t('Fill') },
            { value: 'fit', label: t('Fit') },
            { value: 'crop', label: t('Crop') },
            { value: 'tile', label: t('Tile') },
          ]}
          width={100}
          onChange={(m) => setImage({ scale_mode: m })}
          testId={`${testId}-scale-mode`}
        />
        <IconButton icon="Rotate" label={t('Rotate 90°')} onClick={() => setImage({ rotation: ((Number(img.rotation ?? 0) + 1) % 4 + 4) % 4 })} testId={`${testId}-image-rotate`} />
      </div>
      {profile ? <div className="design-cp-row design-cp-profile">{t('Color profile: {name}', { name: profile })}</div> : null}
      <div className="design-cp-row" data-tip={t('Adjustment presets')}>
        <Dropdown checks={false}
          value={null}
          mixedLabel={current}
          options={presetOptions}
          width={152}
          onChange={(v) => {
            if (v === 'save') setNaming(`Preset ${presets.length + 1}`)
            else if (v.startsWith('built:')) apply(v.slice(6))
            else apply(v.slice(6))
          }}
          testId={`${testId}-presets`}
        />
        <Button disabled={identity} tip={t('Reset adjustments')} onClick={() => apply('reset')} testId={`${testId}-reset`}>
          {t('Reset')}
        </Button>
      </div>
      {naming !== null ? (
        <div className="design-cp-row">
          <TextField value={naming} width={136} autoFocus onCommit={setNaming} ariaLabel={t('Name')} />
          <Button
            variant="primary"
            disabled={!naming.trim()}
            onClick={() => {
              onPreset?.({ save: naming.trim(), filters })
              setNaming(null)
            }}
          >
            {t('Save')}
          </Button>
        </div>
      ) : null}
      {FILTERS.map((label) => {
        const k = filterKey(label)
        const value = filters[k] ?? 0
        return (
          <div key={label} className="design-cp-row design-cp-filter">
            <span className="design-cp-filter-label">{t(label)}</span>
            <div
              className="design-cp-range"
              data-testid={`${testId}-filter-${k}`}
              onPointerDown={(event) => {
                let v = value
                dragIn(
                  event,
                  (x) => {
                    v = Math.round((x * 2 - 1) * 1000) / 1000
                    setImage({ filters: { ...filters, [k]: v } }, 'live')
                  },
                  () => setImage({ filters: { ...filters, [k]: v } }, 'done'),
                )
              }}
            >
              <span className="design-cp-range-fill" style={{ width: `${((value + 1) / 2) * 100}%` }} />
              <span className="design-cp-range-knob" style={{ left: `${((value + 1) / 2) * 100}%` }} />
            </div>
          </div>
        )
      })}
    </>
  )
}

function PatternControls({ paint, onChange, sources, testId }: { paint: Paint; onChange: (p: Paint, phase: ChangePhase) => void; sources: PatternSource[]; testId: string }) {
  const { t } = useDesignKit()
  const pt = paint.pattern!
  const set = (next: Record<string, unknown>, phase: ChangePhase = 'done') => onChange({ ...paint, pattern: { ...pt, ...next } }, phase)
  const source = Number(pt.source)
  const name = sources.find((s) => s.id === source)?.name ?? t('Missing layer')
  const spacing = (Array.isArray(pt.spacing) ? pt.spacing : [0, 0]) as number[]
  const latest = useRef(paint)
  latest.current = paint
  const scrubFor = (apply: (v: number) => Record<string, unknown>): ScrubHandlers => {
    let last: Record<string, unknown> | null = null
    // The paint before the scrub, which Escape puts back.
    let start: Paint | null = null
    return {
      begin: () => {
        start = latest.current
      },
      move: (v) => {
        last = apply(v)
        set(last, 'live')
      },
      end: (cancelled) => {
        if (cancelled) {
          if (last && start) onChange(start, 'cancel')
        } else if (last) set(last, 'done')
        last = null
      },
    }
  }
  return (
    <>
      <div className="design-cp-row">
        <Dropdown checks={false} value={String(source)} mixedLabel={name} options={sources.map((s) => ({ value: String(s.id), label: s.name }))} width={208} onChange={(v) => set({ source: Number(v) })} ariaLabel={t('Pattern source')} testId={`${testId}-pattern-source`} />
      </div>
      <div className="design-cp-row">
        <Dropdown checks={false}
          value={String(pt.tile ?? 'grid')}
          icon="ViewGrid"
          options={[
            { value: 'grid', label: t('Grid') },
            { value: 'horizontalHex', label: t('Horizontal hex') },
            { value: 'verticalHex', label: t('Vertical hex') },
          ]}
          width={120}
          onChange={(v) => set({ tile: v })}
          ariaLabel={t('Tile type')}
        />
        <Dropdown checks={false}
          value={String(pt.align ?? 'start')}
          options={[
            { value: 'start', label: t('Start') },
            { value: 'center', label: t('Center') },
            { value: 'end', label: t('End') },
          ]}
          width={80}
          onChange={(v) => set({ align: v })}
          ariaLabel={t('Pattern alignment')}
        />
      </div>
      <div className="design-cp-row">
        <NumberField value={Math.round(Number(pt.scale ?? 1) * 100)} icon="Scale" tip={t('Pattern scale')} suffix="%" width={64} onCommit={(e) => e.kind === 'set' && set({ scale: Math.min(100, Math.max(0.01, e.value / 100)) })} scrub={scrubFor((v) => ({ scale: Math.min(100, Math.max(0.01, v / 100)) }))} />
        <NumberField value={spacing[0] ?? 0} icon="PaddingH" tip={t('Horizontal spacing')} width={64} onCommit={(e) => e.kind === 'set' && set({ spacing: [Math.max(0, e.value), spacing[1] ?? 0] })} scrub={scrubFor((v) => ({ spacing: [Math.max(0, v), spacing[1] ?? 0] }))} />
        <NumberField value={spacing[1] ?? 0} icon="PaddingV" tip={t('Vertical spacing')} width={64} onCommit={(e) => e.kind === 'set' && set({ spacing: [spacing[0] ?? 0, Math.max(0, e.value)] })} scrub={scrubFor((v) => ({ spacing: [spacing[0] ?? 0, Math.max(0, v)] }))} />
      </div>
    </>
  )
}
