/**
 * Layout grid (2.3, upstream `panels/right/effects.rs` `grids`): shown when every layer is
 * frame-like. A row per grid, the last first (settings toggle, kind, visibility, remove), and the
 * open grid's settings inline: size, or count, type, width, margin or offset, gutter; and colour.
 */
import { useState } from 'react'
import { dispatchUiRequests } from '../kit/requests'
import { useDesignKit } from '../kit/hooks'
import { IconButton, NumberField, SectionHeader, TextField } from '../ui'
import { useCommandField, useDesign } from './hooks'
import { BindableField, COL, Line, MenuField, Sub } from './fields'
import { common, hexOf, isFrameLike, parseHex, type LayoutGrid } from './model'

/** Which grids' settings are open (by index; egui temp data upstream, one set for the panel). */
const openGrids = new Set<number>()

const DEFAULTS: Record<LayoutGrid['kind'], LayoutGrid> = {
  grid: { kind: 'grid', size: 10, count: 5, gutter: 20, offset: 0, align: 'stretch', color: { r: 1, g: 0, b: 0, a: 0.1 }, visible: true },
  columns: { kind: 'columns', size: 0, count: 5, gutter: 20, offset: 0, align: 'stretch', color: { r: 1, g: 0, b: 0, a: 0.1 }, visible: true },
  rows: { kind: 'rows', size: 0, count: 5, gutter: 20, offset: 0, align: 'stretch', color: { r: 1, g: 0, b: 0, a: 0.1 }, visible: true },
}

const autoCount = (g: LayoutGrid) => g.count === 0 && g.align !== 'stretch'

export function LayoutGridSection() {
  const { t, run } = useDesignKit()
  const { nodes, ids } = useDesign()
  const [, redraw] = useState(0)
  if (!nodes.length || !nodes.every(isFrameLike)) return null
  const list = common(nodes, (n) => n.grids ?? [])
  const empty = !list || list.length === 0
  const label = (g: LayoutGrid) =>
    g.kind === 'grid' ? t('Grid ({size})', { size: g.size }) : autoCount(g) ? t(g.kind === 'columns' ? 'Auto columns' : 'Auto rows') : t(g.kind === 'columns' ? '{n} columns' : '{n} rows', { n: Math.max(1, g.count) })
  const write = (next: LayoutGrid[]) => void run('node.set', { ids, props: { grids: next } })
  const change = (i: number, f: (g: LayoutGrid) => LayoutGrid) => (list ? list.map((g, k) => (k === i ? f(g) : g)) : [])
  return (
    <div data-testid="design-design-grids">
      <SectionHeader
        title={t('Layout grid')}
        dim={empty}
        dimAction={1}
        actions={[
          { icon: 'Styles', label: t('Styles'), onClick: () => dispatchUiRequests([{ kind: 'stylePicker.open', data: { ids, slot: 'grid' } }]), testId: 'design-design-grid-styles' },
          { icon: 'Plus', label: t('Add layout grid'), onClick: () => void run('grid.add', { ids }), testId: 'design-design-grid-add' },
        ]}
      />
      {(list ?? [])
        .map((g, i) => ({ g, i }))
        .reverse()
        .map(({ g, i }) => (
          <div key={i}>
            {/* Upstream's dropdown is the room after the button less 56, so the row runs 8 px into the right padding. */}
            <Line style={{ paddingRight: 8 }}>
              <IconButton
                iconSize={16}
                icon="Grid"
                label={t('Layout grid settings')}
                pressed={openGrids.has(i)}
                onClick={() => {
                  if (openGrids.has(i)) openGrids.delete(i)
                  else openGrids.add(i)
                  redraw((v) => v + 1)
                }}
                testId={`design-design-grid-${i}-settings`}
              />
              <span className="design-d-grow">
                <MenuField
                  label={label(g)}
                  width="100%"
                  menuWidth={220}
                  items={(['grid', 'columns', 'rows'] as const).map((kind) => ({ label: t(kind === 'grid' ? 'Grid' : kind === 'columns' ? 'Columns' : 'Rows'), onSelect: () => write(change(i, () => DEFAULTS[kind])), testId: `design-design-grid-${i}-kind-${kind}` }))}
                  testId={`design-design-grid-${i}-kind`}
                />
              </span>
              <IconButton iconSize={16} icon={g.visible ? 'Eye' : 'EyeOff'} label={t('Toggle')} onClick={() => write(change(i, (x) => ({ ...x, visible: !x.visible })))} testId={`design-design-grid-${i}-visible`} />
              <IconButton
                iconSize={16}
                icon="Minus"
                label={t('Remove')}
                onClick={() => {
                  void run('grid.remove', { ids, index: i })
                  openGrids.delete(i)
                }}
                testId={`design-design-grid-${i}-remove`}
              />
            </Line>
            {openGrids.has(i) && list ? <GridSettings list={list} index={i} grid={g} /> : null}
          </div>
        ))}
    </div>
  )
}

function GridSettings({ list, index, grid: g }: { list: LayoutGrid[]; index: number; grid: LayoutGrid }) {
  const { t, run } = useDesignKit()
  const { nodes, ids } = useDesign()
  const set = (f: (x: LayoutGrid) => LayoutGrid) => ({ command: 'node.set', params: { ids, props: { grids: list.map((x, k) => (k === index ? f(x) : x)) } } })
  const apply = (f: (x: LayoutGrid) => LayoutGrid) => {
    const { command, params } = set(f)
    void run(command, params)
  }
  const size = useCommandField((v) => set((x) => ({ ...x, size: Math.max(1, v) })))
  const offset = useCommandField((v) => set((x) => ({ ...x, offset: Math.max(0, v) })))
  const gutter = useCommandField((v) => set((x) => ({ ...x, gutter: Math.max(0, v) })))
  const count = useCommandField((v) => set((x) => ({ ...x, count: Math.round(Math.min(1000, Math.max(x.align === 'stretch' ? 1 : 0, v))) })))
  const alpha = useCommandField((v) => set((x) => ({ ...x, color: { ...x.color, a: Math.min(1, Math.max(0, v / 100)) } })))
  const field = (name: string, value: number, f: typeof size) => <BindableField ids={ids} nodes={nodes} field={`layoutGrids/${index}/${name}`} value={value} width={COL} ariaLabel={t(name)} onCommit={f.onCommit} scrub={f.scrub} testId={`design-design-grid-${index}-${name}`} />
  const cols = g.kind === 'columns'
  const names = cols ? ['Stretch', 'Left', 'Center', 'Right'] : ['Stretch', 'Top', 'Center', 'Bottom']
  const aligns = ['stretch', 'min', 'center', 'max'] as const
  const span = cols ? t('Width') : t('Height')
  return (
    <div data-testid={`design-design-grid-${index}-panel`}>
      {g.kind === 'grid' ? (
        <>
          <Sub a={t('Size')} />
          <Line>{field('size', g.size, size)}</Line>
        </>
      ) : (
        <>
          <Sub a={t('Count')} b={t('Type')} />
          <Line>
            <BindableField ids={ids} nodes={nodes} field={`layoutGrids/${index}/count`} value={autoCount(g) ? null : Math.max(1, g.count)} placeholder={g.align === 'stretch' ? '' : t('Auto')} decimals={0} width={COL} ariaLabel={t('Count')} onCommit={count.onCommit} scrub={count.scrub} testId={`design-design-grid-${index}-count`} />
            <MenuField
              label={t(names[Math.max(0, aligns.indexOf(g.align))])}
              width={COL}
              items={aligns.map((a, k) => ({
                label: t(names[k]),
                checked: a === g.align,
                onSelect: () => apply((x) => ({ ...x, align: a, size: a !== 'stretch' && !(x.size > 0) ? 60 : x.size, count: a === 'stretch' ? Math.max(1, x.count) : x.count })),
                testId: `design-design-grid-${index}-align-${a}`,
              }))}
              testId={`design-design-grid-${index}-align`}
            />
          </Line>
          {g.align === 'stretch' ? (
            <>
              <Sub a={span} b={t('Margin')} />
              <Line>
                <NumberField label={span} value={null} placeholder={t('Auto')} disabled width={COL} onCommit={() => undefined} />
                {field('offset', g.offset, offset)}
              </Line>
              <Sub a={t('Gutter')} />
              <Line>{field('gutter', g.gutter, gutter)}</Line>
            </>
          ) : g.align === 'center' ? (
            <>
              <Sub a={span} b={t('Gutter')} />
              <Line>
                {field('size', g.size, size)}
                {field('gutter', g.gutter, gutter)}
              </Line>
            </>
          ) : (
            <>
              <Sub a={span} b={t('Offset')} />
              <Line>
                {field('size', g.size, size)}
                {field('offset', g.offset, offset)}
              </Line>
              <Sub a={t('Gutter')} />
              <Line>{field('gutter', g.gutter, gutter)}</Line>
            </>
          )}
        </>
      )}
      <Sub a={t('Color')} />
      <Line>
        <span className="design-d-swatch design-d-swatch-16" style={{ ['--swatch' as string]: 'transparent', backgroundImage: `linear-gradient(to right, rgb(${g.color.r * 255}, ${g.color.g * 255}, ${g.color.b * 255}) 50%, rgba(${g.color.r * 255}, ${g.color.g * 255}, ${g.color.b * 255}, ${g.color.a}) 50%), repeating-conic-gradient(#d9d9d9 0 25%, #ffffff 0 50%)`, backgroundSize: 'auto, 6px 6px' }} />
        <TextField
          value={hexOf(g.color)}
          width={80}
          ariaLabel={t('Color')}
          onCommit={(text) => {
            const c = parseHex(text)
            if (c) apply((x) => ({ ...x, color: { ...c, a: x.color.a } }))
          }}
          testId={`design-design-grid-${index}-hex`}
        />
        <NumberField value={Math.round(g.color.a * 100)} suffix="%" width={56} min={0} max={100} ariaLabel={t('Opacity')} onCommit={alpha.onCommit} scrub={alpha.scrub} testId={`design-design-grid-${index}-alpha`} />
      </Line>
    </div>
  )
}
