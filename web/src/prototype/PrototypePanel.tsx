/**
 * The Prototype tab (2.6, upstream `panels/proto_panel.rs`): with nothing selected the page's
 * flow starting points and prototype settings; with a layer selected its flow starting point,
 * its interactions (each opens the interaction editor) and its scroll behaviour. Every edit runs
 * a `proto.*` command or `node.set`.
 */
import { useEffect, useRef, useState } from 'react'
import { useDesignKit, useQuery, useSelection } from '../kit/hooks'
import { Checkbox, Dropdown, IconButton, Note, NumberField, Row, SectionHeader, Separator, SubLabels, TextField, columnWidth, type NumberEdit } from '../ui'
import { nameIn, topLevelFrame, useTree, usePrototype, useVariables, isFrameLike, type ProtoAnswer, type Tree } from './data'
import { InteractionEditor } from './InteractionEditor'
import { CUSTOM_DEFAULT, CUSTOM_DEVICE, DEVICES, actionSummary, colorHex, deviceColor, deviceModel, isLandscape, triggerSummary, type Action, type Flow, type Interaction, type PrototypeSettings, type Variables } from './model'
import { RichDropdown } from './RichMenu'
import { openEditor, protoState, useProtoState } from './store'
import { defaultAction, sameInteractions, targetsFor } from './targets'
import './prototype.css'

const RESPONSIVE_TIP = "Resize frames to the device's screen (or the window without a device): their constraints and auto layout respond"

export const numberOf = (edit: NumberEdit, current: number) => (edit.kind === 'set' ? edit.value : edit.apply(current))

export function PrototypePanel() {
  const selection = useSelection()
  const proto = usePrototype()
  const tree = useTree()
  const variables = useVariables()
  const editor = useProtoState((s) => s.editor)
  const first = selection.ids[0] ?? null
  useEditorKeys(proto)

  // The editor closes when its layer is no longer the first selected one (upstream `prototype`).
  useEffect(() => {
    if (editor && editor.node !== first) openEditor(null)
  }, [editor, first])

  return (
    <div className="design-proto" data-testid="design-prototype-panel">
      {first == null ? (
        <>
          <FlowsOverview proto={proto} />
          <Separator />
          <Settings settings={proto?.settings ?? null} />
        </>
      ) : (
        <Selected first={first} ids={selection.ids} proto={proto} tree={tree} variables={variables} />
      )}
    </div>
  )
}

function Selected({ first, ids, proto, tree, variables }: { first: number; ids: number[]; proto: ProtoAnswer | null; tree: Tree | null; variables: Variables | null }) {
  const isTop = tree != null && topLevelFrame(tree, first) === first
  const lists = proto?.interactions ?? []
  const own = lists.find((entry) => entry.id === first)?.interactions ?? []
  const mixed = lists.length > 1 && !sameInteractions(lists.map((entry) => entry.interactions ?? []))
  return (
    <>
      {isTop ? (
        <>
          <FlowSection frame={first} flows={proto?.flows ?? []} />
          <Separator />
        </>
      ) : null}
      {mixed ? <MixedInteractions ids={ids} first={first} proto={proto} tree={tree} /> : <Interactions node={first} list={own} proto={proto} tree={tree} variables={variables} />}
      <Separator />
      <ScrollSection ids={ids} first={first} tree={tree} />
    </>
  )
}

/** One flow row: play, the editable name, preview, remove (`flow_row`). */
function FlowRow({ flow }: { flow: Flow }) {
  const { run, t } = useDesignKit()
  return (
    <div className="design-proto-flow" data-testid={`design-proto-flow-${flow.start}`}>
      <IconButton icon="Play" label={t('Present this flow')} onClick={() => void run('proto.present', { start: flow.start })} testId="design-proto-flow-play" />
      <InlineName value={flow.name} onCommit={(name) => void run('proto.flow.rename', { start: flow.start, name })} label={t('Flow name')} />
      <IconButton icon="Preview" label={t('Preview this flow')} onClick={() => void run('proto.preview', { start: flow.start })} testId="design-proto-flow-preview" />
      <IconButton icon="Minus" label={t('Remove starting point')} onClick={() => void run('proto.flow.remove', { start: flow.start })} testId="design-proto-flow-remove" />
    </div>
  )
}

/** The inline text field of a flow's name (`text_field_inline`): text until hovered or edited. */
function InlineName({ value, onCommit, label }: { value: string; onCommit: (value: string) => void; label: string }) {
  const [text, setText] = useState(value)
  const [editing, setEditing] = useState(false)
  const cancelled = useRef(false)
  useEffect(() => {
    if (!editing) setText(value)
  }, [editing, value])
  return (
    <input
      className="design-proto-inline"
      value={text}
      aria-label={label}
      spellCheck={false}
      onChange={(event) => setText(event.target.value)}
      onFocus={(event) => {
        setEditing(true)
        event.currentTarget.select()
      }}
      onBlur={() => {
        setEditing(false)
        const next = text.trim()
        if (!cancelled.current && next && next !== value) onCommit(next)
        cancelled.current = false
      }}
      onKeyDown={(event) => {
        event.stopPropagation()
        if (event.key === 'Enter') event.currentTarget.blur()
        else if (event.key === 'Escape') {
          cancelled.current = true
          setText(value)
          event.currentTarget.blur()
        }
      }}
      data-testid="design-proto-flow-name"
    />
  )
}

function FlowsOverview({ proto }: { proto: ProtoAnswer | null }) {
  const { run, t, toast } = useDesignKit()
  const flows = proto?.flows ?? []
  const add = () => {
    const free = proto?.frames.find((frame) => !flows.some((flow) => flow.start === frame.id))
    if (free) void run('proto.flow.add', { start: free.id })
    else toast(t('Every top-level frame already starts a flow'))
  }
  return (
    <section data-testid="design-proto-flows">
      <SectionHeader title={t('Flow starting points')} dim={flows.length === 0} actions={[{ icon: 'Plus', label: t('Add starting point'), onClick: add, testId: 'design-proto-flow-add' }]} />
      {flows.length === 0 ? <Note>{t('Select a frame and add a starting point to present it as a flow.')}</Note> : null}
      {flows.map((flow) => (
        <div key={flow.start}>
          <FlowRow flow={flow} />
          {flow.description?.trim() ? (
            <div className="design-proto-flow-description" data-tip={flow.description.trim().length > 40 ? flow.description.trim() : undefined}>
              {flow.description.trim()}
            </div>
          ) : null}
        </div>
      ))}
    </section>
  )
}

/** Prototype settings: device and rotation, colour, custom size, responsive, background (`settings`). */
function Settings({ settings }: { settings: PrototypeSettings | null }) {
  const { run, t } = useDesignKit()
  const set = (params: Record<string, unknown>) => void run('proto.settings', params)
  const model = deviceModel(settings)
  const rotates = model?.rotates === true
  const landscape = isLandscape(settings)
  const device = settings?.device ?? 'None'
  const color = deviceColor(settings)
  const bg = settings?.background ?? { r: 0.118, g: 0.118, b: 0.118, a: 1 }
  const col = columnWidth(240)
  const custom = settings?.custom_size ?? CUSTOM_DEFAULT
  return (
    <section data-testid="design-proto-settings">
      <SectionHeader title={t('Prototype settings')} actions={[{ icon: 'Preview', label: t('Open prototype preview'), onClick: () => void run('proto.preview', {}), testId: 'design-proto-open-preview' }]} />
      <SubLabels labels={[t('Device')]} />
      <Row>
        <Dropdown
          value={device}
          width="100%"
          ariaLabel={t('Device')}
          testId="design-proto-device"
          onChange={(value) => set({ device: value })}
          options={[
            { value: 'None', label: t('None') },
            { separator: true },
            ...DEVICES.map((d) => ({ value: d.name, label: d.name })),
            { separator: true },
            { value: CUSTOM_DEVICE, label: t(CUSTOM_DEVICE) },
          ]}
        />
        {rotates ? <IconButton icon="Rotate" label={landscape ? t('Rotate to portrait') : t('Rotate to landscape')} pressed={landscape} iconSize={16} onClick={() => set({ rotation: landscape ? 'portrait' : 'landscape' })} testId="design-proto-rotate" /> : null}
      </Row>
      {model && model.colors.length > 0 ? (
        <>
          <SubLabels labels={[t('Color')]} />
          <Row>
            {color ? <span className="design-proto-swatch" style={{ background: color.hex }} /> : null}
            <RichDropdown
              label={color?.name ?? ''}
              width="100%"
              ariaLabel={t('Color')}
              testId="design-proto-device-color"
              items={() =>
                model.colors.map((c) => ({
                  key: c.name,
                  height: 22,
                  testId: `design-proto-device-color-${c.name}`,
                  onSelect: () => set({ color: c.name }),
                  render: () => (
                    <>
                      <span className="design-menu-label">{`${c.name === color?.name ? '✓ ' : '    '}${c.name}`}</span>
                      <span className="design-proto-menu-swatch" style={{ background: c.hex }} />
                    </>
                  ),
                }))
              }
            />
          </Row>
        </>
      ) : null}
      {device === CUSTOM_DEVICE ? (
        <>
          <SubLabels labels={[t('Size')]} />
          <Row>
            <NumberField label="W" value={custom[0]} width={col} step={1} min={1} max={10000} decimals={0} onCommit={(edit) => set({ width: Math.round(numberOf(edit, custom[0])) })} testId="design-proto-custom-w" />
            <NumberField label="H" value={custom[1]} width={col} step={1} min={1} max={10000} decimals={0} onCommit={(edit) => set({ height: Math.round(numberOf(edit, custom[1])) })} testId="design-proto-custom-h" />
            <IconButton icon="Rotate" label={t('Swap width and height')} iconSize={16} onClick={() => (custom[0] !== custom[1] ? set({ width: custom[1], height: custom[0] }) : undefined)} testId="design-proto-custom-swap" />
          </Row>
        </>
      ) : null}
      <Row>
        <span data-tip={t(RESPONSIVE_TIP)} style={{ display: 'contents' }}>
          <Checkbox checked={settings?.responsive === true} label={t('Responsive')} onChange={() => set({ responsive: !(settings?.responsive === true) })} testId="design-proto-responsive" />
        </span>
      </Row>
      <SubLabels labels={[t('Background')]} />
      <Row>
        <span className="design-proto-swatch" style={{ background: `#${colorHex(bg)}` }} />
        <TextField
          value={colorHex(bg)}
          width={96}
          ariaLabel={t('Background')}
          testId="design-proto-background"
          onCommit={(text) => {
            const css = cssColor(text)
            if (css) set({ background: css })
          }}
        />
      </Row>
    </section>
  )
}

/** A typed colour (`1E1E1E`, `#fff`, a CSS name) as `#RRGGBB`, or null. */
export function cssColor(text: string): string | null {
  const raw = text.trim()
  if (!raw) return null
  const hex = raw.replace(/^#/, '')
  if (/^[0-9a-f]{3}$/i.test(hex)) return `#${[...hex].map((c) => c + c).join('')}`
  if (/^[0-9a-f]{6}$/i.test(hex)) return `#${hex}`
  if (/^[a-z]+$/i.test(raw) && CSS.supports('color', raw)) {
    const probe = document.createElement('canvas').getContext('2d')
    if (!probe) return null
    probe.fillStyle = raw
    return /^#[0-9a-f]{6}$/i.test(probe.fillStyle) ? probe.fillStyle : null
  }
  return null
}

/** The selected top-level frame's flow, or the buttons to start one (`flow_section`). */
function FlowSection({ frame, flows }: { frame: number; flows: Flow[] }) {
  const { run, t } = useDesignKit()
  const mine = flows.find((flow) => flow.start === frame)
  return (
    <section data-testid="design-proto-flow-section">
      <SectionHeader
        title={t('Flow starting point')}
        dim={!mine}
        actions={
          mine
            ? []
            : [
                { icon: 'Preview', label: t('Preview this frame'), onClick: () => void run('proto.preview', { start: frame }), testId: 'design-proto-preview-frame' },
                { icon: 'Plus', label: t('Add starting point'), onClick: () => void run('proto.flow.add', { start: frame }), testId: 'design-proto-flow-start' },
              ]
        }
      />
      {mine ? (
        <>
          <FlowRow flow={mine} />
          <Row>
            <TextField value={mine.description ?? ''} placeholder={t('Add a description')} width="100%" onCommit={(description) => void run('proto.flow.rename', { start: frame, description })} testId="design-proto-flow-description" />
          </Row>
        </>
      ) : null}
    </section>
  )
}

/** The layer's interactions (`interactions`): add, open the editor, remove. */
function Interactions({ node, list, proto, tree, variables }: { node: number; list: Interaction[]; proto: ProtoAnswer | null; tree: Tree | null; variables: Variables | null }) {
  const { run, t } = useDesignKit()
  const editor = useProtoState((s) => s.editor)
  const rows = useRef<Map<number, HTMLDivElement>>(new Map())
  const nameOf = nameIn(tree, proto?.frames)
  const ids = targetsFor(proto, node)
  const add = async () => {
    const action: Action = defaultAction('Navigate to', proto, tree, node, null, variables) ?? { type: 'back' }
    const answer = await run<{ index?: number }>('proto.interaction.add', { ids, trigger: { type: 'onClick' }, action })
    if (answer) openEditor({ node, index: answer.index ?? 0 })
  }
  const open = editor && editor.node === node && list[editor.index] ? editor : null
  return (
    <section data-testid="design-proto-interactions">
      <SectionHeader title={t('Interactions')} dim={list.length === 0} actions={[{ icon: 'Plus', label: t('Add interaction'), onClick: () => void add(), testId: 'design-proto-interaction-add' }]} />
      {list.map((interaction, index) => {
        const active = open?.index === index
        const summary = interaction.actions.map((a) => actionSummary(a, nameOf, variables, t)).join(', ')
        return (
          <div
            key={index}
            className="design-proto-interaction"
            ref={(el) => {
              if (el) rows.current.set(index, el)
              else rows.current.delete(index)
            }}
            data-testid={`design-proto-interaction-${index}`}
          >
            <button type="button" className="design-proto-interaction-body" aria-pressed={active} onClick={() => openEditor(active ? null : { node, index })} data-testid={`design-proto-interaction-open-${index}`}>
              <span className="design-proto-glyph" aria-hidden="true" />
              <span className="design-proto-interaction-text">
                <span className="design-proto-interaction-trigger">{triggerSummary(interaction.trigger, t)}</span>
                <span className="design-proto-interaction-actions">{summary}</span>
              </span>
            </button>
            <IconButton
              icon="Minus"
              label={t('Remove interaction')}
              onClick={() => {
                openEditor(null)
                void run('proto.interaction.remove', { ids, index })
              }}
              testId={`design-proto-interaction-remove-${index}`}
            />
          </div>
        )
      })}
      {open && list[open.index] ? <InteractionEditor node={node} index={open.index} interaction={list[open.index]!} anchor={rows.current.get(open.index) ?? null} proto={proto} tree={tree} variables={variables} /> : null}
    </section>
  )
}

/** Several layers with different interactions (`mixed_interactions`). */
function MixedInteractions({ ids, first, proto, tree }: { ids: number[]; first: number; proto: ProtoAnswer | null; tree: Tree | null }) {
  const { run, t } = useDesignKit()
  const replace = async () => {
    const action = defaultAction('Navigate to', proto, tree, first, null, null) ?? { type: 'back' }
    const steps = [
      { command: 'proto.interaction.clear', params: { ids } },
      { command: 'proto.interaction.add', params: { ids, trigger: { type: 'onClick' }, action } },
    ]
    if (await run('command.batch', { label: 'Replace interactions', commands: steps })) openEditor({ node: first, index: 0 })
  }
  return (
    <section data-testid="design-proto-interactions-mixed">
      <SectionHeader title={t('Interactions')} actions={[{ icon: 'Plus', label: t('Replace with a new interaction'), onClick: () => void replace(), testId: 'design-proto-interaction-replace' }]} />
      <Row>
        <span className="design-secondary design-r11">{t('Mixed interactions: click + to replace them')}</span>
      </Row>
      <Row height={28}>
        <button type="button" className="design-proto-egui-button" onClick={() => void run('proto.interaction.clear', { ids })} data-testid="design-proto-interaction-clear">
          {t('Remove all')}
        </button>
      </Row>
    </section>
  )
}

/**
 * Scroll behaviour (`scroll_section`): Position for layers in a frame, Overflow for frames, read
 * from the selected layers' `overflow` and `scrollPosition` (`query('selection')`); mixed values
 * show no value.
 */
function ScrollSection({ ids, first, tree }: { ids: number[]; first: number; tree: Tree | null }) {
  const { run, t } = useDesignKit()
  const selection = useQuery<{ nodes: Array<{ id: number; overflow?: string; scrollPosition?: string }> }>('selection')
  const node = tree?.nodes.get(first)
  const parent = node?.parent != null ? tree?.nodes.get(node.parent) : undefined
  const inFrame = isFrameLike(parent?.type)
  const isFrame = isFrameLike(node?.type)
  if (!inFrame && !isFrame) return null
  const shown = (prop: 'overflow' | 'scrollPosition', fallback: string) => {
    const values = ids.map((id) => selection?.nodes.find((n) => n.id === id)?.[prop] ?? fallback)
    return values.every((v) => v === values[0]) ? values[0]! : null
  }
  const set = (prop: 'overflow' | 'scrollPosition', value: string) => void run('node.set', { ids, props: { [prop]: value } })
  return (
    <section data-testid="design-proto-scroll">
      <SectionHeader title={t('Scroll behavior')} />
      {inFrame ? (
        <Row>
          <span className="design-proto-row-label">{t('Position')}</span>
          <Dropdown
            value={shown('scrollPosition', 'scrolls')}
            width="100%"
            ariaLabel={t('Position')}
            testId="design-proto-scroll-position"
            onChange={(value) => set('scrollPosition', value)}
            options={[
              { value: 'scrolls', label: t('Scroll with parent') },
              { value: 'fixed', label: t('Fixed') },
              { value: 'sticky', label: t('Sticky') },
            ]}
          />
        </Row>
      ) : null}
      {isFrame ? (
        <Row>
          <span className="design-proto-row-label">{t('Overflow')}</span>
          <Dropdown
            value={shown('overflow', 'none')}
            width="100%"
            ariaLabel={t('Overflow')}
            testId="design-proto-overflow"
            onChange={(value) => set('overflow', value)}
            options={[
              { value: 'none', label: t('No scrolling') },
              { value: 'horizontal', label: t('Horizontal') },
              { value: 'vertical', label: t('Vertical') },
              { value: 'both', label: t('Both directions') },
            ]}
          />
        </Row>
      ) : null}
    </section>
  )
}

/** Delete or Backspace with the editor open removes that interaction; Escape closes it (upstream `delete_selected_interaction`, `close_editor`). */
export function useEditorKeys(proto: ProtoAnswer | null) {
  const { run } = useDesignKit()
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      const { editor, capturing } = protoState()
      if (!editor || capturing) return
      const target = event.target
      if (target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName))) return
      if (document.querySelector('.design-menu, .design-rich-menu')) return
      if (event.key === 'Escape') {
        openEditor(null)
      } else if (event.key === 'Delete' || event.key === 'Backspace') {
        openEditor(null)
        void run('proto.interaction.remove', { ids: targetsFor(proto, editor.node), index: editor.index })
      } else return
      event.preventDefault()
      event.stopPropagation()
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [proto, run])
}
