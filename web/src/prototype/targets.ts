/**
 * Which layers an interaction edit applies to (upstream `targets_for`), and the default action of
 * each kind (`action_of`): what "Add interaction", "Add action" and the action kind menu create.
 */
import type { LayerInfo, ProtoAnswer, Tree } from './data'
import { subtree, topLevelFrame } from './data'
import type { Action, ActionLabel, Expr, Interaction, Variables } from './model'
import { actionDest } from './model'

export const sameInteractions = (lists: Array<Interaction[] | null>) => {
  const first = JSON.stringify(lists[0] ?? [])
  return lists.every((list) => JSON.stringify(list ?? []) === first)
}

/** The whole selection when it includes `node` and every selected layer has the same interactions, else just `node`. */
export function targetsFor(proto: ProtoAnswer | null, node: number): number[] {
  const entries = proto?.interactions ?? []
  if (entries.length > 1 && entries.some((e) => e.id === node) && sameInteractions(entries.map((e) => e.interactions))) return entries.map((e) => e.id)
  return [node]
}

/** The component around `node` and the variants of its set, from the layer tree (`proto.destinations` answers instances too). */
export function variantsFor(tree: Tree | null, node: number): { options: number[]; mine: number | null } {
  let at: number | null = node
  for (let i = 0; at != null && i < 256; i += 1) {
    const info: LayerInfo | undefined = tree?.nodes.get(at)
    if (info?.type === 'COMPONENT') {
      const parent = info.parent != null ? tree?.nodes.get(info.parent) : undefined
      return { options: parent?.type === 'COMPONENT_SET' ? [...parent.children].reverse() : [at], mine: at }
    }
    at = info?.parent ?? null
  }
  return { options: [], mine: null }
}

/** A variable's value in its collection's default mode, as a set variable action stores it. */
export function startValue(vars: Variables, id: string): { type: string; value: unknown } {
  const v = vars.variables.find((x) => x.id === id)
  const kind = v?.resolvedType ?? 'number'
  const mode = vars.collections.find((c) => c.id === v?.collection)?.modes[0]?.id
  const value = mode ? v?.values[mode] : undefined
  if (value && value.type !== 'alias') return value
  const placeholder: Record<string, unknown> = { number: 0, boolean: false, string: '', color: { r: 0, g: 0, b: 0, a: 1 } }
  return { type: kind, value: placeholder[kind] ?? 0 }
}

/** A stored value (`VariableValue`) as an action's `value`: colours as `#RRGGBB`. */
export function valueJson(value: { type: string; value: unknown }): unknown {
  if (value.type === 'color') {
    const c = value.value as { r: number; g: number; b: number }
    const h = (n: number) => Math.round(n * 255).toString(16).padStart(2, '0')
    return `#${h(c.r)}${h(c.g)}${h(c.b)}`
  }
  return value.value
}

/** The condition a new if starts with: the first boolean variable (else the first) equal to its value; `true` without variables. */
export function defaultCondition(vars: Variables | null): Expr {
  const v = vars?.variables.find((x) => x.resolvedType === 'boolean') ?? vars?.variables[0]
  if (!v || !vars) return { value: { type: 'boolean', value: true } }
  return { op: { op: 'equals', args: [{ value: { type: 'alias', value: v.id } }, { value: startValue(vars, v.id) }] } }
}

/** What `proto.destinations` answered for the kinds a default action needs (else the tree's guess). */
export type Dests = Partial<Record<'changeTo' | 'scrollTo', number[]>>

/** A default action of kind `label` (null when it needs something the document lacks). */
export function defaultAction(label: ActionLabel, proto: ProtoAnswer | null, tree: Tree | null, node: number, current: Action | null, vars: Variables | null, dests: Dests = {}): Action | null {
  const own = topLevelFrame(tree, node)
  const frames = proto?.frames.map((f) => f.id) ?? []
  const curDest = current ? actionDest(current) : null
  const frameDest = (curDest != null && frames.includes(curDest) ? curDest : null) ?? frames.find((f) => f !== own) ?? frames[0] ?? null
  switch (label) {
    case 'Navigate to':
      return frameDest == null ? null : { type: 'navigate', dest: frameDest, ...(current?.type === 'navigate' && current.preserve_scroll ? { preserve_scroll: true } : {}) }
    case 'Change to': {
      const { options: guessed, mine } = variantsFor(tree, node)
      const options = dests.changeTo ?? guessed
      const dest = options.find((v) => v !== mine) ?? options[0]
      return dest == null ? null : { type: 'changeTo', dest }
    }
    case 'Back':
      return { type: 'back' }
    case 'Scroll to': {
      const list = dests.scrollTo ?? (own == null ? [] : subtree(tree, own))
      const dest = list.find((id) => id !== node)
      return dest == null ? null : { type: 'scrollTo', dest }
    }
    case 'Open link':
      return { type: 'openUrl', url: '' }
    case 'Open overlay':
      return frameDest == null ? null : { type: 'openOverlay', dest: frameDest, position: 'center', close_on_outside: true, background: null, anchor: 'screen' }
    case 'Swap overlay':
      return frameDest == null ? null : { type: 'swapOverlay', dest: frameDest }
    case 'Close overlay':
      return { type: 'closeOverlay' }
    case 'Set variable': {
      const v = vars?.variables[0]
      return v && vars ? { type: 'setVariable', variable: v.id, value: valueJson(startValue(vars, v.id)) } : null
    }
    case 'Set variable mode': {
      const c = vars?.collections.find((x) => x.modes.length > 0)
      const m = c?.modes[1] ?? c?.modes[0]
      return c && m ? { type: 'setVariableMode', collection: c.id, mode: m.id } : null
    }
    case 'Conditional':
      return { type: 'conditional', blocks: [{ condition: defaultCondition(vars), actions: [] }] }
    default:
      return null
  }
}

/** Why an action of kind `label` can't be made here. */
export function unavailable(label: ActionLabel): string {
  switch (label) {
    case 'Change to':
      return 'Change to needs a layer inside a variant'
    case 'Set variable':
      return 'Create a variable first'
    case 'Set variable mode':
      return 'Create a variable collection first'
    default:
      return "There's no destination for that action"
  }
}
