/**
 * The engine's easing curves and transition placements, so the player can composite a
 * transition between two pictures in the browser (engine `kition_design_doc::proto::Easing::eval`
 * and the wasm player's `smart::transition_frames`).
 */

/** An easing as the engine writes it: `"easeOut"`, or `{ customBezier: {…} }` / `{ customSpring: {…} }`. */
export type EasingJson = string | { customBezier?: { x1: number; y1: number; x2: number; y2: number }; customSpring?: { mass: number; stiffness: number; damping: number } } | null | undefined

const SPRINGS: Record<string, [number, number, number]> = {
  gentle: [1, 100, 15],
  quick: [1, 300, 20],
  bouncy: [1, 600, 15],
  slow: [1, 80, 20],
}

const fin = (v: number, d: number) => (Number.isFinite(v) ? v : d)

/** CSS `cubic-bezier(x1, y1, x2, y2)` at time `t`. */
function cubicBezier(t: number, x1: number, y1: number, x2: number, y2: number) {
  const ax = Math.min(1, Math.max(0, fin(x1, 0.42)))
  const bx = Math.min(1, Math.max(0, fin(x2, 0.58)))
  const ay = Math.min(10, Math.max(-10, fin(y1, 0)))
  const by = Math.min(10, Math.max(-10, fin(y2, 1)))
  const b = (p1: number, p2: number, s: number) => {
    const u = 1 - s
    return 3 * u * u * s * p1 + 3 * u * s * s * p2 + s * s * s
  }
  let lo = 0
  let hi = 1
  for (let i = 0; i < 48; i += 1) {
    const mid = (lo + hi) / 2
    if (b(ax, bx, mid) < t) lo = mid
    else hi = mid
  }
  return b(ay, by, (lo + hi) / 2)
}

/** A damped spring released from 0 towards 1, at `t` (0..1) of its settling time. */
function spring(mass: number, stiffness: number, damping: number, t: number) {
  if (t >= 1) return 1
  const ok = (v: number, lo: number, d: number) => (Number.isFinite(v) && v >= lo ? v : d)
  const m = Math.min(ok(mass, 0.01, 1), 1e4)
  const k = Math.min(ok(stiffness, 0.01, 100), 1e6)
  const c = Math.min(ok(damping, 0, 15), 1e6)
  const w0 = Math.sqrt(k / m)
  const zeta = c / (2 * Math.sqrt(k * m))
  const [rate, kSettle] = zeta < 1 ? [zeta * w0, 6.9] : [w0 / (zeta + Math.sqrt(zeta * zeta - 1)), 9.2]
  let settle = rate > 1e-6 ? Math.min(10, Math.max(0.05, kSettle / rate)) : 10
  if (!Number.isFinite(settle)) settle = 10
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

/** The easing curve at linear progress `t` (0..1). */
export function ease(easing: EasingJson, raw: number): number {
  const t = Number.isFinite(raw) ? Math.min(1, Math.max(0, raw)) : 1
  const back = 1.70158
  if (easing && typeof easing === 'object') {
    if (easing.customBezier) {
      const { x1, y1, x2, y2 } = easing.customBezier
      return cubicBezier(t, x1, y1, x2, y2)
    }
    if (easing.customSpring) {
      const { mass, stiffness, damping } = easing.customSpring
      return spring(mass, stiffness, damping, t)
    }
    return t
  }
  switch (easing ?? 'easeOut') {
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
    default: {
      const s = SPRINGS[easing as string]
      return s ? spring(s[0], s[1], s[2], t) : t
    }
  }
}

/** Where the two screens sit during a transition (fractions of the screen) and how opaque they are. */
export type Placement = { fromOff: [number, number]; fromAlpha: number; toOff: [number, number]; toAlpha: number; fromOnTop: boolean }

const DIRECTIONS: Record<string, [number, number]> = { left: [-1, 0], right: [1, 0], top: [0, -1], bottom: [0, 1] }

/** The engine's `transition_frames` at eased progress `t`. */
export function placement(transition: { type: string; direction?: string }, t: number): Placement {
  const base: Placement = { fromOff: [0, 0], fromAlpha: 1, toOff: [0, 0], toAlpha: 1, fromOnTop: false }
  const d = DIRECTIONS[transition.direction ?? 'left'] ?? DIRECTIONS.left
  const mul = (k: number): [number, number] => [d[0] * k, d[1] * k]
  switch (transition.type) {
    case 'instant':
      return { ...base, fromAlpha: 0 }
    case 'dissolve':
    case 'smartAnimate':
      return { ...base, toAlpha: Math.min(1, Math.max(0, t)) }
    case 'moveIn':
      return { ...base, toOff: mul(-(1 - t)) }
    case 'moveOut':
      return { ...base, fromOff: mul(t), fromOnTop: true }
    case 'push':
      return { ...base, fromOff: mul(t), toOff: mul(-(1 - t)) }
    case 'slideIn':
      return { ...base, fromOff: mul(0.3 * t), toOff: mul(-(1 - t)) }
    case 'slideOut':
      return { ...base, fromOff: mul(t), toOff: mul(-0.3 * (1 - t)), fromOnTop: true }
    default:
      return { ...base, toAlpha: 1, fromAlpha: 0 }
  }
}
