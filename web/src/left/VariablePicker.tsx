/**
 * The variable picker (2.2, `variable_ui.rs` `picker`): the dark popover listing the variables
 * that fit, by collection, with a search, as the engine filters them (`query('variables.pickFor')`:
 * the type, the field's scope on every layer, a style field's scopes, no alias loops). Picking
 * binds a layer field, makes a value an alias, binds a style field, or hands the variable to whoever
 * asked. Enabled libraries' variables follow under "{library} · {collection}" (to bind or alias):
 * picking one copies it in (`library.bindVariable`, `library.aliasVariable`). Open it with
 * `openVariablePicker` (the `variablePicker.open` UI request).
 */
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { dispatchUiRequests } from '../kit/requests'
import { Icon } from '../icons/Icon'
import { pressedOutside } from '../ui/Popover'
import { colorCss } from './swatch'
import { display, resolve, useVariables, type Variable, type VariableKind, type Variables } from './variables'

export type VariablePickTarget =
  | { bind: { ids: number[]; field: string } }
  | { alias: { variable: string; mode: string } }
  | { style: { style: string; field: string } }
  /** Anyone else (a prototype action): gets the variable's id. */
  | { pick: (id: string) => void }

export type VariablePickerRequest = {
  target: VariablePickTarget
  /** The type that fits (null: any; an alias takes its variable's type). */
  kind?: VariableKind | null
  /** Where it opens (window px): its top-left, 4 px lower. */
  x: number
  y: number
}

/** Opens the variable picker. */
export function openVariablePicker(request: VariablePickerRequest) {
  dispatchUiRequests([{ kind: 'variablePicker.open', data: request }])
}

const NONE: Record<string, [string, string]> = {
  color: ['No colour variables fit here.', 'No colour variables yet. Create them in Local variables.'],
  number: ['No number variables fit here.', 'No number variables yet. Create them in Local variables.'],
  string: ['No string variables fit here.', 'No string variables yet. Create them in Local variables.'],
  boolean: ['No boolean variables fit here.', 'No boolean variables yet. Create them in Local variables.'],
  any: ['No variables fit here.', 'No variables yet. Create them in Local variables.'],
}

/** One variable of `variables.pickFor` (bindings/wasm/src/lists.rs `pick_for`). */
type Pickable = { id: string; name: string; kind: VariableKind; swatch: string | null; display: string }
type PickFor = { scope: string | string[] | null; groups: Array<{ collection: string; name: string; library: null | { id: string; name: string }; variables: Pickable[] }> }

/** `variables.pickFor`'s params for a picker request. */
function pickParams(target: VariablePickTarget, kind: VariableKind | null | undefined, query: string): Record<string, unknown> {
  const base: Record<string, unknown> = { ...(kind ? { kind } : {}), ...(query ? { query } : {}) }
  if ('bind' in target) return { ...base, target: 'bind', ids: target.bind.ids, field: target.bind.field }
  if ('alias' in target) return { ...base, target: 'alias', variable: target.alias.variable }
  if ('style' in target) return { ...base, target: 'style', style: target.style.style, field: target.style.field }
  return { ...base, target: 'proto' }
}

/** A picker row: the swatch (colours) or the variable icon, the name, and other types' value. */
export function VariableRow({ vars, variable, onPick, testId }: { vars: Variables; variable: Variable; onPick: () => void; testId?: string }) {
  const { t } = useDesignKit()
  const value = resolve(vars, variable.id)
  return (
    <button type="button" className="design-var-row" onClick={onPick} data-testid={testId}>
      {value?.type === 'color' ? <span className="design-left-swatch" style={{ background: colorCss(value.value) }} /> : <Icon name="Variable" size={12} />}
      <span className="design-var-row-name">{variable.name}</span>
      {value && value.type !== 'color' ? <span className="design-var-row-value">{display(value, t)}</span> : null}
    </button>
  )
}

export function VariablePicker({ request, onClose }: { request: VariablePickerRequest; onClose: () => void }) {
  const { t, run } = useDesignKit()
  const vars = useVariables()
  const [query, setQuery] = useState('')
  const panel = useRef<HTMLDivElement>(null)
  const [at, setAt] = useState({ left: request.x, top: request.y + 4 })
  useLayoutEffect(() => {
    const el = panel.current
    if (!el) return
    const left = Math.max(8, Math.min(request.x, window.innerWidth - el.offsetWidth - 8))
    const top = Math.max(8, Math.min(request.y + 4, window.innerHeight - el.offsetHeight - 8))
    setAt((now) => (now.left === left && now.top === top ? now : { left, top }))
  })
  useEffect(() => {
    const down = (event: PointerEvent) => {
      if (panel.current && pressedOutside(panel.current, event.target, null)) onClose()
    }
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      onClose()
    }
    document.addEventListener('pointerdown', down, true)
    window.addEventListener('keydown', key, true)
    return () => {
      document.removeEventListener('pointerdown', down, true)
      window.removeEventListener('keydown', key, true)
    }
  }, [onClose])

  const target = request.target
  const q = query.trim()
  const answer = useQuery<PickFor>('variables.pickFor', pickParams(target, request.kind, q))
  // The last list stays while the next search's answer is on its way.
  const last = useRef<PickFor | null>(null)
  if (answer) last.current = answer
  const groups = (answer ?? last.current)?.groups ?? []

  const pick = (id: string, library: string | null) => {
    onClose()
    if (library && 'bind' in target) void run('library.bindVariable', { library, variable: id, ids: target.bind.ids, field: target.bind.field })
    else if (library && 'alias' in target) void run('library.aliasVariable', { library, alias: id, variable: target.alias.variable, mode: target.alias.mode })
    else if (library) return
    else if ('bind' in target) void run('variable.bind', { ids: target.bind.ids, field: target.bind.field, variable: id })
    else if ('alias' in target) void run('variable.set', { variable: target.alias.variable, mode: target.alias.mode, value: { alias: id } })
    else if ('style' in target) void run('style.bind', { style: target.style.style, field: target.style.field, variable: id })
    else target.pick(id)
  }
  const none = NONE[request.kind ?? 'any'] ?? NONE.any
  const anyOfKind = vars.variables.some((v) => request.kind == null || v.resolvedType === request.kind)
  const loaded = answer != null || last.current != null

  return (
    <div ref={panel} className="design-dark-popover design-var-picker design-layer" style={at} role="dialog" aria-label={t('Variables')} data-testid="design-variable-picker">
      <div className="design-dark-popover-title">{t('Variables')}</div>
      <input className="design-dark-search" value={query} placeholder={t('Search')} aria-label={t('Search')} autoFocus spellCheck={false} onChange={(event) => setQuery(event.target.value)} data-testid="design-variable-picker-search" />
      <div className="design-dark-popover-scroll" style={{ maxHeight: 280 }}>
        {groups.map((group) => (
          <div key={`${group.library?.id ?? ''}/${group.collection}`} data-testid={`design-variable-picker-group-${group.collection}`} data-library={group.library?.id}>
            <div className="design-dark-caption">{group.library ? `${group.library.name} · ${group.name}` : group.name}</div>
            {group.variables.map((v) => (
              <button key={v.id} type="button" className="design-var-row" onClick={() => pick(v.id, group.library?.id ?? null)} data-testid={`design-variable-pick-${v.id}`}>
                {v.swatch ? <span className="design-left-swatch" style={{ background: v.swatch }} /> : <Icon name="Variable" size={12} />}
                <span className="design-var-row-name">{v.name}</span>
                {v.display ? <span className="design-var-row-value">{v.kind === 'boolean' ? t(v.display === 'true' ? 'True' : v.display === 'false' ? 'False' : v.display) : v.display}</span> : null}
              </button>
            ))}
          </div>
        ))}
      </div>
      {loaded && groups.length === 0 ? <div className="design-dark-caption" style={{ fontSize: 11 }}>{t(anyOfKind ? none[0] : none[1])}</div> : null}
    </div>
  )
}
