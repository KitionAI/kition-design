/**
 * Layout / Auto layout (2.3, upstream `panels/layout_panel.rs`): direction and wrap, W and H with
 * the proportion lock, sizing (Fixed / Hug / Fill) with min and max limits, the 3 × 3 alignment
 * grid with the gap (fixed or auto), text baselines, padding (two fields, or one per side), the
 * wrapped lines' gap, grid columns and rows, strokes in layout and canvas stacking, grid track
 * sizes (`layout.gridGeometry`), the selection's Spacing (`arrange.spacingInfo`), a grid child's
 * cell, and Clip content.
 */
import { useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon, type IconName } from '../icons/Icon'
import { Checkbox, Dropdown, IconButton, NumberField, SectionHeader, Segmented, type MenuEntry } from '../ui'
import { useCommandField, useDesign, usePropField, useShortcutTip } from './hooks'
import { AddSlot, BindableField, COL, Line, MenuField, Sub, useMenu } from './fields'
import { common, inInstance, isFrameLike, round2, type AutoLayout, type NodeJson, type Sizing } from './model'

/** The "In an instance" note, shared by everything an instance takes from its main. */
const FROM_MAIN = 'In an instance, this comes from the main component'

/** Individual padding is one toggle for the whole panel (egui temp data upstream). */
let paddingOpen = false

export function LayoutSection() {
  const { t, run } = useDesignKit()
  const { answer, nodes, ids, setProps } = useDesign()
  const tipFor = useShortcutTip()
  const frames = nodes.every(isFrameLike)
  const al = common(nodes, (n) => n.layout ?? null)
  const hasAl = al !== null
  const instance = nodes.some(inInstance)
  const nested = answer.insideInstance === true
  const action = instance ? [] : frames && !hasAl ? [{ icon: 'Plus' as const, label: tipFor(t('Add auto layout'), 'layout.add'), onClick: () => void run('layout.add', { ids }), testId: 'design-design-layout-add' }] : hasAl ? [{ icon: 'Minus' as const, label: tipFor(t('Remove auto layout'), 'layout.remove'), onClick: () => void run('layout.remove', { ids }), testId: 'design-design-layout-remove' }] : []
  const gridParent = nodes.every((n) => n.parentGrid === true)
  const clip = common(nodes, (n) => n.clip === true)
  return (
    <div data-testid="design-design-layout">
      <SectionHeader title={hasAl ? t('Auto layout') : t('Layout')} actions={action} />
      {al ? <Direction layout={al} disabled={instance} /> : null}
      <Dimensions hasAl={hasAl} nested={nested} />
      {al ? (
        <>
          <Alignment layout={al} />
          <Padding layout={al} />
          <div className="design-d-block" data-disabled={instance || undefined} data-tip={instance ? t(FROM_MAIN) : undefined}>
            <Advanced layout={al} disabled={instance} />
            {al.mode === 'grid' && ids.length === 1 ? <Tracks layout={al} id={ids[0]} disabled={instance} /> : null}
          </div>
        </>
      ) : null}
      <Spacing />
      {gridParent && common(nodes, (n) => n.absolute === true) === false ? <GridCell disabled={nested} /> : null}
      {frames ? (
        <Line>
          <Checkbox checked={clip ?? false} label={t('Clip content')} onChange={() => setProps({ clip: !(clip ?? false) })} testId="design-design-clip" />
        </Line>
      ) : null}
    </div>
  )
}

function Direction({ layout, disabled }: { layout: AutoLayout; disabled: boolean }) {
  const { t } = useDesignKit()
  const { setProps } = useDesign()
  return (
    <Line>
      <span data-tip={disabled ? t(FROM_MAIN) : undefined}>
        <Segmented
          value={layout.mode}
          cell={32}
          disabled={disabled}
          segments={[
            { value: 'vertical', label: t('Vertical'), icon: 'LayoutV' },
            { value: 'horizontal', label: t('Horizontal'), icon: 'LayoutH' },
            { value: 'grid', label: t('Grid'), icon: 'LayoutGrid' },
          ]}
          onChange={(mode) => setProps({ layout: mode })}
          testId="design-design-direction"
        />
      </span>
      {layout.mode !== 'grid' ? <IconButton icon="LayoutWrap" label={t('Wrap')} pressed={layout.wrap} disabled={disabled} onClick={() => setProps({ wrap: !layout.wrap })} testId="design-design-wrap" /> : null}
    </Line>
  )
}

type Axis = { sizing: 'sizingH' | 'sizingV'; min: 'minWidth' | 'minHeight'; max: 'maxWidth' | 'maxHeight'; addMin: string; addMax: string; remove: string; minLead: string; maxLead: string; horizontal: boolean }
const AXES: Axis[] = [
  { sizing: 'sizingH', min: 'minWidth', max: 'maxWidth', addMin: 'Add min width', addMax: 'Add max width', remove: 'Remove min and max width', minLead: 'Min W', maxLead: 'Max W', horizontal: true },
  { sizing: 'sizingV', min: 'minHeight', max: 'maxHeight', addMin: 'Add min height', addMax: 'Add max height', remove: 'Remove min and max height', minLead: 'Min H', maxLead: 'Max H', horizontal: false },
]

const SIZING_ICON: Record<Sizing, IconName> = { fixed: 'Fixed', hug: 'Hug', fill: 'FillContainer' }
const sizingLabel = (s: Sizing) => (s === 'fill' ? 'Fill|sizing' : s === 'hug' ? 'Hug' : 'Fixed')

function Dimensions({ hasAl, nested }: { hasAl: boolean; nested: boolean }) {
  const { t } = useDesignKit()
  const { answer, nodes, ids, setProps } = useDesign()
  const wOf = (n: NodeJson) => round2(n.width ?? 0)
  const hOf = (n: NodeJson) => round2(n.height ?? 0)
  const w = usePropField(ids, 'width', { of: wOf, nodes, map: (v) => Math.max(0, v) })
  const h = usePropField(ids, 'height', { of: hOf, nodes, map: (v) => Math.max(0, v) })
  const ws = common(nodes, wOf)
  const hs = common(nodes, hOf)
  const locked = common(nodes, (n) => n.constrainProportions === true) ?? false
  const parentAl = answer.allParentsAutoLayout === true
  const isText = nodes.every((n) => n.type === 'TEXT')
  const canFill = parentAl && nodes.every((n) => n.absolute !== true)
  const canLimit = hasAl || canFill
  return (
    <>
      <Sub a={t('Dimensions')} />
      <Line>
        <BindableField ids={ids} nodes={nodes} field="width" label="W" tip={t('Width')} value={ws} mixed={ws === null} width={COL} min={0} onCommit={w.onCommit} scrub={w.scrub} testId="design-design-width" />
        <BindableField ids={ids} nodes={nodes} field="height" label="H" tip={t('Height')} value={hs} mixed={hs === null} width={COL} min={0} onCommit={h.onCommit} scrub={h.scrub} testId="design-design-height" />
        <span data-tip={nested ? t(FROM_MAIN) : undefined}>
          <IconButton icon={locked ? 'LinkOn' : 'LinkOff'} label={t('Constrain proportions')} pressed={locked} disabled={nested} onClick={() => setProps({ constrainProportions: !locked })} testId="design-design-constrain" />
        </span>
      </Line>
      {hasAl || parentAl || isText ? (
        <Line>
          {AXES.map((axis) => (
            <SizingDropdown key={axis.sizing} axis={axis} hasAl={hasAl} isText={isText} canFill={canFill} canLimit={canLimit} />
          ))}
        </Line>
      ) : null}
      {canLimit ? AXES.map((axis) => <LimitRow key={axis.min} axis={axis} />) : null}
    </>
  )
}

const limitOf = (n: NodeJson, key: string) => (n as Record<string, unknown>)[key] as number | undefined

function SizingDropdown({ axis, hasAl, isText, canFill, canLimit }: { axis: Axis; hasAl: boolean; isText: boolean; canFill: boolean; canLimit: boolean }) {
  const { t } = useDesignKit()
  const { nodes, setProps } = useDesign()
  const menu = useMenu()
  const current = common(nodes, (n) => (axis.horizontal ? n.sizing?.horizontal : n.sizing?.vertical) ?? 'fixed')
  const anyLimit = nodes.some((n) => limitOf(n, axis.min) !== undefined || limitOf(n, axis.max) !== undefined)
  const first = nodes[0]
  const size = first ? (axis.horizontal ? first.width : first.height) ?? 0 : 0
  const items: MenuEntry[] = [
    { label: t('Fixed'), onSelect: () => setProps({ [axis.sizing]: 'fixed' }), testId: `design-design-${axis.sizing}-fixed` },
    ...(hasAl || isText ? [{ label: t('Hug contents'), onSelect: () => setProps({ [axis.sizing]: 'hug' }), testId: `design-design-${axis.sizing}-hug` }] : []),
    ...(canFill ? [{ label: t('Fill container'), onSelect: () => setProps({ [axis.sizing]: 'fill' }), testId: `design-design-${axis.sizing}-fill` }] : []),
  ]
  if (canLimit) {
    items.push({ type: 'separator' })
    if (first && limitOf(first, axis.min) === undefined) items.push({ label: t(axis.addMin), onSelect: () => setProps({ [axis.min]: size }), testId: `design-design-${axis.min}-add` })
    if (first && limitOf(first, axis.max) === undefined) items.push({ label: t(axis.addMax), onSelect: () => setProps({ [axis.max]: size }), testId: `design-design-${axis.max}-add` })
    if (anyLimit) items.push({ label: t('Remove min and max'), onSelect: () => setProps({ [axis.min]: null, [axis.max]: null }) })
  }
  return (
    <>
      <button ref={menu.ref} type="button" className="design-dropdown" style={{ width: COL }} aria-haspopup="menu" aria-expanded={menu.open} onClick={menu.toggle} data-testid={`design-design-${axis.sizing}`} aria-label={axis.horizontal ? t('Horizontal resizing') : t('Vertical resizing')}>
        <span className="design-dropdown-lead design-d-sizing-lead" data-limits={anyLimit || undefined}>
          <Icon name={SIZING_ICON[current ?? 'fixed']} size={14} />
        </span>
        <span className="design-dropdown-label" data-mixed={current === null || undefined}>
          {current ? t(sizingLabel(current)) : t('Mixed')}
        </span>
        <Icon name="Chevron" size={12} className="design-chevron" />
      </button>
      {menu.show(items, 200, `design-design-${axis.sizing}-menu`)}
    </>
  )
}

function LimitRow({ axis }: { axis: Axis }) {
  const { t } = useDesignKit()
  const { nodes, ids, setProps } = useDesign()
  const has = (key: string) => nodes.some((n) => limitOf(n, key) !== undefined)
  const min = usePropField(ids, axis.min, { map: (v) => Math.max(0, v) })
  const max = usePropField(ids, axis.max, { map: (v) => Math.max(0, v) })
  if (!has(axis.min) && !has(axis.max)) return null
  const first = nodes[0]
  const size = first ? (axis.horizontal ? first.width : first.height) ?? 0 : 0
  const side = (key: string, lead: string, add: string, field: typeof min) => {
    if (!has(key)) return <AddSlot label={t(lead)} tip={t(add)} onClick={() => setProps({ [key]: size })} testId={`design-design-${key}-slot`} />
    const v = common(nodes, (n) => (limitOf(n, key) === undefined ? null : round2(limitOf(n, key) as number)))
    return <BindableField ids={ids} nodes={nodes} field={key} label={t(lead)} value={v} mixed={false} width={COL} min={0} onCommit={field.onCommit} scrub={field.scrub} testId={`design-design-${key}`} />
  }
  return (
    <Line>
      {side(axis.min, axis.minLead, axis.addMin, min)}
      {side(axis.max, axis.maxLead, axis.addMax, max)}
      <IconButton icon="Minus" label={t(axis.remove)} onClick={() => setProps({ [axis.min]: null, [axis.max]: null })} testId={`design-design-${axis.min}-remove`} />
    </Line>
  )
}

const AUTO_GAPS = ['spaceBetween', 'spaceAround', 'spaceEvenly']

function Alignment({ layout }: { layout: AutoLayout }) {
  const { t } = useDesignKit()
  const { nodes, ids, setProps } = useDesign()
  const grid = layout.mode === 'grid'
  const horizontal = layout.mode !== 'vertical'
  const auto = AUTO_GAPS.includes(layout.mainAlign)
  const curMain = auto ? 9 : ['min', 'center', 'max'].indexOf(layout.mainAlign)
  const spread = !grid && layout.wrap && layout.contentAlign === 'spaceBetween'
  const curCross = spread ? 9 : layout.crossAlign === 'baseline' ? 0 : ['min', 'center', 'max'].indexOf(layout.crossAlign)
  const gap = usePropField(ids, 'gap')
  const columnGap = usePropField(ids, 'columnGap', { map: (v) => Math.max(0, v) })
  const rowGap = usePropField(ids, 'rowGap', { map: (v) => Math.max(0, v) })
  const cells = []
  for (let yi = 0; yi < 3; yi += 1) {
    for (let xi = 0; xi < 3; xi += 1) {
      const [mainI, crossI] = horizontal ? [xi, yi] : [yi, xi]
      const on = (curMain === mainI || curMain === 9) && (curCross === crossI || curCross === 9)
      const pick = () => setProps({ mainAlign: auto && !grid ? layout.mainAlign : ['min', 'center', 'max'][mainI], crossAlign: ['min', 'center', 'max'][crossI] })
      cells.push(
        <button key={`${xi}-${yi}`} type="button" className="design-d-align-cell" style={{ left: 12 + xi * 24 - 11, top: 10 + yi * 22 - 10 }} data-on={on || undefined} data-dir={horizontal ? 'h' : 'v'} aria-pressed={on} aria-label={`${t('Alignment')} ${xi + 1}, ${yi + 1}`} onClick={pick} data-testid={`design-design-align-cell-${xi}-${yi}`}>
          <span />
        </button>,
      )
    }
  }
  const gapLabel = layout.mainAlign === 'spaceBetween' ? 'Space between' : layout.mainAlign === 'spaceAround' ? 'Space around' : layout.mainAlign === 'spaceEvenly' ? 'Space evenly' : 'Fixed gap'
  return (
    <>
      <div className="design-row design-d-row" style={{ height: 76, alignItems: 'flex-start', paddingTop: 6 }}>
        <div className="design-d-align-grid" data-testid="design-design-align-grid">
          {cells}
        </div>
        <div className="design-d-stack">
          {grid ? (
            <>
              <BindableField ids={ids} nodes={nodes} field="columnGap" icon="Gap" tip={t('Column gap')} value={layout.gap} width={96} min={0} onCommit={columnGap.onCommit} scrub={columnGap.scrub} testId="design-design-column-gap" />
              <BindableField ids={ids} nodes={nodes} field="rowGap" icon="DistributeV" tip={t('Row gap')} value={layout.crossGap} width={96} min={0} onCommit={rowGap.onCommit} scrub={rowGap.scrub} testId="design-design-row-gap" />
            </>
          ) : (
            <>
              <BindableField ids={ids} nodes={nodes} field="gap" icon={layout.mode === 'vertical' ? 'DistributeV' : 'Gap'} tip={t('Gap between items')} value={auto ? null : layout.gap} placeholder={t('Auto')} width={96} onCommit={gap.onCommit} scrub={gap.scrub} testId="design-design-gap" />
              <MenuField
                label={t(gapLabel)}
                items={[
                  ['min', 'Fixed'],
                  ['spaceBetween', 'Auto (space between)'],
                  ['spaceEvenly', 'Auto (space evenly)'],
                  ['spaceAround', 'Auto (space around)'],
                ].map(([value, label]) => ({ label: t(label), onSelect: () => setProps({ mainAlign: value }), testId: `design-design-gap-mode-${value}` }))}
                width={96}
                ariaLabel={t('Auto gap')}
                testId="design-design-gap-mode"
              />
            </>
          )}
        </div>
      </div>
      {layout.mode === 'horizontal' ? (
        <Line h={28}>
          <Checkbox checked={layout.crossAlign === 'baseline'} label={t('Align text baselines')} onChange={() => setProps({ crossAlign: layout.crossAlign === 'baseline' ? 'min' : 'baseline' })} testId="design-design-baseline" />
        </Line>
      ) : null}
    </>
  )
}

function Padding({ layout }: { layout: AutoLayout }) {
  const { t } = useDesignKit()
  const { nodes, ids, setProps } = useDesign()
  const [open, setOpen] = useState(paddingOpen)
  const [pt, pr, pb, pl] = layout.padding
  const clamp = { map: (v: number) => Math.max(0, v) }
  const fields = {
    paddingHorizontal: usePropField(ids, 'paddingHorizontal', clamp),
    paddingVertical: usePropField(ids, 'paddingVertical', clamp),
    paddingLeft: usePropField(ids, 'paddingLeft', clamp),
    paddingTop: usePropField(ids, 'paddingTop', clamp),
    paddingRight: usePropField(ids, 'paddingRight', clamp),
    paddingBottom: usePropField(ids, 'paddingBottom', clamp),
  }
  const crossGap = useCommandField((v) => ({ command: 'node.set', params: { ids, props: { crossGap: Math.max(0, v), contentAlign: 'auto' } } }))
  const toggle = (
    <IconButton
      icon="IndependentPadding"
      label={t('Individual padding')}
      pressed={open}
      onClick={() => {
        paddingOpen = !open
        setOpen(!open)
      }}
      testId="design-design-padding-individual"
    />
  )
  const field = (key: keyof typeof fields, icon: IconName, name: string, value: number | null) => (
    <BindableField ids={ids} nodes={nodes} field={key} icon={icon} tip={t(name)} value={value} mixed={value === null} width={COL} min={0} onCommit={fields[key].onCommit} scrub={fields[key].scrub} testId={`design-design-${key}`} />
  )
  const vertical = layout.mode === 'vertical'
  const spread = layout.contentAlign === 'spaceBetween'
  return (
    <>
      {open ? (
        <>
          <Line>
            {field('paddingLeft', 'PaddingLeft', 'Left padding', pl)}
            {field('paddingTop', 'PaddingTop', 'Top padding', pt)}
            {toggle}
          </Line>
          <Line>
            {field('paddingRight', 'PaddingRight', 'Right padding', pr)}
            {field('paddingBottom', 'PaddingBottom', 'Bottom padding', pb)}
          </Line>
        </>
      ) : (
        <Line>
          {field('paddingHorizontal', 'PaddingH', 'Horizontal padding', pl === pr ? pl : null)}
          {field('paddingVertical', 'PaddingV', 'Vertical padding', pt === pb ? pt : null)}
          {toggle}
        </Line>
      )}
      {layout.mode !== 'grid' && layout.wrap ? (
        <>
          <Sub a={vertical ? t('Column gap') : t('Row gap')} />
          <Line>
            <BindableField ids={ids} nodes={nodes} field="crossGap" icon={vertical ? 'Gap' : 'DistributeV'} tip={vertical ? t('Gap between columns') : t('Gap between rows')} value={spread ? null : layout.crossGap} placeholder={t('Auto')} width={COL} min={0} onCommit={crossGap.onCommit} scrub={crossGap.scrub} testId="design-design-cross-gap" />
            <MenuField
              label={spread ? t('Space between') : t('Fixed gap')}
              items={[
                ['auto', 'Fixed'],
                ['spaceBetween', 'Auto (space between)'],
              ].map(([value, label]) => ({ label: t(label), onSelect: () => setProps({ contentAlign: value }) }))}
              width={COL}
              ariaLabel={vertical ? t('Column gap') : t('Row gap')}
              testId="design-design-cross-gap-mode"
            />
          </Line>
        </>
      ) : null}
      {layout.mode === 'grid' ? <GridCount layout={layout} /> : null}
    </>
  )
}

function GridCount({ layout }: { layout: AutoLayout }) {
  const { t } = useDesignKit()
  const { ids } = useDesign()
  const cols = usePropField(ids, 'gridColumns', { map: (v) => Math.max(1, Math.round(v)) })
  const rows = usePropField(ids, 'gridRows', { map: (v) => Math.max(1, Math.round(v)) })
  return (
    <>
      <Sub a={t('Columns')} b={t('Rows')} />
      <Line>
        <NumberField value={layout.gridColumns} min={1} decimals={0} width={COL} ariaLabel={t('Columns')} onCommit={cols.onCommit} scrub={cols.scrub} testId="design-design-grid-columns" />
        <NumberField value={layout.gridRows} min={1} decimals={0} width={COL} ariaLabel={t('Rows')} onCommit={rows.onCommit} scrub={rows.scrub} testId="design-design-grid-rows" />
      </Line>
    </>
  )
}

function Advanced({ layout, disabled }: { layout: AutoLayout; disabled: boolean }) {
  const { t } = useDesignKit()
  const { setProps } = useDesign()
  return (
    <>
      <Sub a={t('Strokes')} b={t('Canvas stacking')} />
      <Line>
        <MenuField
          label={layout.strokesIncluded ? t('Included') : t('Excluded')}
          items={[
            { label: t('Included in layout'), onSelect: () => setProps({ strokesIncluded: true }) },
            { label: t('Excluded from layout'), onSelect: () => setProps({ strokesIncluded: false }), testId: 'design-design-strokes-excluded' },
          ]}
          width={COL}
          disabled={disabled}
          ariaLabel={t('Strokes')}
          testId="design-design-strokes"
        />
        <MenuField
          label={layout.reverseZ ? t('First on top') : t('Last on top')}
          items={[
            { label: t('Last on top'), onSelect: () => setProps({ reverseZ: false }) },
            { label: t('First on top'), onSelect: () => setProps({ reverseZ: true }), testId: 'design-design-stacking-first' },
          ]}
          width={COL}
          disabled={disabled}
          ariaLabel={t('Canvas stacking')}
          testId="design-design-stacking"
        />
      </Line>
    </>
  )
}

type Track = { start: number; end: number; size: number }
/** `query('layout.gridGeometry')`: the grid's resolved tracks (frame space), or null for other layers. */
type GridGeometry = { id: number; columns: Track[]; rows: Track[] } | null

/** Track sizes of one grid frame, as the engine resolves them from the children's sizes. */
function Tracks({ layout, id, disabled }: { layout: AutoLayout; id: number; disabled: boolean }) {
  const { t } = useDesignKit()
  const geometry = useQuery<GridGeometry>('layout.gridGeometry', { id })
  if (!geometry) return null
  return (
    <>
      {(['columns', 'rows'] as const).map((axis) => (
        <div key={axis}>
          <Sub a={axis === 'columns' ? t('Column sizes') : t('Row sizes')} />
          {geometry[axis].slice(0, 24).map((track, i) => (
            <TrackRow key={i} id={id} axis={axis} index={i} track={(axis === 'columns' ? layout.columnTracks : layout.rowTracks)[i] ?? { sizing: 'fill', value: 1 }} size={round2(track.size)} disabled={disabled} />
          ))}
        </div>
      ))}
    </>
  )
}

type SpacingAxis = { value: number | null; tracks: Array<{ ids: number[] }>; gaps: number[] } | null
/** `query('arrange.spacingInfo')`: the selection's rows or columns outside auto layout flow. */
type SpacingInfo = { horizontal: SpacingAxis; vertical: SpacingAxis }

/** The Spacing row (upstream `selection_spacing::panel`): the gap between the selection's columns and rows, Mixed when they differ. */
function Spacing() {
  const { t } = useDesignKit()
  const { ids } = useDesign()
  const info = useQuery<SpacingInfo>('arrange.spacingInfo', { ids }, { skip: ids.length < 2 })
  const horizontal = useCommandField((v) => ({ command: 'arrange.spacing', params: { ids, horizontal: Math.min(Math.max(v, -1e6), 1e6), fromGesture: true } }))
  const vertical = useCommandField((v) => ({ command: 'arrange.spacing', params: { ids, vertical: Math.min(Math.max(v, -1e6), 1e6), fromGesture: true } }))
  if (ids.length < 2 || !info || (!info.horizontal && !info.vertical)) return null
  return (
    <>
      <Sub a={t('Spacing')} />
      <Line>
        {info.horizontal ? <NumberField icon="DistributeH" tip={t('Horizontal spacing')} value={info.horizontal.value} mixed={info.horizontal.value === null} width={COL} onCommit={horizontal.onCommit} scrub={horizontal.scrub} testId="design-design-spacing-h" /> : null}
        {info.vertical ? <NumberField icon="DistributeV" tip={t('Vertical spacing')} value={info.vertical.value} mixed={info.vertical.value === null} width={COL} onCommit={vertical.onCommit} scrub={vertical.scrub} testId="design-design-spacing-v" /> : null}
      </Line>
    </>
  )
}

function TrackRow({ id, axis, index, track, size, disabled }: { id: number; axis: 'columns' | 'rows'; index: number; track: { sizing: Sizing; value: number }; size: number; disabled: boolean }) {
  const { t, run } = useDesignKit()
  const fill = track.sizing === 'fill'
  const base = { ids: [id], axis, index }
  const field = useCommandField((v) => ({ command: 'layout.setTrack', params: { ...base, sizing: fill ? 'fill' : 'fixed', value: Math.max(0, v) } }))
  return (
    <Line>
      <NumberField label={String(index + 1)} value={fill ? track.value : size} suffix={fill ? 'fr' : ''} step={fill ? 0.1 : 1} width={COL} disabled={disabled} onCommit={field.onCommit} scrub={field.scrub} testId={`design-design-track-${axis}-${index}`} />
      {/* Upstream's button says Fixed / Hug / Fill; its menu (no check marks) says Hug contents and Fill (fr). */}
      <MenuField
        label={t(track.sizing === 'fill' ? 'Fill' : track.sizing === 'hug' ? 'Hug' : 'Fixed')}
        icon={SIZING_ICON[track.sizing]}
        items={(
          [
            ['fixed', 'Fixed'],
            ['hug', 'Hug contents'],
            ['fill', 'Fill (fr)'],
          ] as const
        ).map(([sizing, label]) => ({ label: t(label), onSelect: () => void run('layout.setTrack', { ...base, sizing }), testId: `design-design-track-${axis}-${index}-sizing-${sizing}` }))}
        width={COL}
        disabled={disabled}
        testId={`design-design-track-${axis}-${index}-sizing`}
      />
    </Line>
  )
}

function GridCell({ disabled }: { disabled: boolean }) {
  const { t } = useDesignKit()
  const { nodes, ids, setProps } = useDesign()
  const one = nodes.length === 1 ? nodes[0].gridCell : undefined
  const c0 = (one?.column ?? 0) + 1
  const r0 = (one?.row ?? 0) + 1
  const at = (c: number, r: number) => ({ command: 'node.set', params: { ids, props: { gridColumn: Math.max(0, c - 1), gridRow: Math.max(0, r - 1) } } })
  const column = useCommandField((v) => at(v, r0))
  const row = useCommandField((v) => at(c0, v))
  const colSpan = usePropField(ids, 'gridColumnSpan', { map: (v) => Math.max(1, Math.round(v)) })
  const rowSpan = usePropField(ids, 'gridRowSpan', { map: (v) => Math.max(1, Math.round(v)) })
  const cs = common(nodes, (n) => n.gridCell?.columnSpan ?? 1)
  const rs = common(nodes, (n) => n.gridCell?.rowSpan ?? 1)
  const alignH = common(nodes, (n) => n.gridCell?.alignH ?? 'auto')
  const alignV = common(nodes, (n) => n.gridCell?.alignV ?? 'auto')
  return (
    <div className="design-d-block" data-disabled={disabled || undefined} data-tip={disabled ? t(FROM_MAIN) : undefined}>
      {one ? (
        <>
          <Sub a={t('Column')} b={t('Row')} />
          <Line>
            <NumberField value={c0} min={1} decimals={0} width={COL} disabled={disabled} ariaLabel={t('Column')} onCommit={column.onCommit} scrub={column.scrub} testId="design-design-cell-column" />
            <NumberField value={r0} min={1} decimals={0} width={COL} disabled={disabled} ariaLabel={t('Row')} onCommit={row.onCommit} scrub={row.scrub} testId="design-design-cell-row" />
            {one.anchored ? <IconButton icon="Reset" label={t('Place automatically')} disabled={disabled} onClick={() => setProps({ gridColumn: null, gridRow: null })} testId="design-design-cell-reset" /> : null}
          </Line>
        </>
      ) : null}
      <Sub a={t('Column span')} b={t('Row span')} />
      <Line>
        <NumberField value={cs} mixed={cs === null} min={1} decimals={0} width={COL} disabled={disabled} ariaLabel={t('Column span')} onCommit={colSpan.onCommit} scrub={colSpan.scrub} testId="design-design-cell-column-span" />
        <NumberField value={rs} mixed={rs === null} min={1} decimals={0} width={COL} disabled={disabled} ariaLabel={t('Row span')} onCommit={rowSpan.onCommit} scrub={rowSpan.scrub} testId="design-design-cell-row-span" />
      </Line>
      <Sub a={t('Horizontal align')} b={t('Vertical align')} />
      <Line>
        <Dropdown checks={false}
          value={alignH}
          options={[
            { value: 'auto', label: t('Auto') },
            { value: 'min', label: t('Left') },
            { value: 'center', label: t('Center') },
            { value: 'max', label: t('Right') },
          ]}
          mixedLabel={t('Mixed')}
          onChange={(v) => setProps({ gridAlignH: v === 'auto' ? null : v })}
          width={COL}
          disabled={disabled}
          ariaLabel={t('Horizontal align')}
          testId="design-design-cell-align-h"
        />
        <Dropdown checks={false}
          value={alignV}
          options={[
            { value: 'auto', label: t('Auto') },
            { value: 'min', label: t('Top') },
            { value: 'center', label: t('Center') },
            { value: 'max', label: t('Bottom') },
          ]}
          mixedLabel={t('Mixed')}
          onChange={(v) => setProps({ gridAlignV: v === 'auto' ? null : v })}
          width={COL}
          disabled={disabled}
          ariaLabel={t('Vertical align')}
          testId="design-design-cell-align-v"
        />
      </Line>
    </div>
  )
}
