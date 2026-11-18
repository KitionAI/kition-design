/**
 * The prototype model as the engine stores it (`kition_design_doc::proto`, serialised by the `node`
 * query's `interactions`), the upstream panel's label tables and summaries
 * (`panels/proto_panel.rs`), the device catalogue (`PrototypeSettings::DEVICES`), and the easing
 * curves (`Easing::eval`, `Easing::seconds`).
 */

export type Trigger =
  | { type: 'onClick' | 'onHover' | 'onPress' | 'onDrag' | 'mouseEnter' | 'mouseLeave' | 'mouseDown' | 'mouseUp' }
  | { type: 'afterDelay'; ms: number }
  | { type: 'keyPress'; key: string }

export type Direction = 'left' | 'right' | 'top' | 'bottom'
export type TransitionType = 'instant' | 'dissolve' | 'smartAnimate' | 'moveIn' | 'moveOut' | 'push' | 'slideIn' | 'slideOut'
export type Transition = { type: TransitionType; direction?: Direction }

export type Easing =
  | 'linear'
  | 'easeIn'
  | 'easeOut'
  | 'easeInOut'
  | 'easeInBack'
  | 'easeOutBack'
  | 'easeInOutBack'
  | 'gentle'
  | 'quick'
  | 'bouncy'
  | 'slow'
  | { customBezier: { x1: number; y1: number; x2: number; y2: number } }
  | { customSpring: { mass: number; stiffness: number; damping: number } }

export type Color = { r: number; g: number; b: number; a: number }
/** A prototype expression (`Expr`): a literal or variable reference, an operator, a mode. */
export type Expr =
  | { value: { type: string; value: unknown } }
  | { op: { op: string; args: Expr[] } }
  | { mode: { collection: string } }
  | { inMode: { variable: string; mode: string } }

export type OverlayPosition = 'center' | 'topLeft' | 'topCenter' | 'topRight' | 'bottomLeft' | 'bottomCenter' | 'bottomRight' | 'manual'
export type OverlayAnchor = 'screen' | 'triggerInside' | 'triggerOutside'

export type Action =
  | { type: 'navigate'; dest: number; preserve_scroll?: boolean }
  | { type: 'back' }
  | { type: 'scrollTo'; dest: number; offset?: [number, number] }
  | { type: 'openOverlay'; dest: number; position?: OverlayPosition; close_on_outside?: boolean; background?: Color | null; offset?: [number, number]; anchor?: OverlayAnchor }
  | { type: 'swapOverlay'; dest: number }
  | { type: 'closeOverlay' }
  | { type: 'openUrl'; url: string }
  | { type: 'changeTo'; dest: number }
  | { type: 'setVariable'; variable: string; value: unknown; expr?: Expr }
  | { type: 'setVariableMode'; collection: string; mode: string }
  | { type: 'conditional'; blocks: Array<{ condition?: Expr; actions: Action[] }> }

export type Interaction = {
  trigger: Trigger
  actions: Action[]
  transition: Transition
  duration_ms: number
  easing: Easing
  delay_ms?: number
}

export type Flow = { start: number; name: string; description?: string }

export type PrototypeSettings = {
  device: string
  background: Color
  rotation: 'portrait' | 'landscape'
  color?: string
  custom_size?: [number, number]
  responsive?: boolean
}

/** `query('variables')` (`variable.list`). */
export type Variables = {
  collections: Array<{ id: string; name: string; modes: Array<{ id: string; name: string }> }>
  variables: Array<{ id: string; name: string; collection: string; resolvedType: string; values: Record<string, { type: string; value: unknown }> }>
}

// Labels (the upstream English, translated through the catalogs by `t`).

export const TRIGGERS = ['On click', 'On drag', 'While hovering', 'While pressing', 'Key/Gamepad', 'Mouse enter', 'Mouse leave', 'Mouse down', 'Mouse up', 'After delay'] as const
export type TriggerLabel = (typeof TRIGGERS)[number]

export function triggerKind(t: Trigger): TriggerLabel {
  switch (t.type) {
    case 'onDrag':
      return 'On drag'
    case 'onHover':
      return 'While hovering'
    case 'onPress':
      return 'While pressing'
    case 'keyPress':
      return 'Key/Gamepad'
    case 'mouseEnter':
      return 'Mouse enter'
    case 'mouseLeave':
      return 'Mouse leave'
    case 'mouseDown':
      return 'Mouse down'
    case 'mouseUp':
      return 'Mouse up'
    case 'afterDelay':
      return 'After delay'
    default:
      return 'On click'
  }
}

export function triggerOf(kind: TriggerLabel, current: Trigger): Trigger {
  switch (kind) {
    case 'On drag':
      return { type: 'onDrag' }
    case 'While hovering':
      return { type: 'onHover' }
    case 'While pressing':
      return { type: 'onPress' }
    case 'Key/Gamepad':
      return { type: 'keyPress', key: current.type === 'keyPress' ? current.key : '' }
    case 'Mouse enter':
      return { type: 'mouseEnter' }
    case 'Mouse leave':
      return { type: 'mouseLeave' }
    case 'Mouse down':
      return { type: 'mouseDown' }
    case 'Mouse up':
      return { type: 'mouseUp' }
    case 'After delay':
      return { type: 'afterDelay', ms: current.type === 'afterDelay' ? current.ms : 800 }
    default:
      return { type: 'onClick' }
  }
}

export const GAMEPAD_BUTTONS = ['Gamepad A', 'Gamepad B', 'Gamepad X', 'Gamepad Y', 'Gamepad LB', 'Gamepad RB', 'Gamepad LT', 'Gamepad RT', 'Gamepad View', 'Gamepad Menu', 'Gamepad LS', 'Gamepad RS', 'Gamepad Up', 'Gamepad Down', 'Gamepad Left', 'Gamepad Right', 'Gamepad Home']

export const isGamepadButton = (key: string) => GAMEPAD_BUTTONS.some((b) => b.toLowerCase() === key.toLowerCase())

type T = (key: string, values?: Record<string, string | number>) => string

/** The trigger as the list shows it ("Key/Gamepad Shift+K", "After delay 800ms"). */
export function triggerSummary(trigger: Trigger, t: T): string {
  if (trigger.type === 'keyPress') {
    if (!trigger.key) return t('Key/Gamepad')
    if (isGamepadButton(trigger.key)) return trigger.key
    return t('Key/Gamepad {key}', { key: trigger.key })
  }
  if (trigger.type === 'afterDelay') return t('After delay {ms}ms', { ms: trigger.ms })
  return t(triggerKind(trigger))
}

export const TRANSITIONS: Array<[TransitionType, string]> = [
  ['instant', 'Instant'],
  ['dissolve', 'Dissolve'],
  ['smartAnimate', 'Smart animate'],
  ['moveIn', 'Move in'],
  ['moveOut', 'Move out'],
  ['push', 'Push'],
  ['slideIn', 'Slide in'],
  ['slideOut', 'Slide out'],
]

export const hasDirection = (type: TransitionType) => type === 'moveIn' || type === 'moveOut' || type === 'push' || type === 'slideIn' || type === 'slideOut'

export function transitionOf(type: TransitionType, current: Transition): Transition {
  return hasDirection(type) ? { type, direction: current.direction ?? 'left' } : { type }
}

export const transitionLabel = (t: Transition) => TRANSITIONS.find(([type]) => type === t.type)?.[1] ?? 'Instant'

export const EASINGS: Array<[Exclude<Easing, object>, string]> = [
  ['linear', 'Linear'],
  ['easeIn', 'Ease in'],
  ['easeOut', 'Ease out'],
  ['easeInOut', 'Ease in and out'],
  ['easeInBack', 'Ease in back'],
  ['easeOutBack', 'Ease out back'],
  ['easeInOutBack', 'Ease in and out back'],
  ['gentle', 'Gentle'],
  ['quick', 'Quick'],
  ['bouncy', 'Bouncy'],
  ['slow', 'Slow'],
]
export const CUSTOM_BEZIER: Easing = { customBezier: { x1: 0.42, y1: 0, x2: 0.58, y2: 1 } }
export const CUSTOM_SPRING: Easing = { customSpring: { mass: 1, stiffness: 100, damping: 15 } }

export function easingLabel(e: Easing): string {
  if (typeof e === 'object') return 'customBezier' in e ? 'Custom bezier' : 'Custom spring'
  return EASINGS.find(([id]) => id === e)?.[1] ?? 'Ease out'
}

export const easingKind = (e: Easing) => (typeof e === 'object' ? ('customBezier' in e ? 'customBezier' : 'customSpring') : e)

export const ACTIONS = ['Navigate to', 'Change to', 'Back', 'Scroll to', 'Open link', 'Open overlay', 'Swap overlay', 'Close overlay', 'Set variable', 'Set variable mode', 'Conditional'] as const
export type ActionLabel = (typeof ACTIONS)[number]
/** A separator goes before these indexes of `ACTIONS`. */
export const ACTION_GROUPS = [2, 4, 5, 8, 10]

export function actionLabel(a: Action): ActionLabel {
  switch (a.type) {
    case 'navigate':
      return 'Navigate to'
    case 'back':
      return 'Back'
    case 'scrollTo':
      return 'Scroll to'
    case 'openOverlay':
      return 'Open overlay'
    case 'swapOverlay':
      return 'Swap overlay'
    case 'closeOverlay':
      return 'Close overlay'
    case 'openUrl':
      return 'Open link'
    case 'changeTo':
      return 'Change to'
    case 'setVariable':
      return 'Set variable'
    case 'setVariableMode':
      return 'Set variable mode'
    default:
      return 'Conditional'
  }
}

export const actionDest = (a: Action): number | null => ('dest' in a && typeof a.dest === 'number' ? a.dest : null)

/** Every action, nested ones (inside conditionals) included. */
export function walkActions(actions: readonly Action[], depth = 0): Action[] {
  const out: Action[] = []
  for (const a of actions) {
    out.push(a)
    if (a.type === 'conditional' && depth < 16) for (const b of a.blocks) out.push(...walkActions(b.actions, depth + 1))
  }
  return out
}

/** Whether the action plays an animation (the interaction's transition applies to it). */
export const animates = (a: Action) => !['openUrl', 'setVariable', 'setVariableMode', 'closeOverlay', 'conditional'].includes(a.type)

export const OVERLAY_POSITIONS: Array<[OverlayPosition, string]> = [
  ['center', 'Center'],
  ['topLeft', 'Top left'],
  ['topCenter', 'Top center'],
  ['topRight', 'Top right'],
  ['bottomLeft', 'Bottom left'],
  ['bottomCenter', 'Bottom center'],
  ['bottomRight', 'Bottom right'],
  ['manual', 'Manual'],
]

// Expressions (`Expr::display`).

const SYMBOLS: Record<string, string> = {
  add: '+',
  subtract: '-',
  multiply: '*',
  divide: '/',
  equals: '==',
  notEqual: '!=',
  less: '<',
  lessOrEqual: '<=',
  greater: '>',
  greaterOrEqual: '>=',
  and: 'and',
  or: 'or',
  not: 'not',
  negate: '-',
}
export const opSymbol = (op: string) => SYMBOLS[op] ?? op

const hex2 = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, '0').toUpperCase()
export const colorHex = (c: Color) => `${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}`
export const colorCss = (c: Color) => `rgba(${Math.round(c.r * 255)}, ${Math.round(c.g * 255)}, ${Math.round(c.b * 255)}, ${c.a})`

function literal(v: { type: string; value: unknown }, vars: Variables | null): string {
  switch (v.type) {
    case 'alias':
      return vars?.variables.find((x) => x.id === v.value)?.name ?? String(v.value)
    case 'string':
      return JSON.stringify(v.value)
    case 'color':
      return `#${colorHex(v.value as Color)}`
    default:
      return String(v.value)
  }
}

/** An expression as written (`count + 1`, `done == true`). */
export function exprDisplay(e: Expr, vars: Variables | null, depth = 0): string {
  if (depth > 32) return '…'
  if ('value' in e) return literal(e.value, vars)
  if ('mode' in e) return `mode(${vars?.collections.find((c) => c.id === e.mode.collection)?.name ?? e.mode.collection})`
  if ('inMode' in e) {
    const v = vars?.variables.find((x) => x.id === e.inMode.variable)
    const c = vars?.collections.find((x) => x.id === v?.collection)
    return `${v?.name ?? e.inMode.variable}[${c?.modes.find((m) => m.id === e.inMode.mode)?.name ?? e.inMode.mode}]`
  }
  const { op, args } = e.op
  const sym = opSymbol(op)
  const inner = (a: Expr) => {
    const s = exprDisplay(a, vars, depth + 1)
    return 'op' in a && a.op.args.length > 1 ? `(${s})` : s
  }
  if (args.length === 1) return op === 'not' ? `not ${inner(args[0]!)}` : `${sym}${inner(args[0]!)}`
  return args.map(inner).join(` ${sym} `)
}

/** One-line action summary ("Navigate to Home — Dark"). */
export function actionSummary(a: Action, nameOf: (id: number) => string | null, vars: Variables | null, t: T): string {
  switch (a.type) {
    case 'openUrl':
      return a.url ? t('Open link {url}', { url: a.url }) : t('Open link')
    case 'setVariable': {
      const name = vars?.variables.find((v) => v.id === a.variable)?.name ?? a.variable
      return a.expr ? t('Set {name} to {value}', { name, value: exprDisplay(a.expr, vars) }) : t('Set variable {name}', { name })
    }
    case 'setVariableMode': {
      const c = vars?.collections.find((x) => x.id === a.collection)
      const mode = c?.modes.find((m) => m.id === a.mode)?.name ?? a.mode
      return t('Set {name} to {value}', { name: c?.name ?? a.collection, value: mode })
    }
    case 'conditional': {
      const c = a.blocks[0]?.condition
      return c ? t('If {condition}', { condition: exprDisplay(c, vars) }) : t('Conditional')
    }
    default: {
      const dest = actionDest(a)
      return dest == null ? t(actionLabel(a)) : `${t(actionLabel(a))} ${nameOf(dest) ?? t('Missing layer')}`
    }
  }
}

// Easing curves (`Easing::eval`, `Easing::seconds`).

function spring(e: Easing): [number, number, number] | null {
  if (typeof e === 'object') return 'customSpring' in e ? [e.customSpring.mass, e.customSpring.stiffness, e.customSpring.damping] : null
  return ({ gentle: [1, 100, 15], quick: [1, 300, 20], bouncy: [1, 600, 15], slow: [1, 80, 20] } as Record<string, [number, number, number]>)[e] ?? null
}

export const isSpring = (e: Easing) => spring(e) !== null

function makeSpring(mass: number, stiffness: number, damping: number) {
  const ok = (v: number, lo: number, d: number) => (Number.isFinite(v) && v >= lo ? v : d)
  const m = Math.min(ok(mass, 0.01, 1), 1e4)
  const k = Math.min(ok(stiffness, 0.01, 100), 1e6)
  const c = Math.min(ok(damping, 0, 15), 1e6)
  const w0 = Math.sqrt(k / m)
  const zeta = c / (2 * Math.sqrt(k * m))
  const [rate, kSettle] = zeta < 1 ? [zeta * w0, 6.9] : [w0 / (zeta + Math.sqrt(zeta * zeta - 1)), 9.2]
  let settle = rate > 1e-6 ? Math.min(10, Math.max(0.05, kSettle / rate)) : 10
  if (!Number.isFinite(settle)) settle = 10
  const at = (t: number) => {
    if (t >= 1) return 1
    const tau = t * settle
    let x: number
    if (zeta < 1) {
      const wd = w0 * Math.sqrt(1 - zeta * zeta)
      x = 1 - Math.exp(-zeta * w0 * tau) * (Math.cos(wd * tau) + ((zeta * w0) / wd) * Math.sin(wd * tau))
    } else if (Math.abs(zeta - 1) < 1e-9) {
      x = 1 - Math.exp(-w0 * tau) * (1 + w0 * tau)
    } else {
      const s = Math.sqrt(zeta * zeta - 1)
      const r1 = -w0 / (zeta + s)
      const r2 = -w0 * (zeta + s)
      x = 1 - (r2 * Math.exp(r1 * tau) - r1 * Math.exp(r2 * tau)) / (r2 - r1)
    }
    return Number.isFinite(x) ? x : 1
  }
  return { settle, at }
}

function cubicBezier(t: number, x1: number, y1: number, x2: number, y2: number) {
  const fin = (v: number, d: number) => (Number.isFinite(v) ? v : d)
  const cx1 = Math.min(1, Math.max(0, fin(x1, 0.42)))
  const cx2 = Math.min(1, Math.max(0, fin(x2, 0.58)))
  const cy1 = Math.min(10, Math.max(-10, fin(y1, 0)))
  const cy2 = Math.min(10, Math.max(-10, fin(y2, 1)))
  const b = (p1: number, p2: number, s: number) => {
    const u = 1 - s
    return 3 * u * u * s * p1 + 3 * u * s * s * p2 + s * s * s
  }
  let lo = 0
  let hi = 1
  for (let i = 0; i < 48; i += 1) {
    const mid = (lo + hi) / 2
    if (b(cx1, cx2, mid) < t) lo = mid
    else hi = mid
  }
  return b(cy1, cy2, (lo + hi) / 2)
}

/** The curve at `t` (0..1). */
export function easingEval(e: Easing, t0: number): number {
  const t = Number.isFinite(t0) ? Math.min(1, Math.max(0, t0)) : 1
  const back = 1.70158
  if (typeof e === 'object' && 'customBezier' in e) return cubicBezier(t, e.customBezier.x1, e.customBezier.y1, e.customBezier.x2, e.customBezier.y2)
  const s = spring(e)
  if (s) return makeSpring(...s).at(t)
  switch (e) {
    case 'linear':
      return t
    case 'easeIn':
      return t * t * t
    case 'easeOut':
      return 1 - (1 - t) ** 3
    case 'easeInOut':
      return t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2
    case 'easeInBack':
      return (back + 1) * t * t * t - back * t * t
    case 'easeOutBack':
      return 1 + (back + 1) * (t - 1) ** 3 + back * (t - 1) ** 2
    case 'easeInOutBack': {
      const c2 = back * 1.525
      return t < 0.5 ? ((2 * t) ** 2 * ((c2 + 1) * 2 * t - c2)) / 2 : ((2 * t - 2) ** 2 * ((c2 + 1) * (t * 2 - 2) + c2) + 2) / 2
    }
    default:
      return t
  }
}

/** How long it plays (seconds) at duration `ms`: a spring runs until it settles. */
export function easingSeconds(e: Easing, ms: number): number {
  const s = spring(e)
  return s ? makeSpring(...s).settle : ms / 1000
}

/** The CSS timing function closest to `e` (for the player's transitions). */
export function easingCss(e: Easing): string {
  if (typeof e === 'object' && 'customBezier' in e) {
    const { x1, y1, x2, y2 } = e.customBezier
    return `cubic-bezier(${x1}, ${y1}, ${x2}, ${y2})`
  }
  if (isSpring(e)) {
    const points = Array.from({ length: 33 }, (_, i) => easingEval(e, i / 32).toFixed(4))
    return `linear(${points.join(', ')})`
  }
  return (
    {
      linear: 'linear',
      easeIn: 'cubic-bezier(0.32, 0, 0.67, 0)',
      easeOut: 'cubic-bezier(0.33, 1, 0.68, 1)',
      easeInOut: 'cubic-bezier(0.65, 0, 0.35, 1)',
      easeInBack: 'cubic-bezier(0.36, 0, 0.66, -0.56)',
      easeOutBack: 'cubic-bezier(0.34, 1.56, 0.64, 1)',
      easeInOutBack: 'cubic-bezier(0.68, -0.6, 0.32, 1.6)',
    } as Record<string, string>
  )[e as string] ?? 'ease-out'
}

// Devices (`PrototypeSettings::DEVICES`).

export type Device = { name: string; width: number; height: number; rotates: boolean; colors: Array<{ name: string; hex: string }> }

export const CUSTOM_DEVICE = 'Custom size'
export const CUSTOM_DEFAULT: [number, number] = [1280, 720]

export const DEVICES: Device[] = [
  { name: 'iPhone 17', width: 402, height: 874, rotates: true, colors: [{ name: 'Black', hex: '#38393D' }, { name: 'White', hex: '#E9E9E5' }] },
  { name: 'iPhone 17 Pro', width: 402, height: 874, rotates: true, colors: [{ name: 'Black', hex: '#38393D' }, { name: 'Silver', hex: '#D7D7D5' }] },
  { name: 'iPhone 17 Pro Max', width: 440, height: 956, rotates: true, colors: [{ name: 'Black', hex: '#38393D' }, { name: 'Silver', hex: '#D7D7D5' }] },
  { name: 'iPhone Air', width: 420, height: 912, rotates: true, colors: [{ name: 'Black', hex: '#38393D' }, { name: 'Silver', hex: '#D7D7D5' }] },
  { name: 'iPhone 16', width: 393, height: 852, rotates: true, colors: [{ name: 'Black', hex: '#3A3B3F' }, { name: 'White', hex: '#EEEDE8' }, { name: 'Pink', hex: '#ECBFCB' }, { name: 'Teal', hex: '#A3CFC6' }, { name: 'Blue', hex: '#7284D4' }] },
  { name: 'iPhone 16 Pro', width: 402, height: 874, rotates: true, colors: [{ name: 'Black', hex: '#3B3A38' }, { name: 'White', hex: '#ECEBE6' }] },
  { name: 'iPhone 16 Pro Max', width: 440, height: 956, rotates: true, colors: [{ name: 'Black', hex: '#3B3A38' }, { name: 'White', hex: '#ECEBE6' }, { name: 'Natural', hex: '#BFB9AF' }, { name: 'Sand', hex: '#C6AC8F' }] },
  { name: 'iPhone 16 Plus', width: 430, height: 932, rotates: true, colors: [{ name: 'Black', hex: '#3A3B3F' }, { name: 'White', hex: '#EEEDE8' }] },
  { name: 'iPhone 15 Pro', width: 393, height: 852, rotates: true, colors: [{ name: 'Black', hex: '#3B3A38' }, { name: 'Silver', hex: '#D7D7D5' }] },
  { name: 'iPhone 15 Pro Max', width: 430, height: 932, rotates: true, colors: [{ name: 'Black', hex: '#3B3A38' }, { name: 'Silver', hex: '#D7D7D5' }] },
  { name: 'iPhone 15 Plus', width: 430, height: 932, rotates: true, colors: [{ name: 'Black', hex: '#3A3B3F' }, { name: 'White', hex: '#EEEDE8' }] },
  { name: 'iPhone 14 Pro', width: 393, height: 852, rotates: true, colors: [{ name: 'Black', hex: '#3B3A38' }, { name: 'Silver', hex: '#D7D7D5' }] },
  { name: 'iPhone 14 Pro Max', width: 430, height: 932, rotates: true, colors: [{ name: 'Black', hex: '#3B3A38' }, { name: 'Silver', hex: '#D7D7D5' }] },
  { name: 'iPhone 14', width: 390, height: 844, rotates: true, colors: [{ name: 'Black', hex: '#3A3B3F' }, { name: 'White', hex: '#EEEDE8' }] },
  { name: 'Android Compact', width: 412, height: 917, rotates: true, colors: [{ name: 'Black', hex: '#2E2F33' }, { name: 'Silver', hex: '#D6D8DB' }, { name: 'Green', hex: '#A6C0A3' }] },
  { name: 'Android Medium', width: 700, height: 840, rotates: true, colors: [{ name: 'Black', hex: '#2E2F33' }, { name: 'Silver', hex: '#D6D8DB' }] },
  { name: 'iPad mini', width: 744, height: 1133, rotates: true, colors: [{ name: 'Grey', hex: '#6A6B6F' }, { name: 'Silver', hex: '#E0E1E3' }, { name: 'Blue', hex: '#9CB1C9' }, { name: 'Purple', hex: '#C6BCD7' }] },
  { name: 'MacBook Air', width: 1280, height: 832, rotates: false, colors: [{ name: 'Silver', hex: '#C9CACE' }, { name: 'Grey', hex: '#7B7C80' }, { name: 'Dark blue', hex: '#2E333D' }, { name: 'Cream', hex: '#E0D9CB' }] },
  { name: 'Desktop', width: 1440, height: 1024, rotates: false, colors: [{ name: 'Silver', hex: '#B8B9BD' }, { name: 'Black', hex: '#2C2C2E' }] },
]

/** The chosen device model, upright (null for "None" or an unknown name). */
export function deviceModel(s: PrototypeSettings | null): Device | null {
  if (!s) return null
  if (s.device === CUSTOM_DEVICE) {
    const [w, h] = s.custom_size ?? CUSTOM_DEFAULT
    return { name: CUSTOM_DEVICE, width: w, height: h, rotates: false, colors: [] }
  }
  return DEVICES.find((d) => d.name === s.device) ?? null
}

export const isLandscape = (s: PrototypeSettings | null) => s?.rotation === 'landscape' && deviceModel(s)?.rotates === true

/** The device as presented: wider than tall when turned. */
export function presentedDevice(s: PrototypeSettings | null): Device | null {
  const d = deviceModel(s)
  if (!d) return null
  return isLandscape(s) ? { ...d, width: d.height, height: d.width } : d
}

export function deviceColor(s: PrototypeSettings | null) {
  const colors = deviceModel(s)?.colors ?? []
  return colors.find((c) => c.name === s?.color) ?? colors[0] ?? null
}

/** The key combination of a key press (`Shift+K`), named as the engine names keys. */
export function keyCombo(event: { key: string; code: string; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }): string | null {
  const name = keyName(event.code, event.key)
  if (!name) return null
  const mods = [event.ctrlKey && 'Ctrl', event.altKey && 'Alt', event.shiftKey && 'Shift', event.metaKey && 'Cmd'].filter(Boolean)
  return [...mods, name].join('+')
}

const CODE_NAMES: Record<string, string> = {
  Space: 'Space',
  Enter: 'Enter',
  NumpadEnter: 'Enter',
  Escape: 'Escape',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  Minus: 'Minus',
  Equal: 'Equals',
  Comma: 'Comma',
  Period: 'Period',
  Semicolon: 'Semicolon',
  Slash: 'Slash',
  Backslash: 'Backslash',
  Backquote: 'Backtick',
  BracketLeft: 'OpenBracket',
  BracketRight: 'CloseBracket',
  Quote: 'Quote',
}

function keyName(code: string, key: string): string | null {
  if (['Shift', 'Control', 'Alt', 'Meta'].includes(key)) return null
  if (/^Key[A-Z]$/.test(code)) return code.slice(3)
  if (/^Digit\d$/.test(code)) return code.slice(5)
  if (/^Numpad\d$/.test(code)) return code.slice(6)
  if (/^F\d{1,2}$/.test(code)) return code
  return CODE_NAMES[code] ?? (key.length === 1 ? key.toUpperCase() : key)
}
