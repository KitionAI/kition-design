/**
 * An interaction's actions in the interaction editor (upstream `proto_editor.rs` `action_list`,
 * `action_item`, `conditional_blocks`, `set_variable_fields`, and `proto_panel.rs`
 * `action_fields`): one open action at a time with its settings, the others as summaries with a
 * "More" menu (move up, move down, remove), "Add action", conditional if / else if / else blocks,
 * and set variable values or expressions. Every edit runs a `proto.action.*` or
 * `proto.conditional.*` command.
 */
import { useRef, useState, type ReactNode } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon, type IconName } from '../icons/Icon'
import { Checkbox, Dropdown, IconButton, Menu, NumberField, Row, Segmented, SubLabels, TextField, type MenuEntry } from '../ui'
import type { ProtoAnswer, Tree } from './data'
import { nameIn } from './data'
import { ACTIONS, ACTION_GROUPS, OVERLAY_POSITIONS, actionLabel, actionSummary, colorHex, exprDisplay, opSymbol, type Action, type ActionLabel, type Expr, type Variables } from './model'
import { cssColor, numberOf } from './PrototypePanel'
import { RichDropdown } from './RichMenu'
import { openPathKey, setOpenPath, useProtoState } from './store'
import { defaultAction, startValue, unavailable, valueJson, type Dests } from './targets'

type Cx = { node: number; index: number; ids: number[]; proto: ProtoAnswer | null; tree: Tree | null; variables: Variables | null }

/** Runs `command` on the action at `path` (for every layer edited together). */
function useRunAt(cx: Cx) {
  const { run } = useDesignKit()
  return <R,>(path: number[], command: string, extra: Record<string, unknown> = {}) => run<R>(command, { ids: cx.ids, index: cx.index, path, ...extra })
}

/** `proto.destinations` kinds per action. */
type DestKind = 'navigate' | 'overlay' | 'changeTo' | 'scrollTo'
type DestAnswer = { kind: string; destinations: Array<{ id: number; name: string; type: string }> }

/** The destinations a default action of kind `label` needs, from `proto.destinations`. */
function useDests(cx: Cx) {
  const { query } = useDesignKit()
  return async (label: ActionLabel): Promise<Dests> => {
    const kind = label === 'Change to' ? 'changeTo' : label === 'Scroll to' ? 'scrollTo' : null
    if (!kind) return {}
    const answer = await query<DestAnswer>('proto.destinations', { id: cx.node, kind }).catch(() => null)
    return answer ? { [kind]: answer.destinations.map((d) => d.id) } : {}
  }
}

const INDENT = 12
const DEFAULT_OPEN = [0]

export function ActionList(props: { node: number; index: number; ids: number[]; actions: Action[]; prefix: number[]; proto: ProtoAnswer | null; tree: Tree | null; variables: Variables | null }) {
  const { actions, prefix, ...cx } = props
  const target = { node: cx.node, index: cx.index }
  const open = useProtoState((s) => s.openPaths[openPathKey(target)]) ?? DEFAULT_OPEN
  const single = prefix.length === 0 && actions.length === 1
  return (
    <div className="design-proto-actions" data-testid={prefix.length ? undefined : 'design-proto-actions'}>
      {actions.map((action, i) => {
        const path = [...prefix, i]
        const isOpen = single || (open.length >= path.length && path.every((p, j) => open[j] === p))
        return <ActionItem key={i} cx={cx} action={action} path={path} count={actions.length} open={isOpen} single={single} />
      })}
      <AddAction cx={cx} path={[...prefix, actions.length]} />
    </div>
  )
}

/** The action kinds as menu entries (a separator before each group). */
function kindEntries(t: (key: string) => string, pick: (label: ActionLabel) => void, current?: ActionLabel): MenuEntry[] {
  return ACTIONS.flatMap((label, i) => [...(ACTION_GROUPS.includes(i) ? [{ type: 'separator' as const }] : []), { label: t(label), checked: current === undefined ? undefined : current === label, onSelect: () => pick(label), testId: `design-proto-action-kind-${label.replace(/\W+/g, '-').toLowerCase()}` }])
}

/** A text button with an icon ("+ Add action", `link_button`). */
function LinkButton({ icon, label, onClick, tip, testId, buttonRef }: { icon: IconName; label: string; onClick: () => void; tip?: string; testId?: string; buttonRef?: React.Ref<HTMLButtonElement> }) {
  return (
    <button ref={buttonRef} type="button" className="design-proto-link" onClick={onClick} data-tip={tip} data-testid={testId}>
      <Icon name={icon} size={12} />
      <span>{label}</span>
    </button>
  )
}

function AddAction({ cx, path }: { cx: Cx; path: number[] }) {
  const { t, toast } = useDesignKit()
  const runAt = useRunAt(cx)
  const button = useRef<HTMLButtonElement>(null)
  const [menu, setMenu] = useState(false)
  const dests = useDests(cx)
  const pick = async (label: ActionLabel) => {
    const action = defaultAction(label, cx.proto, cx.tree, cx.node, null, cx.variables, await dests(label))
    if (!action) return toast(t(unavailable(label)))
    const answer = await runAt<{ path?: number[] }>(path, 'proto.action.add', { action })
    if (answer) setOpenPath({ node: cx.node, index: cx.index }, answer.path ?? path)
  }
  return (
    <Row height={28}>
      <LinkButton buttonRef={button} icon="Plus" label={t('Add action')} onClick={() => setMenu(true)} testId={path.length === 1 ? 'design-proto-add-action' : undefined} />
      {menu ? <Menu tone="light" anchor={button.current} placement="below-start" minWidth={200} items={kindEntries(t, (label) => void pick(label))} onClose={() => setMenu(false)} testId="design-proto-add-action-menu" /> : null}
    </Row>
  )
}

function ActionItem({ cx, action, path, count, open, single }: { cx: Cx; action: Action; path: number[]; count: number; open: boolean; single: boolean }) {
  const { t, toast } = useDesignKit()
  const runAt = useRunAt(cx)
  const target = { node: cx.node, index: cx.index }
  const i = path[path.length - 1] ?? 0
  const removable = path.length > 1 || count > 1
  const more = useRef<HTMLButtonElement>(null)
  const [menu, setMenu] = useState(false)
  const nameOf = nameIn(cx.tree, cx.proto?.frames)
  const parentPath = path.slice(0, Math.max(0, path.length - 2))
  const setAction = (next: Action) => void runAt(path, 'proto.action.set', { action: next })
  const dests = useDests(cx)
  const replaceWith = async (label: ActionLabel) => {
    const next = defaultAction(label, cx.proto, cx.tree, cx.node, action, cx.variables, await dests(label))
    if (next) setAction(next)
    else toast(t(unavailable(label)))
  }
  const summary = actionSummary(action, nameOf, cx.variables, t)
  const moreItems: MenuEntry[] = [
    ...(i > 0 ? [{ label: t('Move up'), onSelect: () => void move(i - 1) }] : []),
    ...(i + 1 < count ? [{ label: t('Move down'), onSelect: () => void move(i + 1) }] : []),
    ...(removable ? [...(count > 1 ? [{ type: 'separator' as const }] : []), { label: t('Remove'), onSelect: () => void remove(), testId: 'design-proto-action-remove' }] : []),
  ]
  async function move(to: number) {
    const answer = await runAt<{ path?: number[] }>(path, 'proto.action.move', { to })
    if (answer && open && answer.path) setOpenPath(target, answer.path)
  }
  async function remove() {
    if (await runAt(path, 'proto.action.remove')) setOpenPath(target, parentPath)
  }
  const kindDropdown = (
    <KindDropdown label={t(actionLabel(action))} current={actionLabel(action)} onPick={replaceWith} />
  )
  return (
    <div className="design-proto-action" data-testid={`design-proto-action-${path.join('-')}`}>
      <Row>
        {single ? null : (
          <button type="button" className="design-proto-chevron" aria-expanded={open} data-tip={open ? t('Close') : t('Open')} onClick={() => setOpenPath(target, open ? parentPath : path)} data-testid={`design-proto-action-toggle-${path.join('-')}`}>
            <Icon name={open ? 'Chevron' : 'ChevronRight'} size={12} />
          </button>
        )}
        <div className="design-proto-action-main">
          {open ? (
            kindDropdown
          ) : (
            <button type="button" className="design-proto-action-summary" data-tip={summary.length > 30 ? summary : undefined} onClick={() => setOpenPath(target, path)} data-testid={`design-proto-action-summary-${path.join('-')}`}>
              {summary}
            </button>
          )}
        </div>
        {single ? null : (
          <>
            <button ref={more} type="button" className="design-icon-button" aria-label={t('More')} data-tip={t('More')} onClick={() => setMenu(true)} disabled={!moreItems.length} data-testid={`design-proto-action-more-${path.join('-')}`}>
              <Icon name="Dots" size={16} />
            </button>
            {menu ? <Menu tone="light" anchor={more.current} placement="below-end" minWidth={180} items={moreItems} onClose={() => setMenu(false)} /> : null}
          </>
        )}
      </Row>
      {open ? (
        <div style={{ paddingLeft: single ? 0 : INDENT }}>
          {action.type === 'conditional' ? (
            <ConditionalBlocks cx={cx} path={path} blocks={action.blocks} />
          ) : action.type === 'setVariable' ? (
            <SetVariableFields cx={cx} path={path} action={action} setAction={setAction} />
          ) : (
            <ActionFields cx={cx} action={action} setAction={setAction} />
          )}
        </div>
      ) : null}
    </div>
  )
}

function KindDropdown({ label, current, onPick }: { label: string; current: ActionLabel; onPick: (label: ActionLabel) => void }) {
  const { t } = useDesignKit()
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  return (
    <>
      <button ref={button} type="button" className="design-dropdown" style={{ width: '100%' }} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((now) => !now)} data-testid="design-proto-action-kind">
        <span className="design-dropdown-label">{label}</span>
        <Icon name="Chevron" size={12} className="design-chevron" />
      </button>
      {open ? <Menu tone="light" anchor={button.current} placement="below-start" gap={4} minWidth={button.current?.offsetWidth} items={kindEntries(t, onPick, current)} onClose={() => setOpen(false)} testId="design-proto-action-kind-menu" /> : null}
    </>
  )
}

/**
 * A destination picker (`dest_dropdown`): a frame-led dropdown of what `proto.destinations`
 * answers for `kind` on the layer, the current one checked.
 */
function DestDropdown({ cx, label, kind, current, onPick, testId }: { cx: Cx; label: string; kind: DestKind; current: number | null; onPick: (id: number) => void; testId?: string }) {
  const { t } = useDesignKit()
  const answer = useQuery<DestAnswer>('proto.destinations', { id: cx.node, kind })
  const options = answer?.destinations ?? []
  const nameOf = nameIn(cx.tree, cx.proto?.frames)
  const text = current == null ? t('None') : (options.find((d) => d.id === current)?.name ?? nameOf(current) ?? t('Missing layer'))
  return (
    <>
      <SubLabels labels={[label]} />
      <Row>
        <RichDropdown
          label={text}
          icon={<Icon name="Frame" size={14} />}
          width="100%"
          maxHeight={320}
          ariaLabel={label}
          testId={testId ?? 'design-proto-destination'}
          tip={text.length > 26 ? text : undefined}
          items={() =>
            options.map((d) => ({
              key: String(d.id),
              testId: `design-proto-destination-${d.id}`,
              onSelect: () => onPick(d.id),
              render: () => <span className="design-menu-label">{`${d.id === current ? '✓ ' : ''}${d.name}`}</span>,
            }))
          }
        />
      </Row>
    </>
  )
}

/** The settings of an action (`action_fields`). */
function ActionFields({ cx, action, setAction }: { cx: Cx; action: Action; setAction: (a: Action) => void }) {
  const { t } = useDesignKit()
  switch (action.type) {
    case 'navigate':
      return (
        <>
          <DestDropdown cx={cx} label={t('Destination')} kind="navigate" current={action.dest} onPick={(dest) => setAction({ ...action, dest })} />
          <Row height={28}>
            <Checkbox checked={action.preserve_scroll === true} label={t('Preserve scroll position')} onChange={(on) => setAction({ ...action, preserve_scroll: on })} testId="design-proto-preserve-scroll" />
          </Row>
        </>
      )
    case 'swapOverlay':
      return <DestDropdown cx={cx} label={t('Overlay')} kind="overlay" current={action.dest} onPick={(dest) => setAction({ ...action, dest })} />
    case 'changeTo':
      return <DestDropdown cx={cx} label={t('Variant')} kind="changeTo" current={action.dest} onPick={(dest) => setAction({ ...action, dest })} />
    case 'scrollTo': {
      const offset = action.offset ?? [0, 0]
      const write = (i: number, v: number) => {
        const next: [number, number] = [offset[0], offset[1]]
        next[i] = Math.round(Math.min(100000, Math.max(-100000, v)))
        setAction({ ...action, offset: next })
      }
      return (
        <>
          <DestDropdown cx={cx} label={t('Scroll to layer')} kind="scrollTo" current={action.dest} onPick={(dest) => setAction({ ...action, dest })} />
          <SubLabels labels={[t('Offset')]} />
          <Row>
            <div className="design-proto-pair">
              <NumberField label="X" value={offset[0]} width="100%" decimals={0} onCommit={(edit) => write(0, numberOf(edit, offset[0]))} testId="design-proto-scroll-x" />
              <NumberField label="Y" value={offset[1]} width="100%" decimals={0} onCommit={(edit) => write(1, numberOf(edit, offset[1]))} testId="design-proto-scroll-y" />
            </div>
          </Row>
        </>
      )
    }
    case 'openUrl':
      return (
        <>
          <SubLabels labels={[t('Link')]} />
          <Row>
            <TextField value={action.url} placeholder="https://" width="100%" ariaLabel={t('Link')} onCommit={(url) => setAction({ type: 'openUrl', url: url.trim() })} testId="design-proto-url" />
          </Row>
        </>
      )
    case 'openOverlay':
      return <OverlayFields cx={cx} action={action} setAction={setAction} />
    case 'setVariableMode':
      return <ModeFields cx={cx} action={action} setAction={setAction} />
    default:
      return null
  }
}

function OverlayFields({ cx, action, setAction }: { cx: Cx; action: Extract<Action, { type: 'openOverlay' }>; setAction: (a: Action) => void }) {
  const { t } = useDesignKit()
  const position = action.position ?? 'center'
  const anchor = action.anchor ?? 'screen'
  const closeOnOutside = action.close_on_outside !== false
  const background = action.background ?? null
  const offset = action.offset ?? null
  const write = (i: number, v: number) => {
    const next: [number, number] = [offset?.[0] ?? 0, offset?.[1] ?? 0]
    next[i] = Math.round(Math.min(100000, Math.max(-100000, v)))
    setAction({ ...action, offset: next })
  }
  return (
    <>
      <DestDropdown cx={cx} label={t('Overlay')} kind="overlay" current={action.dest} onPick={(dest) => setAction({ ...action, dest })} />
      <SubLabels labels={[t('Overlay position')]} />
      <Row>
        <Dropdown value={position} width="100%" ariaLabel={t('Overlay position')} testId="design-proto-overlay-position" options={OVERLAY_POSITIONS.map(([value, label]) => ({ value, label: t(label) }))} onChange={(next) => setAction({ ...action, position: next })} />
      </Row>
      {position !== 'manual' ? (
        <>
          <SubLabels labels={[t('Relative to')]} />
          <Row>
            <Segmented text value={anchor === 'screen' ? 'screen' : 'trigger'} testId="design-proto-overlay-relative" segments={[{ value: 'screen', label: t('Whole screen') }, { value: 'trigger', label: t('Trigger layer') }]} onChange={(v) => setAction({ ...action, anchor: v === 'screen' ? 'screen' : 'triggerOutside' })} />
          </Row>
          {anchor !== 'screen' ? (
            <Row>
              <Segmented text value={anchor} testId="design-proto-overlay-inside" segments={[{ value: 'triggerInside', label: t('Inside') }, { value: 'triggerOutside', label: t('Outside') }]} onChange={(v) => setAction({ ...action, anchor: v })} />
            </Row>
          ) : null}
        </>
      ) : (
        <Row>
          <div className="design-proto-pair">
            <NumberField label="X" value={offset ? offset[0] : null} placeholder={t('Auto')} width="100%" decimals={0} onCommit={(edit) => write(0, numberOf(edit, offset?.[0] ?? 0))} testId="design-proto-overlay-x" />
            <NumberField label="Y" value={offset ? offset[1] : null} placeholder={t('Auto')} width="100%" decimals={0} onCommit={(edit) => write(1, numberOf(edit, offset?.[1] ?? 0))} testId="design-proto-overlay-y" />
          </div>
        </Row>
      )}
      <Row height={28}>
        <Checkbox checked={closeOnOutside} label={t('Close when clicking outside')} onChange={(on) => setAction({ ...action, close_on_outside: on })} testId="design-proto-overlay-close-outside" />
      </Row>
      <Row height={28}>
        <Checkbox checked={background != null} label={t('Add background behind overlay')} onChange={(on) => setAction({ ...action, background: on ? { r: 0, g: 0, b: 0, a: 0.25 } : null })} testId="design-proto-overlay-background" />
      </Row>
      {background ? (
        <Row>
          <span className="design-proto-swatch" style={{ background: `rgba(${Math.round(background.r * 255)}, ${Math.round(background.g * 255)}, ${Math.round(background.b * 255)}, ${background.a})` }} />
          <TextField
            value={colorHex(background)}
            width={96}
            onCommit={(text) => {
              const css = cssColor(text)
              if (!css) return
              const n = parseInt(css.slice(1), 16)
              setAction({ ...action, background: { r: ((n >> 16) & 255) / 255, g: ((n >> 8) & 255) / 255, b: (n & 255) / 255, a: background.a } })
            }}
            testId="design-proto-overlay-background-hex"
          />
          <NumberField value={Math.round(background.a * 100)} suffix="%" width={64} decimals={0} min={0} max={100} onCommit={(edit) => setAction({ ...action, background: { ...background, a: Math.min(100, Math.max(0, numberOf(edit, background.a * 100))) / 100 } })} testId="design-proto-overlay-background-alpha" />
        </Row>
      ) : null}
    </>
  )
}

function ModeFields({ cx, action, setAction }: { cx: Cx; action: Extract<Action, { type: 'setVariableMode' }>; setAction: (a: Action) => void }) {
  const { t } = useDesignKit()
  const collections = cx.variables?.collections ?? []
  const current = collections.find((c) => c.id === action.collection)
  return (
    <>
      <SubLabels labels={[t('Collection')]} />
      <Row>
        <Dropdown
          value={current?.id ?? null}
          mixedLabel={t('Choose collection')}
          width="100%"
          testId="design-proto-collection"
          options={collections.map((c) => ({ value: c.id, label: c.name }))}
          onChange={(id) => {
            const c = collections.find((x) => x.id === id)
            if (c?.modes[0]) setAction({ type: 'setVariableMode', collection: c.id, mode: c.modes[0].id })
          }}
        />
      </Row>
      <SubLabels labels={[t('Mode')]} />
      <Row>
        <Dropdown value={current?.modes.some((m) => m.id === action.mode) ? action.mode : null} mixedLabel={t('Choose mode')} width="100%" testId="design-proto-mode" options={(current?.modes ?? []).map((m) => ({ value: m.id, label: m.name }))} onChange={(mode) => setAction({ ...action, mode })} />
      </Row>
    </>
  )
}

/** A variable dropdown (the shared variable picker's place; lead `Variable` icon). */
function VariableDropdown({ cx, value, label, onPick, testId }: { cx: Cx; value: string | null; label: string; onPick: (id: string) => void; testId?: string }) {
  const { t } = useDesignKit()
  const vars = cx.variables?.variables ?? []
  return (
    <RichDropdown
      label={label}
      icon={<Icon name="Variable" size={14} />}
      width="100%"
      maxHeight={320}
      testId={testId}
      tip={label}
      items={() =>
        vars.length
          ? vars.map((v) => ({ key: v.id, onSelect: () => onPick(v.id), testId: `${testId}-${v.id}`, render: () => <span className="design-menu-label">{`${v.id === value ? '✓ ' : '    '}${v.name}`}</span> }))
          : [{ key: 'none', onSelect: () => undefined, render: () => <span className="design-menu-label">{t('Create a variable first')}</span> }]
      }
    />
  )
}

/** A field for a literal of `kind` (`literal_field`): number, true / false, colour, text. */
function LiteralField({ kind, value, onCommit, testId }: { kind: string; value: unknown; onCommit: (v: unknown) => void; testId?: string }) {
  const { t } = useDesignKit()
  if (kind === 'number') {
    const n = typeof value === 'number' ? value : Number(value) || 0
    return <NumberField value={n} width="100%" onCommit={(edit) => onCommit(numberOf(edit, n))} testId={testId} />
  }
  if (kind === 'boolean') {
    const on = value === true || value === 'true'
    return <Dropdown value={on ? 'true' : 'false'} width="100%" testId={testId} options={[{ value: 'true', label: 'true' }, { value: 'false', label: 'false' }]} onChange={(v) => onCommit(v === 'true')} />
  }
  if (kind === 'color') {
    const hex = typeof value === 'string' ? value.replace('#', '').toUpperCase() : typeof value === 'object' && value ? colorHex(value as { r: number; g: number; b: number; a: number }) : '000000'
    return (
      <>
        <span className="design-proto-swatch" style={{ background: `#${hex}` }} />
        <TextField value={hex} width="100%" onCommit={(text) => {
          const css = cssColor(text)
          if (css) onCommit(css)
        }} testId={testId} />
      </>
    )
  }
  return <TextField value={typeof value === 'string' ? value : String(value ?? '')} placeholder={t('Text')} width="100%" onCommit={onCommit} testId={testId} />
}

/** An expression's problem: the engine's message and where in the text a parse error is. */
type ExprError = { error: string; at: number | null }

/** Checks `text` with `proto.parseExpression` (null when it holds; else why, and where). */
function useCheckExpression() {
  const { query } = useDesignKit()
  return async (text: string, expect: 'condition' | 'value', variable?: string): Promise<ExprError | null> => {
    try {
      const answer = await query<{ ok: boolean; error: string | null; at: number | null }>('proto.parseExpression', { text, expect, ...(variable ? { variable } : {}) })
      return answer.ok ? null : { error: answer.error ?? 'Invalid expression', at: answer.at ?? null }
    } catch (error) {
      return { error: message(error), at: null }
    }
  }
}

/**
 * An expression typed as text (`expr_field`): checked with the engine's `proto.parseExpression`
 * read before it is written; a failed one keeps its text and says why under the field, marking
 * where a parse error is.
 */
function ExprField({ shown, placeholder, onCommit, trailing, testId }: { shown: string; placeholder: string; onCommit: (text: string) => Promise<ExprError | null>; trailing?: ReactNode; testId?: string }) {
  const [draft, setDraft] = useState<{ over: string; text: string; error: ExprError } | null>(null)
  const live = draft && draft.over === shown ? draft : null
  const at = live?.error.at
  const chars = live ? Array.from(live.text) : []
  return (
    <>
      <Row>
        <div style={{ flex: 1, minWidth: 0 }}>
          <TextField
            value={live?.text ?? shown}
            placeholder={placeholder}
            width="100%"
            onCommit={(text) => {
              void onCommit(text).then((error) => setDraft(error ? { over: shown, text, error } : null))
            }}
            testId={testId}
          />
        </div>
        {trailing}
      </Row>
      {live ? (
        <div className="design-proto-expr-error" data-testid={testId ? `${testId}-error` : undefined} data-at={at ?? undefined}>
          {at != null ? (
            <code className="design-proto-expr-at">
              {chars.slice(0, at).join('')}
              <mark>{chars[at] ?? ' '}</mark>
              {chars.slice(at + 1).join('')}
            </code>
          ) : null}
          <span>{live.error.error}</span>
        </div>
      ) : null}
    </>
  )
}

const message = (error: unknown) => (error instanceof Error ? error.message : String(error)).replace(/^[a-z-]+: /, '')

/** A set variable action (`set_variable_fields`): the variable and its fixed value or expression. */
function SetVariableFields({ cx, path, action, setAction }: { cx: Cx; path: number[]; action: Extract<Action, { type: 'setVariable' }>; setAction: (a: Action) => void }) {
  const { t, run } = useDesignKit()
  const check = useCheckExpression()
  const vars = cx.variables
  const variable = vars?.variables.find((v) => v.id === action.variable)
  const pick = (id: string) => {
    const picked = vars?.variables.find((v) => v.id === id)
    if (!picked || !vars) return
    const same = variable?.resolvedType === picked.resolvedType
    setAction(same ? { ...action, variable: id } : { type: 'setVariable', variable: id, value: valueJson(startValue(vars, id)) })
  }
  const expression = action.expr
  const toggle = (
    <IconButton
      icon="Code"
      label={expression ? t('Use a fixed value') : t('Use an expression, e.g. count + 1')}
      pressed={expression != null}
      iconSize={16}
      onClick={() => {
        if (!variable || !vars) return
        if (expression) {
          const fixed = 'value' in expression && expression.value.type === variable.resolvedType ? valueJson(expression.value) : action.value ?? valueJson(startValue(vars, variable.id))
          setAction({ type: 'setVariable', variable: variable.id, value: fixed })
        } else setAction({ ...action, expr: { value: { type: 'alias', value: variable.id } } })
      }}
      testId="design-proto-expression-toggle"
    />
  )
  return (
    <>
      <SubLabels labels={[t('Variable')]} />
      <Row>
        <VariableDropdown cx={cx} value={action.variable} label={variable?.name ?? t('Choose variable')} onPick={pick} testId="design-proto-variable" />
      </Row>
      {variable ? (
        <>
          <SubLabels labels={[expression ? t('Expression') : t('Value')]} />
          {expression ? (
            <ExprField
              shown={exprDisplay(expression, vars)}
              placeholder={t('e.g. count + 1')}
              trailing={toggle}
              testId="design-proto-expression"
              onCommit={async (text) => {
                const problem = await check(text, 'value', variable.id)
                if (problem) return problem
                const { expr: _old, ...rest } = action
                const answer = await run('proto.action.set', { ids: cx.ids, index: cx.index, path, action: { ...rest, expression: text } })
                return answer === null ? { error: t('e.g. count + 1'), at: null } : null
              }}
            />
          ) : (
            <Row>
              <div className="design-proto-literal">
                <LiteralField kind={variable.resolvedType} value={action.value} onCommit={(value) => setAction({ ...action, value })} testId="design-proto-variable-value" />
              </div>
              {toggle}
            </Row>
          )}
        </>
      ) : null}
    </>
  )
}

const BINARY = ['equals', 'notEqual', 'less', 'lessOrEqual', 'greater', 'greaterOrEqual', 'and', 'or']

type Simple = { variable: string; op: string; rhs: { value: { type: string; value: unknown } } | { variable: string } | null }

/** The condition as the builder edits it (`simple`), or null when it needs the expression field. */
function simple(e: Expr): Simple | null {
  if (!('op' in e)) return null
  const { op, args } = e.op
  const lhs = args[0]
  if (!lhs || !('value' in lhs) || lhs.value.type !== 'alias') return null
  const variable = String(lhs.value.value)
  if (op === 'not' && args.length === 1) return { variable, op, rhs: null }
  const rhs = args[1]
  if (!BINARY.includes(op) || args.length !== 2 || !rhs || !('value' in rhs)) return null
  return { variable, op, rhs: rhs.value.type === 'alias' ? { variable: String(rhs.value.value) } : { value: rhs.value } }
}

function simpleExpr(s: Simple): Expr {
  const lhs: Expr = { value: { type: 'alias', value: s.variable } }
  if (!s.rhs) return { op: { op: 'not', args: [lhs] } }
  return { op: { op: s.op, args: [lhs, 'variable' in s.rhs ? { value: { type: 'alias', value: s.rhs.variable } } : { value: s.rhs.value }] } }
}

const opsFor = (kind: string | undefined) => (kind === 'number' ? ['equals', 'notEqual', 'less', 'lessOrEqual', 'greater', 'greaterOrEqual'] : kind === 'boolean' ? ['equals', 'notEqual', 'and', 'or', 'not'] : ['equals', 'notEqual'])

/** If / else if / else (`conditional_blocks`): each block's condition and actions, then "Else if" and "Else". */
function ConditionalBlocks({ cx, path, blocks }: { cx: Cx; path: number[]; blocks: Array<{ condition?: Expr; actions: Action[] }> }) {
  const { t } = useDesignKit()
  const runAt = useRunAt(cx)
  const [textMode, setTextMode] = useState<Record<number, boolean>>({})
  const hasElse = blocks.length > 0 && blocks[blocks.length - 1]!.condition == null
  return (
    <div className="design-proto-conditional" data-testid="design-proto-conditional">
      {blocks.map((block, b) => {
        const label = block.condition ? (b === 0 ? t('If') : t('Else if')) : t('Else')
        const builder = block.condition ? simple(block.condition) : null
        const asText = block.condition != null && (!builder || textMode[b] === true)
        const tip = !builder ? t('This condition is edited as an expression') : asText ? t('Edit with the builder') : t('Edit as an expression')
        return (
          <div key={b} className="design-proto-block" data-testid={`design-proto-block-${b}`}>
            <Row height={28}>
              <span className="design-proto-block-label">{label}</span>
              {block.condition ? <IconButton icon="Code" label={tip} pressed={asText} iconSize={16} onClick={() => builder && setTextMode((m) => ({ ...m, [b]: !asText }))} /> : null}
              <IconButton icon="Minus" label={block.condition ? t('Remove this condition and its actions') : t('Remove the else and its actions')} iconSize={16} onClick={() => void runAt(path, 'proto.conditional.removeBlock', { block: b })} testId={`design-proto-block-remove-${b}`} />
            </Row>
            {block.condition ? asText || !builder ? <ConditionText cx={cx} path={path} block={b} condition={block.condition} /> : <ConditionBuilder cx={cx} path={path} block={b} s={builder} /> : null}
            <div className="design-proto-block-actions">
              <ActionList node={cx.node} index={cx.index} ids={cx.ids} actions={block.actions} prefix={[...path, b]} proto={cx.proto} tree={cx.tree} variables={cx.variables} />
            </div>
          </div>
        )
      })}
      <Row height={28}>
        <LinkButton icon="Plus" label={t('Else if')} tip={t('Add another condition')} onClick={() => void runAt(path, 'proto.conditional.addBlock', {})} testId="design-proto-else-if" />
        {hasElse ? null : <LinkButton icon="Plus" label={t('Else')} tip={t('Add actions for when no condition holds')} onClick={() => void runAt(path, 'proto.conditional.addBlock', { else: true })} testId="design-proto-else" />}
      </Row>
    </div>
  )
}

function ConditionText({ cx, path, block, condition }: { cx: Cx; path: number[]; block: number; condition: Expr }) {
  const { t, run } = useDesignKit()
  const check = useCheckExpression()
  return (
    <ExprField
      shown={exprDisplay(condition, cx.variables)}
      placeholder={t('e.g. count >= 3 and not done')}
      testId={`design-proto-condition-${block}`}
      onCommit={async (text) => {
        const problem = await check(text, 'condition')
        if (problem) return problem
        const answer = await run('proto.conditional.setCondition', { ids: cx.ids, index: cx.index, path, block, expression: text })
        return answer === null ? { error: t('e.g. count >= 3 and not done'), at: null } : null
      }}
    />
  )
}

function ConditionBuilder({ cx, path, block, s }: { cx: Cx; path: number[]; block: number; s: Simple }) {
  const { t } = useDesignKit()
  const runAt = useRunAt(cx)
  const vars = cx.variables
  const variable = vars?.variables.find((v) => v.id === s.variable)
  const set = (next: Simple) => void runAt(path, 'proto.conditional.setCondition', { block, condition: simpleExpr(next) })
  const withOp = (op: string): Simple => {
    if (op === 'not') return { ...s, op, rhs: null }
    if (!s.rhs) return { ...s, op, rhs: { value: vars ? startValue(vars, s.variable) : { type: 'boolean', value: true } } }
    return { ...s, op }
  }
  return (
    <Row>
      <div className="design-proto-builder">
        <VariableDropdown
          cx={cx}
          value={s.variable}
          label={variable?.name ?? t('Missing variable')}
          testId={`design-proto-condition-variable-${block}`}
          onPick={(id) => {
            const picked = vars?.variables.find((v) => v.id === id)
            if (!picked || !vars) return
            set(picked.resolvedType === variable?.resolvedType ? { ...s, variable: id } : { variable: id, op: 'equals', rhs: { value: startValue(vars, id) } })
          }}
        />
        <Dropdown value={s.op} width={48} menuWidth={80} testId={`design-proto-condition-op-${block}`} options={opsFor(variable?.resolvedType).map((op) => ({ value: op, label: opSymbol(op) }))} onChange={(op) => set(withOp(op))} />
        {s.rhs == null ? <span /> : 'variable' in s.rhs ? (
          <span className="design-proto-bound" data-tip={vars?.variables.find((v) => v.id === (s.rhs as { variable: string }).variable)?.name}>
            {vars?.variables.find((v) => v.id === (s.rhs as { variable: string }).variable)?.name ?? t('Missing variable')}
          </span>
        ) : (
          <div className="design-proto-literal">
            <LiteralField
              kind={s.rhs.value.type}
              value={s.rhs.value.type === 'color' ? s.rhs.value.value : s.rhs.value.value}
              testId={`design-proto-condition-value-${block}`}
              onCommit={(v) => {
                const type = (s.rhs as { value: { type: string } }).value.type
                const value = type === 'color' && typeof v === 'string' ? hexToColor(v) : v
                set({ ...s, rhs: { value: { type, value } } })
              }}
            />
          </div>
        )}
      </div>
    </Row>
  )
}

function hexToColor(css: string) {
  const n = parseInt(css.replace('#', ''), 16)
  return { r: ((n >> 16) & 255) / 255, g: ((n >> 8) & 255) / 255, b: (n & 255) / 255, a: 1 }
}
