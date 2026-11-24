/**
 * Dev Mode's Inspect panel (2.6, upstream `panels/right/inspect.rs` and `panels/dev_mode.rs`).
 * Nothing selected: the ready-for-dev list, the annotation categories and the page's kept
 * measurements. A layer selected: its status (Focus view, Compare changes), annotations, dev
 * resources, measurements, inspected properties in a colour format, assets, the component
 * playground, code and Connect to code.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { Icon } from '../icons/Icon'
import { useDesignKit, useQuery, useSelection } from '../kit/hooks'
import { Button, IconButton, Note, Popover, Row, SectionHeader, Separator, SubLabels, TextField, type MenuEntry } from '../ui'
import { loadCheckpoint, useCheckpoints } from './checkpoints'
import { COLOR_FORMATS, LANGUAGES, STATUS_COLORS, copyText, setDev, useDev, type FocusState, type ReadyFilter } from './store'
import { MenuDropdown, NoteField, PropRow, TwoLineRow, toBytes, useThumbnail, type Prop } from './widgets'
import './dev.css'

export type Section = { title: string; rows: Prop[] }
type Category = { id: string; name: string; color: string; count: number }
type Annotation = { id: number; index: number; label: string; name: string; category: { id: string; name: string; color: string } | null; properties: Array<Prop & { property: string }> }
type Redline = { from: number; fromName: string; index: number; to?: number | null; toName?: string | null; side?: string | null; labels: string[] }
type SelectionNode = { id: number; name: string; type: string; devStatus?: string; devChanged?: boolean }
type LayerRow = { id: number; type: string; children?: LayerRow[] }

/** The pinnable properties (engine `annotations::PINNABLE`): id, label, inspected section, row label. */
const PINNABLE: Array<[string, string, string, string | null]> = [
  ['width', 'Width', 'Layout', 'Width'],
  ['height', 'Height', 'Layout', 'Height'],
  ['top', 'Top', 'Layout', 'Top'],
  ['left', 'Left', 'Layout', 'Left'],
  ['rotation', 'Rotation', 'Layout', 'Rotation'],
  ['opacity', 'Opacity', 'Layout', 'Opacity'],
  ['cornerRadius', 'Corner radius', 'Layout', 'Radius'],
  ['direction', 'Direction', 'Auto layout', 'Direction'],
  ['gap', 'Gap', 'Auto layout', 'Gap'],
  ['padding', 'Padding', 'Auto layout', 'Padding'],
  ['alignment', 'Alignment', 'Auto layout', 'Align'],
  ['fontFamily', 'Font', 'Typography', 'Font'],
  ['fontStyle', 'Font style', 'Typography', 'Style'],
  ['fontSize', 'Font size', 'Typography', 'Size'],
  ['lineHeight', 'Line height', 'Typography', 'Line height'],
  ['letterSpacing', 'Letter spacing', 'Typography', 'Letter spacing'],
  ['textAlign', 'Text align', 'Typography', 'Align'],
  ['fill', 'Fill', 'Fills', null],
  ['stroke', 'Stroke', 'Strokes', null],
  ['strokeWeight', 'Stroke weight', 'Strokes', 'Weight'],
  ['effects', 'Effects', 'Effects', null],
]

/** Whether the inspected `sections` have something for pinnable `p` (`annotations::applies`). */
/**
 * Text wrapped as egui wraps it: a row breaks at its last space when it has one, and only falls
 * back to a dash (or any character) inside a word too long for a row. Browsers take the last break
 * of any kind, so words with a dash are kept whole unless they cannot fit (`.design-egui-word`).
 */
function eguiWrapped(text: string) {
  return text.split(/(\s+)/).map((part, i) => (part.includes('-') && !/\s/.test(part) ? <span key={i} className="design-egui-word">{part}</span> : part))
}

function applies(sections: Section[], [, , section, row]: (typeof PINNABLE)[number]) {
  const s = sections.find((item) => item.title === section)
  if (!s) return false
  if (row) return s.rows.some((r) => r.label === row)
  return s.rows.some((r) => !PINNABLE.some(([, , other, label]) => other === section && label === r.label))
}

export const CATEGORY_COLORS = ['#2F6FEB', '#8A4FE0', '#1E9E6A', '#D97A06', '#D63A3A', '#D13F9B', '#1192A8', '#6B7280']
const MAX_ANNOTATIONS = 64
const MAX_RESOURCES = 32

/** The status colour of a frame (`canvas::status_label`): ready, changed, completed; else none. */
export function statusColor(status: string | undefined, changed: boolean | undefined): string | null {
  if (status === 'readyForDev') return changed ? STATUS_COLORS.changed : STATUS_COLORS.readyForDev
  if (status === 'completed') return STATUS_COLORS.completed
  return null
}

export function DevPanel() {
  const selection = useSelection()
  return (
    <div className="design-dev" data-testid="design-dev-panel">
      {selection.count ? <LayerInspect ids={selection.ids} /> : <NothingSelected />}
    </div>
  )
}

// ------------------------------------------------------------------ nothing selected

function NothingSelected() {
  const { t } = useDesignKit()
  return (
    <>
      <SectionHeader title={t('Inspect')} />
      <Note>{t('Select a layer to see its properties and code')}</Note>
      <Note>{t('Hover other layers to measure the distance')}</Note>
      <ReadyList />
      <Categories />
      <Measurements ids={[]} parentOf={null} />
    </>
  )
}

const FILTERS: Array<{ value: ReadyFilter; label: string }> = [
  { value: 'readyForDev', label: 'Ready for dev' },
  { value: 'changed', label: 'Changed' },
  { value: 'completed', label: 'Completed' },
  { value: 'any', label: 'Any status' },
]

function ReadyList() {
  const { t, run, viewTo, inspect } = useDesignKit()
  const dev = useDev()
  const status = dev.readyFilter === 'changed' ? 'readyForDev' : dev.readyFilter
  const answer = useQuery<{ items: Array<{ id: number; name: string; page: number; status: string; changed: boolean }> }>('readyForDev', { status })
  const items = (answer?.items ?? []).filter((item) => dev.readyFilter !== 'changed' || item.changed)
  const filter = FILTERS.find((item) => item.value === dev.readyFilter) ?? FILTERS[0]
  return (
    <>
      <Separator />
      <SectionHeader title={t('Ready for dev')} dim={items.length === 0} />
      <Row>
        <MenuDropdown
          label={t(filter.label)}
          icon="Code"
          width="100%"
          testId="design-dev-ready-filter"
          items={() => FILTERS.map((item) => ({ label: t(item.label), checked: item.value === dev.readyFilter, onSelect: () => setDev({ readyFilter: item.value }), testId: `design-dev-ready-filter-${item.value}` }))}
        />
      </Row>
      {items.length === 0 ? (
        <Note>{t('Nothing here yet. Set a frame\'s status to list it.')}</Note>
      ) : (
        <div className="design-dev-list">
          {items.map((item) => (
            <div
              key={item.id}
              className="design-dev-ready"
              role="button"
              data-tip={t('Go to {name}', { name: item.name })}
              data-testid={`design-dev-ready-${item.id}`}
              onClick={async () => {
                await run('select.set', { ids: [item.id] })
                await viewTo('view.zoomSelection')
              }}
            >
              <span className="design-dev-dot" style={{ background: statusColor(item.status, item.changed) ?? 'var(--design-text-tertiary)' }} />
              <span className="design-dev-ready-name">{item.name}</span>
              {item.changed ? <span className="design-dev-badge" style={{ background: STATUS_COLORS.changed }}>{t('Changed')}</span> : null}
              <span className="design-dev-ready-page">{inspect.pages.find((page) => page.id === item.page)?.name ?? ''}</span>
              <span className="design-dev-ready-focus" onClick={(event) => event.stopPropagation()}>
                <IconButton icon="Frame" label={t('Open in focus view')} size={22} onClick={() => void focusOn(run, item.id)} testId={`design-dev-ready-focus-${item.id}`} />
              </span>
            </div>
          ))}
        </div>
      )}
    </>
  )
}

/** Runs `dev.focus` (the focus bar follows `dev.focusState`). */
export async function focusOn(run: ReturnType<typeof useDesignKit>['run'], id: number | null, direction?: 'next' | 'previous') {
  await run('dev.focus', direction ? { direction } : { id })
}

function Categories() {
  const { t, run } = useDesignKit()
  const dev = useDev()
  const answer = useQuery<{ categories: Category[] }>('annotationCategories')
  const categories = answer?.categories ?? []
  const only = categories.find((c) => c.id === dev.categories)
  const label = dev.categories === 'none' ? t('No category') : only ? only.name : t('All categories')
  const add = () => {
    let k = 1
    while (categories.some((c) => c.name.toLowerCase() === `category ${k}`)) k += 1
    void run('annotation.addCategory', { name: `Category ${k}` })
  }
  return (
    <>
      <Separator />
      <SectionHeader title={t('Annotations')} actions={[{ icon: 'Plus', label: t('Add a category'), onClick: add, testId: 'design-dev-add-category' }]} />
      <Row>
        <MenuDropdown
          label={label}
          icon={only ? undefined : 'Comment'}
          dot={only?.color}
          width="100%"
          tip={t('Annotations shown on the canvas')}
          testId="design-dev-category-filter"
          items={() => [
            { label: t('All categories'), checked: dev.categories === 'all', onSelect: () => setDev({ categories: 'all' }) },
            { label: t('No category'), checked: dev.categories === 'none', onSelect: () => setDev({ categories: 'none' }) },
            ...categories.map((c) => ({ label: c.name, checked: dev.categories === c.id, onSelect: () => setDev({ categories: c.id }) })),
          ]}
        />
      </Row>
      <SubLabels labels={[t('Categories')]} />
      {categories.map((c) => (
        <CategoryRow key={c.id} category={c} />
      ))}
      <div style={{ height: 4 }} />
    </>
  )
}

function CategoryRow({ category }: { category: Category }) {
  const { t, run } = useDesignKit()
  const swatch = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <div className="design-dev-category" data-testid={`design-dev-category-${category.id}`}>
      <button ref={swatch} type="button" className="design-dev-category-swatch" style={{ background: category.color }} aria-label={t('Colour')} data-tip={t('Colour')} onClick={() => setOpen((now) => !now)} />
      <Popover open={open} anchor={swatch.current} onClose={() => setOpen(false)} placement="below-start" className="design-dev-palette">
        {CATEGORY_COLORS.map((color) => (
          <button
            key={color}
            type="button"
            className="design-dev-palette-swatch"
            style={{ background: color }}
            aria-pressed={color.toLowerCase() === category.color.toLowerCase()}
            aria-label={color}
            onClick={() => {
              setOpen(false)
              void run('annotation.editCategory', { id: category.id, color })
            }}
          />
        ))}
      </Popover>
      <input
        className="design-dev-inline"
        defaultValue={category.name}
        key={category.name}
        aria-label={t('Name')}
        spellCheck={false}
        onKeyDown={(event) => {
          event.stopPropagation()
          if (event.key === 'Enter') event.currentTarget.blur()
          if (event.key === 'Escape') {
            event.currentTarget.value = category.name
            event.currentTarget.blur()
          }
        }}
        onBlur={(event) => {
          const name = event.currentTarget.value.trim()
          if (name && name !== category.name) void run('annotation.editCategory', { id: category.id, name })
          else event.currentTarget.value = category.name
        }}
      />
      <IconButton icon="Minus" size={22} label={t('Delete category (its annotations stay)')} onClick={() => void run('annotation.removeCategory', { id: category.id })} testId={`design-dev-category-remove-${category.id}`} />
    </div>
  )
}

const SIDES = [
  ['top', 'Along the top'],
  ['right', 'Along the right'],
  ['bottom', 'Along the bottom'],
  ['left', 'Along the left'],
] as const

/** Kept measurements: with `ids`, those touching the first, and the "Add measurement" menu. */
function Measurements({ ids, parentOf }: { ids: number[]; parentOf: Map<number, number | null> | null }) {
  const { t, run } = useDesignKit()
  const all = useQuery<Redline[]>('redlines', {})
  const first = ids[0] ?? null
  const rows = (all ?? []).filter((m) => first == null || m.from === first || m.to === first)
  if (first == null && rows.length === 0) return null
  const side = (s: string | null | undefined) => t(SIDES.find(([key]) => key === s)?.[1] ?? 'Along the left')
  const around = first != null && parentOf?.get(first) != null
  return (
    <>
      <Separator />
      <SectionHeader title={t('Measurements')} dim={rows.length === 0} />
      {rows.map((m) => (
        <TwoLineRow
          key={`${m.from}:${m.index}`}
          icon="Measure"
          title={m.to != null ? `${m.fromName} → ${m.toName ?? ''}` : `${m.fromName} · ${side(m.side)}`}
          sub={m.labels.join(' · ')}
          onClick={() => void run('select.set', { ids: [m.from] })}
          reserve={37}
          testId={`design-dev-measurement-${m.from}-${m.index}`}
        >
          <IconButton icon="Minus" size={22} label={t('Remove measurement')} onClick={() => void run('devMode.removeMeasurement', { from: m.from, index: m.index })} testId={`design-dev-measurement-remove-${m.from}-${m.index}`} />
        </TwoLineRow>
      ))}
      {first != null ? (
        <Row>
          <MenuDropdown
            label={t('Add measurement')}
            icon="Measure"
            width="100%"
            tip={t('Keep a measurement on the canvas for developers')}
            testId="design-dev-add-measurement"
            items={() => {
              const items: MenuEntry[] = []
              if (ids.length >= 2) items.push({ label: t('Between the selected layers'), onSelect: () => void run('devMode.addMeasurement', { from: ids[0], to: ids[1] }) })
              if (around) items.push({ label: t('To the layer around it'), onSelect: () => void run('devMode.addMeasurement', { from: first }) })
              for (const [key, label] of SIDES) items.push({ label: t(label), onSelect: () => void run('devMode.addMeasurement', { from: first, side: key }), testId: `design-dev-add-measurement-${key}` })
              return items
            }}
          />
        </Row>
      ) : null}
      <div style={{ height: 4 }} />
    </>
  )
}

// ------------------------------------------------------------------ a layer selected

/** Parents and types from the layers tree (the selection's ancestors are always listed). */
function useTree() {
  const layers = useQuery<{ page: number; rows: LayerRow[] }>('layers')
  return useMemo(() => {
    const parentOf = new Map<number, number | null>()
    const typeOf = new Map<number, string>()
    const walk = (rows: LayerRow[], parent: number | null) => {
      for (const row of rows) {
        parentOf.set(row.id, parent)
        typeOf.set(row.id, row.type)
        if (row.children) walk(row.children, row.id)
      }
    }
    walk(layers?.rows ?? [], null)
    return { parentOf, typeOf }
  }, [layers])
}

const FRAME_LIKE = new Set(['FRAME', 'COMPONENT', 'COMPONENT_SET'])

function LayerInspect({ ids }: { ids: number[] }) {
  const { t } = useDesignKit()
  const dev = useDev()
  const first = ids[0]
  const selection = useQuery<{ nodes: SelectionNode[] }>('selection')
  const nodes = selection?.nodes ?? []
  const node = nodes.find((n) => n.id === first)
  const tree = useTree()
  const hasStatus = (id: number) => {
    const type = tree.typeOf.get(id)
    if (type === 'SECTION') return true
    if (!type || !FRAME_LIKE.has(type)) return false
    const parent = tree.parentOf.get(id)
    return parent == null || tree.typeOf.get(parent) === 'SECTION'
  }
  const statusShown = nodes.length > 0 && ids.every((id) => tree.typeOf.has(id) && hasStatus(id))
  const props = useQuery<{ sections: Section[] }>('devInspect', { id: first, colorFormat: dev.colorFormat })
  const sections = props?.sections ?? []
  const color = COLOR_FORMATS.find((item) => item.value === dev.colorFormat) ?? COLOR_FORMATS[0]
  const insideInstance = useMemo(() => {
    for (let at = tree.parentOf.get(first); at != null; at = tree.parentOf.get(at)) if (tree.typeOf.get(at) === 'INSTANCE') return true
    return false
  }, [first, tree])
  return (
    <>
      <SectionHeader title={node?.name ?? ''} testId="design-dev-layer-header" />
      {statusShown ? <Status nodes={nodes.filter((n) => ids.includes(n.id))} /> : null}
      <Annotations id={first} sections={sections} />
      <DevResources id={first} insideInstance={insideInstance} />
      <Measurements ids={ids} parentOf={tree.parentOf} />
      <Row>
        <span className="design-dev-label">{t('Color format')}</span>
        <MenuDropdown label={color.label} width={72} testId="design-dev-color-format" items={() => COLOR_FORMATS.map((item) => ({ label: item.label, onSelect: () => setDev({ colorFormat: item.value }), testId: `design-dev-color-format-${item.value}` }))} />
      </Row>
      <InspectSections sections={sections} />
      <Assets ids={ids} />
      <CodeSections id={first} type={tree.typeOf.get(first) ?? node?.type ?? ''} />
    </>
  )
}

/** The inspected properties' sections (upstream `inspect_section`), as the Inspect tab and the presenter show them. */
export function InspectSections({ sections, testId = 'design-dev-section' }: { sections: Section[]; testId?: string }) {
  const { t } = useDesignKit()
  return (
    <>
      {sections.map((section) => (
        <div key={section.title} data-testid={`${testId}-${section.title.replace(/\W+/g, '-').toLowerCase()}`}>
          <Separator />
          <div className="design-dev-section-title">{t(section.title)}</div>
          {section.rows.map((row, index) => (
            <PropRow key={index} prop={row} />
          ))}
          <div style={{ height: 4 }} />
        </div>
      ))}
    </>
  )
}

function Status({ nodes }: { nodes: SelectionNode[] }) {
  const { t, run } = useDesignKit()
  const focused = useQuery<FocusState | null>('dev.focusState')
  const designKit = useDesignKit()
  // The newest saved version the design's history here reaches (the host's checkpoints).
  const newestVersion = useCheckpoints(true).list.find((v) => v.available) ?? null
  const key = (n: SelectionNode) => `${n.devStatus ?? 'none'}:${n.devStatus === 'readyForDev' && n.devChanged ? 1 : 0}`
  const same = nodes.every((n) => key(n) === key(nodes[0]))
  const one = nodes[0]
  const label = !same ? t('Mixed') : one.devStatus === 'readyForDev' ? t(one.devChanged ? 'Changed' : 'Ready for dev') : one.devStatus === 'completed' ? t('Completed') : t('No status')
  const ids = nodes.map((n) => n.id)
  const single = nodes.length === 1 ? one : null
  const changed = single?.devStatus === 'readyForDev' && single.devChanged === true
  const newest = !changed ? newestVersion : null
  return (
    <>
      <Separator />
      <SubLabels labels={[t('Status')]} />
      <Row>
        <MenuDropdown
          label={label}
          icon="Code"
          width="100%"
          testId="design-dev-status"
          items={() =>
            [
              ['none', 'No status'],
              ['readyForDev', 'Ready for dev'],
              ['completed', 'Completed'],
            ].map(([value, text]) => ({ label: t(text), onSelect: () => void run('node.set', { ids, props: { devStatus: value } }), testId: `design-dev-status-${value}` }))
          }
        />
      </Row>
      {single ? (
        <Row>
          {focused?.id !== single.id ? (
            <Button tip={t('Show only this on the canvas')} onClick={() => void focusOn(run, single.id)} testId="design-dev-focus-view">
              {t('Focus view')}
            </Button>
          ) : null}
          {changed || newest ? (
            <Button variant={changed ? 'primary' : 'secondary'} tip={t(changed ? 'What changed since it was marked ready for dev' : 'What changed since a saved version')} onClick={async () => {
                const version = newest ? await loadCheckpoint(designKit, newest).then((v) => v.version, () => null) : null
                if (newest && !version) return
                void run('dev.compare', { id: single.id, version, show: true })
              }} testId="design-dev-compare">
              {t('Compare changes')}
            </Button>
          ) : null}
        </Row>
      ) : null}
      <div style={{ height: 4 }} />
    </>
  )
}

function Annotations({ id, sections }: { id: number; sections: Section[] }) {
  const { t, run } = useDesignKit()
  const answer = useQuery<{ annotations: Annotation[] }>('annotations', { id })
  const categories = useQuery<{ categories: Category[] }>('annotationCategories')?.categories ?? []
  const list = (answer?.annotations ?? []).filter((a) => a.id === id)
  return (
    <>
      <Separator />
      <SectionHeader title={t('Annotations')} dim={list.length === 0} />
      {list.map((a) => {
        const pinned = a.properties.map((p) => p.property)
        return (
          <div key={a.index} className="design-dev-annotation" data-testid={`design-dev-annotation-${a.index}`}>
            <div className="design-dev-note-head">
              <span>{t('Note {n}', { n: a.index + 1 })}</span>
              <IconButton icon="Minus" size={22} label={t('Remove annotation')} onClick={() => void run('annotation.remove', { id, index: a.index })} testId={`design-dev-annotation-remove-${a.index}`} />
            </div>
            <Row height={30}>
              <MenuDropdown
                label={a.category?.name ?? t('No category')}
                dot={a.category?.color}
                width="100%"
                testId={`design-dev-annotation-category-${a.index}`}
                items={() => [
                  { label: t('No category'), checked: !a.category, onSelect: () => void run('annotation.edit', { id, index: a.index, category: null }) },
                  ...categories.map((c) => ({ label: c.name, checked: a.category?.id === c.id, onSelect: () => void run('annotation.edit', { id, index: a.index, category: c.id }) })),
                ]}
              />
            </Row>
            <NoteField
              value={a.label}
              rows={2}
              placeholder={t('Add a note for developers')}
              testId={`design-dev-annotation-note-${a.index}`}
              onCommit={(label) => {
                if (!label.trim() && a.properties.length === 0) void run('annotation.remove', { id, index: a.index })
                else void run('annotation.edit', { id, index: a.index, label })
              }}
            />
            <div style={{ height: 2 }} />
            {a.properties.map((p) => (
              <PropRow key={p.property} prop={{ ...p, value: p.value || '—' }} />
            ))}
            <Row height={30}>
              <MenuDropdown
                label={t('Pin a property')}
                icon="Plus"
                width="100%"
                testId={`design-dev-annotation-pin-${a.index}`}
                items={() =>
                  PINNABLE.filter((p) => pinned.includes(p[0]) || applies(sections, p)).map((p) => ({
                    label: t(p[1]),
                    checked: pinned.includes(p[0]),
                    testId: `design-dev-pin-${p[0]}`,
                    onSelect: () => {
                      const next = pinned.includes(p[0]) ? pinned.filter((q) => q !== p[0]) : [...pinned, p[0]]
                      if (!next.length && !a.label.trim()) void run('annotation.remove', { id, index: a.index })
                      else void run('annotation.edit', { id, index: a.index, properties: next })
                    },
                  }))
                }
              />
            </Row>
            <div style={{ height: 6 }} />
          </div>
        )
      })}
      {list.length < MAX_ANNOTATIONS ? (
        <NoteField
          key={`new:${id}:${list.length}`}
          value=""
          rows={1}
          clearOnCommit
          placeholder={t('Add an annotation for developers')}
          testId="design-dev-annotation-new"
          onCommit={(label) => {
            if (label.trim()) void run('annotation.add', { id, label })
          }}
        />
      ) : null}
      <div style={{ height: 4 }} />
    </>
  )
}

function DevResources({ id, insideInstance }: { id: number; insideInstance: boolean }) {
  const { t, run } = useDesignKit()
  const list = useQuery<Array<{ name: string; url: string; index?: number; from?: number }>>('devResources', { id }) ?? []
  const own = list.filter((item) => item.index != null).length
  const canAdd = !insideInstance && own < MAX_RESOURCES
  const [adding, setAdding] = useState(false)
  const [editing, setEditing] = useState<number | null>(null)
  const names = useQuery<{ nodes: Array<{ id: number; name: string }> }>('selection')
  const fromName = (from: number) => names?.nodes.find((n) => n.id === from)?.name
  const open = (url: string) => {
    if (/^https?:\/\//i.test(url)) window.open(url, '_blank', 'noopener')
  }
  return (
    <>
      <Separator />
      <SectionHeader title={t('Dev resources')} dim={list.length === 0 && !adding} actions={canAdd ? [{ icon: 'Plus', label: t('Add dev resource'), onClick: () => setAdding((now) => !now), testId: 'design-dev-add-resource' }] : []} />
      {list.map((item, k) =>
        item.index != null && editing === item.index ? (
          <div key={k}>
            <SubLabels labels={[t('Name'), t('Link')]} />
            <Row>
              <TextField value={item.name} placeholder={t('Name')} width="calc(50% - 20px)" onCommit={(name) => void run('devResource.edit', { id, index: item.index, name })} />
              <TextField value={item.url} placeholder="https://…" width="calc(50% - 20px)" onCommit={(url) => void run('devResource.edit', { id, index: item.index, url })} />
              <IconButton icon="Check" label={t('Done')} onClick={() => setEditing(null)} />
            </Row>
          </div>
        ) : (
          <TwoLineRow key={k} icon="Link" title={item.name} accent sub={item.from != null ? t('From {name}', { name: fromName(item.from) ?? '' }) : item.url} tip={item.url} onClick={() => open(item.url)} reserve={item.index != null ? 68 : 16} testId={`design-dev-resource-${k}`}>
            {item.index != null ? (
              <>
                <IconButton icon="Pencil" size={22} label={t('Edit dev resource')} onClick={() => setEditing(item.index ?? null)} />
                <IconButton
                  icon="Minus"
                  size={22}
                  label={t('Remove dev resource')}
                  onClick={() => {
                    setEditing(null)
                    void run('devResource.remove', { id, index: item.index })
                  }}
                  testId={`design-dev-resource-remove-${k}`}
                />
              </>
            ) : null}
          </TwoLineRow>
        ),
      )}
      {adding && canAdd ? (
        <Row>
          <TextField
            value=""
            autoFocus
            width="100%"
            placeholder={t('Paste a link (https://…)')}
            testId="design-dev-resource-new"
            onCommit={async (url) => {
              if (url.trim() && (await run('devResource.add', { id, url: url.trim() })) != null) setAdding(false)
            }}
          />
        </Row>
      ) : list.length === 0 && canAdd ? (
        <Note>{t('Link docs, code or tickets for developers.')}</Note>
      ) : null}
      <div style={{ height: 4 }} />
    </>
  )
}

const FORMATS = ['PNG', 'JPG', 'SVG', 'PDF', 'WebP', 'XML'] as const

function Assets({ ids }: { ids: number[] }) {
  const { t, run } = useDesignKit()
  const answer = useQuery<{ assets: Array<{ id: number; name: string; kind: string; exports: string[] }> }>('devAssets', { ids })
  const assets = answer?.assets ?? []
  return (
    <>
      <Separator />
      <SectionHeader title={t('Assets')} actions={assets.length ? [{ icon: 'Export', label: t('Export all'), onClick: () => void run('file.exportLayers', { ids: assets.map((a) => a.id), assets: true }), testId: 'design-dev-export-all' }] : []} />
      {assets.length === 0 ? <Note>{t('No assets: no export settings, icons or images. Add export settings in the Design tab.')}</Note> : assets.map((asset) => <AssetRow key={asset.id} asset={asset} />)}
      <div style={{ height: 4 }} />
    </>
  )
}

const formats = new Map<number, (typeof FORMATS)[number]>()

function AssetRow({ asset }: { asset: { id: number; name: string; exports: string[] } }) {
  const { t, invoke, editor, toast } = useDesignKit()
  const thumb = useThumbnail(asset.id, 40)
  const initial = FORMATS.find((f) => asset.exports[0]?.toUpperCase().startsWith(f.toUpperCase())) ?? 'PNG'
  const [format, setFormat] = useState<(typeof FORMATS)[number]>(formats.get(asset.id) ?? initial)
  const exportIt = async () => {
    const ext = format === 'XML' ? 'xml' : format.toLowerCase()
    const name = `${asset.name.split('/').pop() ?? asset.name}.${ext === 'jpg' ? 'jpg' : ext}`
    try {
      const bytes = toBytes(await invoke('exportNode', String(asset.id), ext, 1))
      await editor.host.exportFile(name, bytes)
      toast(t('Exported {name}', { name }))
    } catch (error) {
      toast(error instanceof Error ? error.message : String(error))
    }
  }
  return (
    <div className="design-dev-asset" role="button" data-tip={t('Export {name}', { name: asset.name })} data-testid={`design-dev-asset-${asset.id}`} onClick={() => void exportIt()}>
      <span className="design-dev-asset-thumb">{thumb ? <img src={thumb} alt="" /> : null}</span>
      <span className="design-dev-asset-name">{asset.name}</span>
      <span className="design-dev-asset-format" onClick={(event) => event.stopPropagation()}>
        <MenuDropdown
          label={format}
          width={74}
          items={() =>
            FORMATS.map((f) => ({
              label: f,
              onSelect: () => {
                formats.set(asset.id, f)
                setFormat(f)
              },
            }))
          }
        />
      </span>
      <span className="design-dev-asset-export">
        <Icon name="Export" size={12} />
      </span>
    </div>
  )
}

type Playground = { code: string; component: number; set?: number | null; properties: Array<{ name: string; type: string; value: unknown; options?: string[] }> }

const COMPONENTISH = new Set(['COMPONENT', 'COMPONENT_SET', 'INSTANCE'])

function CodeSections({ id, type }: { id: number; type: string }) {
  const { t, toast } = useDesignKit()
  const dev = useDev()
  const language = LANGUAGES.find((l) => l.value === dev.language) ?? LANGUAGES[0]
  const [tried, setTried] = useState<{ id: number; values: Record<string, unknown> }>({ id, values: {} })
  const values = tried.id === id ? tried.values : {}
  const isComponent = COMPONENTISH.has(type)
  const playground = useQuery<Playground>('playground', { id, properties: values, language: dev.language === 'tailwind' ? 'react' : dev.language }, { skip: !isComponent })
  const code = useQuery<{ code: string; connected: boolean }>('code', { id, language: dev.language, unit: language.web && dev.rem ? 'rem' : 'px' })
  const triedAny = Object.keys(values).length > 0
  const shown = triedAny && playground ? playground.code : (code?.code ?? '')
  const copy = () => {
    void copyText(shown)
    toast(t('Copied {value}', { value: language.label }))
  }
  return (
    <>
      {isComponent && playground ? <PlaygroundSection id={id} playground={playground} values={values} onValues={(next) => setTried({ id, values: next })} /> : null}
      <Separator />
      <SectionHeader title={t('Code')} actions={[{ icon: 'Export', label: t('Copy'), onClick: copy }]} />
      <Row>
        <MenuDropdown label={language.label} icon="Code" width={120} testId="design-dev-language" items={() => LANGUAGES.map((l) => ({ label: l.label, onSelect: () => setDev({ language: l.value }), testId: `design-dev-language-${l.value}` }))} />
        {language.web ? <MenuDropdown label={dev.rem ? 'rem' : 'px'} width={56} testId="design-dev-unit" items={() => [{ label: 'px', onSelect: () => setDev({ rem: false }) }, { label: 'rem', onSelect: () => setDev({ rem: true }) }]} /> : null}
      </Row>
      {/* egui's label shows a trailing line break as an empty last line; a <pre> does not. */}
      <pre className="design-dev-code" data-testid="design-dev-code">
        {eguiWrapped(shown.endsWith('\n') ? `${shown}\n` : shown)}
      </pre>
      <Row>
        {/* egui's plain `ui.button`: no frame until hovered. */}
        <button type="button" className="design-dev-text-button" onClick={copy} data-testid="design-dev-copy-code">
          {t('Copy {value}', { value: language.label })}
        </button>
      </Row>
      {isComponent && playground ? <ConnectSection id={id} owner={playground.set ?? playground.component} connected={code?.connected === true} /> : null}
    </>
  )
}

/** The playground's instance with the tried values (`playgroundPreview`, drawn on a copy) as an object URL. */
function usePlaygroundPreview(id: number, values: Record<string, unknown>, side: number): string | null {
  const { invoke, revision } = useDesignKit()
  const [url, setUrl] = useState<string | null>(null)
  const key = JSON.stringify(values)
  useEffect(() => {
    let live = true
    let made: string | null = null
    const ratio = window.devicePixelRatio || 1
    void invoke<Uint8Array>('playgroundPreview', JSON.stringify({ id, properties: JSON.parse(key), scale: 2, maxSide: Math.round(side * ratio) })).then(
      (bytes) => {
        if (!live) return
        made = URL.createObjectURL(new Blob([toBytes(bytes) as BlobPart], { type: 'image/png' }))
        setUrl(made)
      },
      () => undefined,
    )
    return () => {
      live = false
      if (made) {
        const old = made
        setTimeout(() => URL.revokeObjectURL(old), 1000)
      }
    }
  }, [id, invoke, key, revision, side])
  return url
}

function PlaygroundSection({ id, playground, values, onValues }: { id: number; playground: Playground; values: Record<string, unknown>; onValues: (values: Record<string, unknown>) => void }) {
  const { t } = useDesignKit()
  const thumb = usePlaygroundPreview(id, values, 208)
  const set = (name: string, value: unknown) => onValues({ ...values, [name]: value })
  const tried = Object.keys(values).length > 0
  return (
    <>
      <Separator />
      <SectionHeader title={t('Playground')} actions={tried ? [{ icon: 'Rotate', label: t('Reset'), onClick: () => onValues({}) }] : []} />
      <div className="design-dev-playground-preview" data-testid="design-dev-playground-preview">{thumb ? <img src={thumb} alt="" data-testid="design-dev-playground-image" /> : null}</div>
      <div style={{ height: 4 }} />
      {playground.properties.map((p) =>
        p.type === 'variant' ? (
          <Row key={p.name}>
            <span className="design-dev-label">{p.name}</span>
            <MenuDropdown label={String(p.value ?? '')} width="100%" testId={`design-dev-play-${p.name}`} items={() => (p.options ?? []).map((o) => ({ label: o, checked: o === p.value, onSelect: () => set(p.name, o) }))} />
          </Row>
        ) : p.type === 'boolean' ? (
          <Row key={p.name} height={28}>
            <button type="button" role="checkbox" className="design-check" aria-checked={p.value === true} onClick={() => set(p.name, p.value !== true)}>
              <span className="design-check-box">{p.value === true ? <Icon name="Check" size={11} /> : null}</span>
              <span>{p.name}</span>
            </button>
          </Row>
        ) : p.type === 'text' ? (
          <Row key={p.name}>
            <span className="design-dev-label">{p.name}</span>
            <TextField value={String(p.value ?? '')} width="100%" onCommit={(text) => set(p.name, text.slice(0, 10000))} />
          </Row>
        ) : null,
      )}
      <Note>{t('What you try here changes this preview and the code below, not the file')}</Note>
    </>
  )
}

type Link = { language: string; importPath?: string; name: string; template: string }

function ConnectSection({ id, owner, connected }: { id: number; owner: number; connected: boolean }) {
  const { t, run, toast } = useDesignKit()
  const dev = useDev()
  const connectable = ['react', 'leptos', 'vue', 'swiftui', 'compose', 'arkui'].includes(dev.language)
  const links = useQuery<{ id: number; owner: number | null; links: Link[] }>('codeLinks', { id }, { skip: !connected })
  const link = connected ? links?.links.find((l) => l.language === dev.language) : undefined
  const params = { id: owner, language: dev.language }
  const set = async (field: string, value: string) => {
    const answer = await run<{ unknown?: string[] }>('codeConnect.set', { ...params, [field]: value })
    if (answer?.unknown?.length) toast(t('Not properties, kept as written: {names}', { names: answer.unknown.join(', ') }))
  }
  const actions = connected
    ? [{ icon: 'Minus' as const, label: t('Disconnect from code'), onClick: () => void run('codeConnect.remove', params), testId: 'design-dev-disconnect' }]
    : connectable
      ? [{ icon: 'Plus' as const, label: t('Connect to code'), onClick: () => void run('codeConnect.set', params), testId: 'design-dev-connect' }]
      : []
  return (
    <div data-testid="design-dev-connect-section" data-for={id}>
      <Separator />
      <SectionHeader title={t('Connect to code')} dim={!connected} actions={actions} />
      {!connected ? (
        <Note>{connectable ? t('Link this component to its code component: instances then show your real snippet with their values.') : t('Choose React, Leptos, Vue, SwiftUI, Compose or ArkUI above to connect this component to its code.')}</Note>
      ) : (
        <>
          <div className="design-dev-connected">
            <span className="design-dev-badge" style={{ background: 'var(--design-success)' }}>
              {t('Connected')}
            </span>
            <span className="design-dev-connected-from">{link ? (link.importPath ? t('{name} from {path}', { name: link.name, path: link.importPath }) : link.name) : ''}</span>
          </div>
          <SubLabels labels={[t('Import path'), t('Name')]} />
          <Row>
            <TextField value={link?.importPath ?? ''} placeholder={t('Import path')} width="calc(50% - 4px)" onCommit={(value) => void set('importPath', value)} />
            <TextField value={link?.name ?? ''} placeholder={t('Component name')} width="calc(50% - 4px)" onCommit={(value) => void set('name', value)} />
          </Row>
          <SubLabels labels={[t('Template')]} />
          <NoteField value={link?.template ?? ''} rows={3} placeholder="<Button size={size}>{label}</Button>" onCommit={(value) => void set('template', value)} />
          <Note>{t('{property} is its value; {property ? "a" : "b"} picks a string. Other {…} stays as written.')}</Note>
        </>
      )}
    </div>
  )
}
