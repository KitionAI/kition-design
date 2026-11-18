/**
 * Effects (2.4, upstream `panels/right/effects.rs`): the header (Styles, Add), the linked style's
 * chip, one row per effect (the top one first: settings toggle, type, visibility, remove; right-click
 * copies and pastes; rows drag to reorder) and, under an open row, its settings: shadows (offset,
 * blur, spread, colour, show behind), blurs (uniform or progressive, with canvas handles), noise,
 * texture and glass. Every edit sends the whole effect (`effect.set`); scrubs are one undo step.
 */
import { useRef } from 'react'
import { useDesignKit, useInspect, useInteraction, type Interaction } from '../kit/hooks'
import { Checkbox, Dropdown, IconButton, NumberField, Row, SectionHeader, Separator, TextField, type NumberEdit, type ScrubHandlers } from '../ui'
import { type Color, fromHexOrName, readColor, toHex, withAlpha } from './color'
import { type SelectionData, common, MIXED } from './data'
import { movedIndex, removedIndex, solid } from './paint'
import { BlurHandles } from './PaintHandles'
import { usePaintWriter } from './PaintPicker'
import { StyleChip, clipboardState, useContextMenu, useListDrag, openStyles } from './shared'
import { closePicker, openPicker, paintState, setEffectsOpen, toggleEffectOpen, updatePicker, usePaintState } from './store'
import { PaintSwatch } from './Swatch'

export type EffectJson = Record<string, unknown> & { type: string; visible?: boolean }

export const EFFECT_TYPES: Array<[string, string]> = [
  ['dropShadow', 'Drop shadow'],
  ['innerShadow', 'Inner shadow'],
  ['layerBlur', 'Layer blur'],
  ['backgroundBlur', 'Background blur'],
  ['noise', 'Noise'],
  ['texture', 'Texture'],
  ['glass', 'Glass'],
]

const MAX_BLUR = 10000

/** Writes whole effects: typed edits run, scrubs go through one interaction ("Effect"). */
function useEffectWriter(ids: number[], index: number) {
  const { run, t } = useDesignKit()
  const interaction = useInteraction()
  const active = useRef<Interaction | null>(null)
  const set = (effect: EffectJson) => void run('effect.set', { ids, index, effect })
  const scrub = (make: (v: number) => EffectJson): ScrubHandlers => ({
    begin: () => {
      active.current = interaction(t('Effect'))
    },
    move: (v) => active.current?.update('effect.set', { ids, index, effect: make(v) }),
    end: (cancelled) => {
      const i = active.current
      active.current = null
      void (cancelled ? i?.cancel() : i?.commit())
    },
  })
  return { set, scrub }
}

export function EffectsSection({ data }: { data: SelectionData }) {
  const { t, run } = useDesignKit()
  const state = usePaintState()
  const ids = data.ids
  const value = common<EffectJson[]>(data, 'effects')
  const list = value === MIXED || !Array.isArray(value) ? null : value
  const empty = !list || list.length === 0
  const header = useContextMenu(() => (clipboardState().effect ? [{ label: t('Paste effect'), onSelect: () => void run('effect.paste', { ids }) }] : []), 'design-effect-header-menu')
  const drag = useListDrag(list?.length ?? 0, (from, to) => {
    void run('effect.move', { ids, from, to })
    // Open settings and an open picker stay with their effects.
    const open = new Set<number>()
    for (const i of paintState().effectsOpen) open.add(movedIndex(i, from, to))
    setEffectsOpen(open)
    const p = paintState().picker
    if (p?.kind === 'effect') updatePicker({ ...p, index: movedIndex(p.index, from, to) })
  })
  const removed = (i: number) => {
    const open = new Set<number>()
    for (const k of paintState().effectsOpen) {
      const n = removedIndex(k, i)
      if (n !== null) open.add(n)
    }
    setEffectsOpen(open)
    const p = paintState().picker
    if (p?.kind === 'effect') {
      const k = removedIndex(p.index, i)
      if (k === null) closePicker()
      else updatePicker({ ...p, index: k })
    }
  }
  return (
    <div data-testid="design-effect-section">
      <SectionHeader
        title={t('Effects')}
        dim={empty}
        dimAction={1}
        onContextMenu={header.open}
        testId="design-effect-header"
        actions={[
          { icon: 'Styles', label: t('Styles'), onClick: () => openStyles(ids, 'effect', 'design-effect-styles'), testId: 'design-effect-styles' },
          { icon: 'Plus', label: t('Add effect'), onClick: () => void run('effect.add', { ids, type: 'dropShadow' }), testId: 'design-effect-add' },
        ]}
      />
      {header.menu}
      <StyleChip ids={ids} slot="effect" nodes={data.nodes} testId="design-effect-style" />
      {list
        ?.map((effect, index) => ({ effect, index }))
        .reverse()
        .map(({ effect, index }) => (
          <EffectRow key={index} data={data} index={index} effect={effect} open={state.effectsOpen.has(index)} drag={drag} onRemoved={removed} />
        ))}
      <Separator />
    </div>
  )
}

function EffectRow({ data, index, effect, open, drag, onRemoved }: { data: SelectionData; index: number; effect: EffectJson; open: boolean; drag: ReturnType<typeof useListDrag>; onRemoved: (i: number) => void }) {
  const { t, run } = useDesignKit()
  const ids = data.ids
  const testId = `design-effect-${index}`
  const label = EFFECT_TYPES.find(([id]) => id === effect.type)?.[1] ?? effect.type
  const menu = useContextMenu(() => [
    {
      label: t('Copy effect'),
      onSelect: () => void run('effect.copy', { ids, index }),
    },
    ...(clipboardState().effect ? [{ label: t('Paste effect'), onSelect: () => void run('effect.paste', { ids, index }) }] : []),
  ], `${testId}-menu`)
  const visible = effect.visible !== false
  return (
    <div className="design-paint-row" {...drag.row(index)} data-testid={testId}>
      <div onContextMenu={menu.open}>
        {drag.grip(index, t(label))}
        <Row>
          <IconButton iconSize={16} icon="Effects" label={t('Effect settings')} pressed={open} onClick={() => toggleEffectOpen(index)} testId={`${testId}-settings`} />
          <Dropdown checks={false}
            value={effect.type}
            options={EFFECT_TYPES.map(([id, l]) => ({ value: id, label: t(l) }))}
            width="calc(100% - 88px)"
            onChange={(type) => void run('effect.set', { ids, index, effect: { type } })}
            testId={`${testId}-type`}
          />
          <IconButton iconSize={16} icon={visible ? 'Eye' : 'EyeOff'} label={t('Toggle')} onClick={() => void run('effect.set', { ids, index, effect: { ...effect, visible: !visible } })} testId={`${testId}-visible`} />
          <IconButton
            icon="Minus"
            label={t('Remove')}
            testId={`${testId}-remove`}
            onClick={async () => {
              if ((await run('effect.remove', { ids, index })) !== null) onRemoved(index)
            }}
          />
        </Row>
      </div>
      {menu.menu}
      {open ? <EffectSettings data={data} index={index} effect={effect} /> : null}
    </div>
  )
}

const half = 'calc((100% - 8px) / 2)'

function EffectSettings({ data, index, effect }: { data: SelectionData; index: number; effect: EffectJson }) {
  const { t } = useDesignKit()
  const inspect = useInspect()
  const ids = data.ids
  const { set, scrub } = useEffectWriter(ids, index)
  const n = (key: string, fallback = 0) => {
    const x = effect[key]
    return typeof x === 'number' && Number.isFinite(x) ? x : fallback
  }
  /** A number field of the effect: typed edits and scrubs write `make(value)`. */
  const field = (key: string, label: string, value: number, make: (v: number) => EffectJson, opts: { suffix?: string; step?: number; width?: string } = {}) => (
    <NumberField
      key={key}
      value={value}
      label={label}
      suffix={opts.suffix}
      step={opts.step}
      width={opts.width ?? half}
      onCommit={(edit: NumberEdit) => set(make(edit.kind === 'set' ? edit.value : edit.apply(value)))}
      scrub={scrub(make)}
      testId={`design-effect-${index}-${key}`}
    />
  )
  const with_ = (patch: Record<string, unknown>): EffectJson => ({ ...effect, ...patch })
  const pct = (v: number) => Math.round(v * 100)
  const unit = (v: number) => Math.min(1, Math.max(0, v / 100))
  switch (effect.type) {
    case 'dropShadow':
    case 'innerShadow': {
      const offset = (Array.isArray(effect.offset) ? effect.offset : [0, 4]) as number[]
      return (
        <div className="design-effect-settings" data-testid={`design-effect-${index}-panel`}>
          <Row>
            {field('offsetX', 'X', offset[0] ?? 0, (v) => with_({ offset: [v, offset[1] ?? 0] }))}
            {field('offsetY', 'Y', offset[1] ?? 0, (v) => with_({ offset: [offset[0] ?? 0, v] }))}
          </Row>
          <Row>
            {field('radius', t('Blur'), n('radius', 4), (v) => with_({ radius: Math.max(0, v) }))}
            {field('spread', t('Spread'), n('spread'), (v) => with_({ spread: v }))}
          </Row>
          <EffectColor ids={ids} index={index} effect={effect} secondary={false} />
          {effect.type === 'dropShadow' ? (
            <Row>
              <Checkbox checked={effect.showBehind === true} label={t('Show behind transparent areas')} onChange={(on) => set(with_({ showBehind: on }))} testId={`design-effect-${index}-behind`} />
            </Row>
          ) : null}
        </div>
      )
    }
    case 'layerBlur':
    case 'backgroundBlur': {
      const progressive = effect.blurType === 'progressive'
      const single = ids.length === 1 && effect.visible !== false && progressive
      return (
        <div className="design-effect-settings" data-testid={`design-effect-${index}-panel`}>
          <Row>
            <Dropdown checks={false}
              value={progressive ? 'progressive' : 'uniform'}
              options={[
                { value: 'uniform', label: t('Uniform') },
                { value: 'progressive', label: t('Progressive') },
              ]}
              width="100%"
              onChange={(blurType) => set(with_({ blurType }))}
              testId={`design-effect-${index}-blur-type`}
            />
          </Row>
          <Row>
            {progressive ? (
              <>
                {field('startRadius', t('Start'), n('startRadius'), (v) => with_({ startRadius: Math.min(MAX_BLUR, Math.max(0, v)) }))}
                {field('radius', t('End'), n('radius', 4), (v) => with_({ radius: Math.min(MAX_BLUR, Math.max(0, v)) }))}
              </>
            ) : (
              field('radius', t('Blur'), n('radius', 4), (v) => with_({ radius: Math.min(MAX_BLUR, Math.max(0, v)) }), { width: '100%' })
            )}
          </Row>
          {single && inspect.selection.length === 1 ? <ProgressiveHandles id={ids[0]} index={index} effect={effect} /> : null}
        </div>
      )
    }
    case 'noise': {
      const mode = String(effect.mode ?? 'mono')
      return (
        <div className="design-effect-settings" data-testid={`design-effect-${index}-panel`}>
          <Row>
            <Dropdown checks={false}
              value={mode}
              options={[
                { value: 'mono', label: t('Mono') },
                { value: 'duo', label: t('Duo') },
                { value: 'multi', label: t('Multi') },
              ]}
              width="100%"
              onChange={(m) => set(with_({ mode: m }))}
              testId={`design-effect-${index}-noise-mode`}
            />
          </Row>
          <Row>
            {field('size', t('Size'), n('size', 0.5), (v) => with_({ size: Math.min(1000, Math.max(0.01, v)), sizeY: null }), { step: 0.1 })}
            {field('density', t('Density'), pct(n('density', 0.5)), (v) => with_({ density: unit(v) }), { suffix: '%' })}
          </Row>
          {mode === 'multi' ? (
            <Row>{field('opacity', t('Opacity'), pct(n('opacity', 0.25)), (v) => with_({ opacity: unit(v) }), { suffix: '%' })}</Row>
          ) : (
            <>
              <EffectColor ids={ids} index={index} effect={effect} secondary={false} />
              {mode === 'duo' ? <EffectColor ids={ids} index={index} effect={effect} secondary /> : null}
            </>
          )}
        </div>
      )
    }
    case 'texture':
      return (
        <div className="design-effect-settings" data-testid={`design-effect-${index}-panel`}>
          <Row>
            {field('size', t('Size'), n('size', 0.5), (v) => with_({ size: Math.min(1000, Math.max(0.01, v)), sizeY: null }), { step: 0.1 })}
            {field('radius', t('Radius'), n('radius', 4), (v) => with_({ radius: Math.max(0, v) }))}
          </Row>
          <Row>
            <Checkbox checked={effect.clipToShape === true} label={t('Clip to shape')} onChange={(on) => set(with_({ clipToShape: on }))} testId={`design-effect-${index}-clip`} />
          </Row>
        </div>
      )
    case 'glass':
      return (
        <div className="design-effect-settings" data-testid={`design-effect-${index}-panel`}>
          <Row>
            {field('lightAngle', t('Light|glass').split('|')[0], Math.round(n('lightAngle', -45)), (v) => with_({ lightAngle: Math.min(180, Math.max(-180, v)) }), { suffix: '°' })}
            {field('lightIntensity', t('Intensity'), pct(n('lightIntensity', 0.8)), (v) => with_({ lightIntensity: unit(v) }), { suffix: '%' })}
          </Row>
          <Row>
            {field('refraction', t('Refraction'), pct(n('refraction', 0.8)), (v) => with_({ refraction: unit(v) }), { suffix: '%' })}
            {field('depth', t('Depth'), n('depth', 20), (v) => with_({ depth: Math.min(1000, Math.max(1, v)) }))}
          </Row>
          <Row>
            {field('dispersion', t('Dispersion'), pct(n('dispersion', 0.5)), (v) => with_({ dispersion: unit(v) }), { suffix: '%' })}
            {field('frost', t('Frost'), n('frost', 4), (v) => with_({ frost: Math.min(1000, Math.max(0, v)) }))}
          </Row>
          <Row>{field('splay', t('Splay'), pct(n('splay')), (v) => with_({ splay: unit(v) }), { suffix: '%' })}</Row>
        </div>
      )
    default:
      return null
  }
}

/** The progressive blur's canvas handles (one layer, settings open, Design tab). */
function ProgressiveHandles({ id, index, effect }: { id: number; index: number; effect: EffectJson }) {
  const { ui } = useDesignKit()
  const write = usePaintWriter('Edit progressive blur')
  if (ui.rightTab !== 'design') return null
  return <BlurHandles id={id} index={index} effect={effect} onChange={(next, phase) => write('effect.set', { ids: [id], index, effect: next }, phase, null)} />
}

/** An effect's colour row (`fx_color`): swatch (opens the picker), hex, alpha. */
function EffectColor({ ids, index, effect, secondary }: { ids: number[]; index: number; effect: EffectJson; secondary: boolean }) {
  const { t } = useDesignKit()
  const { set, scrub } = useEffectWriter(ids, index)
  const state = usePaintState()
  const key = secondary ? 'secondary' : 'color'
  const c: Color | null = readColor(effect[key])
  if (!c) return null
  const write = (next: Color) => ({ ...effect, [key]: { r: next.r, g: next.g, b: next.b, a: next.a } })
  const open = state.picker?.kind === 'effect' && state.picker.index === index && state.picker.secondary === secondary
  const testId = `design-effect-${index}-${secondary ? 'secondary' : 'color'}`
  return (
    <Row testId={testId}>
      <button type="button" className="design-paint-swatch" aria-label={t('Color')} aria-expanded={open} data-testid={`${testId}-swatch`} onClick={() => (open ? closePicker() : openPicker({ kind: 'effect', ids, index, secondary }))}>
        <PaintSwatch paint={solid(withAlpha(c, 1), c.a)} size={16} />
      </button>
      <TextField
        value={toHex(c)}
        width={80}
        ariaLabel={t('Color')}
        testId={`${testId}-hex`}
        onCommit={(text) => {
          const next = fromHexOrName(text)
          if (next) set(write(withAlpha(next, c.a)))
        }}
      />
      <NumberField
        value={Math.round(c.a * 100)}
        suffix="%"
        width={56}
        onCommit={(edit) => set(write(withAlpha(c, (edit.kind === 'set' ? edit.value : edit.apply(c.a * 100)) / 100)))}
        scrub={scrub((v) => write(withAlpha(c, v / 100)))}
        ariaLabel={t('Opacity')}
        testId={`${testId}-alpha`}
      />
    </Row>
  )
}
