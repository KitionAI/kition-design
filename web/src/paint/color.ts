/**
 * Colours as the engine keeps them (non-premultiplied sRGB, components 0..1) and the colour
 * models the picker shows: hex, RGB, HSL, HSB, CIE LCH (D50) and OKLCH, ported from the engine's
 * colour crate (`crates/support/color`) so values round-trip as upstream's do.
 */

export type Color = { r: number; g: number; b: number; a: number }

export const BLACK: Color = { r: 0, g: 0, b: 0, a: 1 }
export const WHITE: Color = { r: 1, g: 1, b: 1, a: 1 }

const clamp01 = (v: number) => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0)

export function color(r: number, g: number, b: number, a = 1): Color {
  return { r: clamp01(r), g: clamp01(g), b: clamp01(b), a: clamp01(a) }
}

export function withAlpha(c: Color, a: number): Color {
  return { ...c, a: clamp01(a) }
}

export function rgba8(c: Color): [number, number, number, number] {
  return [c.r, c.g, c.b, c.a].map((v) => Math.round(clamp01(v) * 255)) as [number, number, number, number]
}

const hex2 = (v: number) => v.toString(16).padStart(2, '0').toUpperCase()

/** `RRGGBB` (no `#`, no alpha), as the panels show colours. */
export function toHex(c: Color): string {
  const [r, g, b] = rgba8(c)
  return `${hex2(r)}${hex2(g)}${hex2(b)}`
}

/** `#RRGGBB`, or `#RRGGBBAA` when not opaque (what commands take). */
export function toCssHex(c: Color): string {
  const [r, g, b, a] = rgba8(c)
  return a === 255 ? `#${hex2(r)}${hex2(g)}${hex2(b)}` : `#${hex2(r)}${hex2(g)}${hex2(b)}${hex2(a)}`
}

/** A CSS colour for drawing (`rgba(…)`). */
export function cssColor(c: Color, alpha = c.a): string {
  const [r, g, b] = rgba8(c)
  return `rgba(${r}, ${g}, ${b}, ${Math.round(clamp01(alpha) * 1000) / 1000})`
}

/** `color.to_css()`: hex when opaque, else `rgba(r, g, b, a)`. */
export function toCss(c: Color): string {
  const [r, g, b] = rgba8(c)
  if (c.a >= 0.999) return toCssHex(c)
  const a8 = (a: number) => Math.round(clamp01(a) * 255)
  const short = c.a.toFixed(2)
  const alpha = a8(Number(short)) === a8(c.a) ? short : c.a.toFixed(3)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

/** `Color::from_hex`: `#` optional; 1, 2, 3, 4, 6 or 8 digits (`F` → `FFFFFF`, `AB` → `ABABAB`, `ABC` → `AABBCC`). */
export function fromHex(text: string): Color | null {
  let s = text.trim()
  if (s.startsWith('#')) s = s.slice(1)
  if (!/^[0-9a-fA-F]+$/.test(s)) return null
  let full: string
  switch (s.length) {
    case 1:
      full = s.repeat(6)
      break
    case 2:
      full = s.repeat(3)
      break
    case 3:
      full = s.split('').map((ch) => ch + ch).join('')
      break
    case 4:
      full = s.split('').map((ch) => ch + ch).join('')
      break
    case 6:
    case 8:
      full = s
      break
    default:
      return null
  }
  const n = (i: number) => parseInt(full.slice(i, i + 2), 16) / 255
  return color(n(0), n(2), n(4), full.length === 8 ? n(6) : 1)
}

/** Any colour CSS understands (names, `rgb()`, `hsl()`, `lch()`, …), through the browser's parser. */
function fromBrowser(text: string): Color | null {
  if (typeof document === 'undefined') return null
  const probe = document.createElement('canvas').getContext('2d')
  if (!probe) return null
  probe.fillStyle = '#010203'
  probe.fillStyle = text
  const first = String(probe.fillStyle)
  probe.fillStyle = '#030201'
  probe.fillStyle = text
  // An invalid colour leaves the previous one.
  if (String(probe.fillStyle) !== first) return null
  const value = first.trim()
  if (value.startsWith('#')) return fromHex(value)
  const m = /^rgba?\(([^)]+)\)$/.exec(value)
  if (!m) return null
  const parts = m[1].split(/[\s,/]+/).filter(Boolean).map(Number)
  if (parts.length < 3 || parts.some((p) => !Number.isFinite(p))) return null
  return color(parts[0] / 255, parts[1] / 255, parts[2] / 255, parts[3] ?? 1)
}

/** `Color::from_hex_or_name`: hex, else a CSS colour name. */
export function fromHexOrName(text: string): Color | null {
  return fromHex(text) ?? (/^[a-z]+$/i.test(text.trim()) ? fromBrowser(text.trim()) : null)
}

/** `Color::parse_css`: hex, `rgb()`, `hsl()`, `lch()`, `oklch()`, names, `transparent`. */
export function parseCss(text: string): Color | null {
  const t = text.trim()
  if (t.startsWith('#')) return fromHex(t)
  return fromBrowser(t)
}

/** The colour the engine's JSON gives: `{r, g, b, a}`, `[r, g, b, a?]` or a CSS/hex string. */
export function readColor(value: unknown): Color | null {
  if (typeof value === 'string') return parseCss(value) ?? fromHex(value)
  if (Array.isArray(value) && value.length >= 3) return color(Number(value[0]), Number(value[1]), Number(value[2]), value.length > 3 ? Number(value[3]) : 1)
  if (value && typeof value === 'object') {
    const v = value as Record<string, unknown>
    if (typeof v.r === 'number') return color(Number(v.r), Number(v.g), Number(v.b), v.a === undefined ? 1 : Number(v.a))
  }
  return null
}

/** Whether two colours are the same 8-bit RGBA (the document stores 8 bits). */
export function sameColor(a: Color, b: Color): boolean {
  return rgba8(a).join() === rgba8(b).join()
}

export function lerp(a: Color, b: Color, t: number): Color {
  return { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: a.a + (b.a - a.a) * t }
}

// ---- HSB and HSL ----

export type Hsb = { h: number; s: number; b: number }

export function toHsb(c: Color): Hsb {
  const max = Math.max(c.r, c.g, c.b)
  const min = Math.min(c.r, c.g, c.b)
  const d = max - min
  let h = 0
  if (d > 1e-9) {
    if (max === c.r) h = 60 * (((c.g - c.b) / d) % 6)
    else if (max === c.g) h = 60 * ((c.b - c.r) / d + 2)
    else h = 60 * ((c.r - c.g) / d + 4)
  }
  if (h < 0) h += 360
  return { h, s: max <= 1e-9 ? 0 : d / max, b: max }
}

export function fromHsb({ h, s, b }: Hsb, a = 1): Color {
  const hh = ((((h % 360) + 360) % 360) / 60)
  const c = b * s
  const x = c * (1 - Math.abs((hh % 2) - 1))
  const m = b - c
  const [r, g, bl] = hh < 1 ? [c, x, 0] : hh < 2 ? [x, c, 0] : hh < 3 ? [0, c, x] : hh < 4 ? [0, x, c] : hh < 5 ? [x, 0, c] : [c, 0, x]
  return color(r + m, g + m, bl + m, a)
}

export type Hsl = { h: number; s: number; l: number }

export function toHsl(c: Color): Hsl {
  const { h } = toHsb(c)
  const max = Math.max(c.r, c.g, c.b)
  const min = Math.min(c.r, c.g, c.b)
  const l = (max + min) / 2
  const d = max - min
  const s = d <= 1e-9 ? 0 : d / (1 - Math.abs(2 * l - 1))
  return { h, s, l }
}

export function fromHsl({ h, s, l }: Hsl, a = 1): Color {
  const b = l + s * Math.min(l, 1 - l)
  const sv = b <= 1e-9 ? 0 : 2 * (1 - l / b)
  return fromHsb({ h, s: sv, b }, a)
}

// ---- CIE LCH (D50) and OKLCH, as the engine's `lch.rs` ----

type V3 = [number, number, number]
type M3 = [V3, V3, V3]

const mul = (m: M3, v: V3): V3 => [
  m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
  m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
  m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
]
const decode = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)
const encode = (v: number) => clamp01(v <= 0.0031308 ? v * 12.92 : 1.055 * Math.max(v, 0) ** (1 / 2.4) - 0.055)
const linear = (c: Color): V3 => [decode(c.r), decode(c.g), decode(c.b)]

const SRGB_TO_XYZ: M3 = [
  [0.41239079926595934, 0.357584339383878, 0.1804807884018343],
  [0.21263900587151027, 0.715168678767756, 0.07219231536073371],
  [0.01933081871559182, 0.11919477979462598, 0.9505321522496607],
]
const XYZ_TO_SRGB: M3 = [
  [3.2409699419045226, -1.537383177570094, -0.4986107602930034],
  [-0.9692436362808796, 1.8759675015077202, 0.04155505740717559],
  [0.05563007969699366, -0.20397695888897652, 1.0569715142428786],
]
const D65_TO_D50: M3 = [
  [1.0479298208405488, 0.022946793341019088, -0.05019222954313557],
  [0.029627815688159344, 0.990434484573249, -0.01707382502938514],
  [-0.009243058152591178, 0.015055144896577895, 0.7518742899580008],
]
const D50_TO_D65: M3 = [
  [0.9554734527042182, -0.023098536874261423, 0.0632593086610217],
  [-0.028369706963208136, 1.0099954580058226, 0.021041398966943008],
  [0.012314001688319899, -0.020507696433477912, 1.3303659366080753],
]
const D50_WHITE: V3 = [0.3457 / 0.3585, 1, (1 - 0.3457 - 0.3585) / 0.3585]
const KAPPA = 24389 / 27
const EPSILON = 216 / 24389
const SRGB_TO_LMS: M3 = [
  [0.4122214708, 0.5363325363, 0.0514459929],
  [0.2119034982, 0.6806995451, 0.1073969566],
  [0.0883024619, 0.2817188376, 0.6299787005],
]
const LMS_TO_OKLAB: M3 = [
  [0.2104542553, 0.793617785, -0.0040720468],
  [1.9779984951, -2.428592205, 0.4505937099],
  [0.0259040371, 0.7827717662, -0.808675766],
]
const OKLAB_TO_LMS: M3 = [
  [1, 0.3963377774, 0.2158037573],
  [1, -0.1055613458, -0.0638541728],
  [1, -0.0894841775, -1.291485548],
]
const LMS_TO_SRGB: M3 = [
  [4.0767416621, -3.3077115913, 0.2309699292],
  [-1.2684380046, 2.6097574011, -0.3413193965],
  [-0.0041960863, -0.7034186147, 1.707614701],
]

function labOf(c: Color): V3 {
  const xyz = mul(D65_TO_D50, mul(SRGB_TO_XYZ, linear(c)))
  const f = (v: number) => (v > EPSILON ? Math.cbrt(v) : (KAPPA * v + 16) / 116)
  const [fx, fy, fz] = [f(xyz[0] / D50_WHITE[0]), f(xyz[1] / D50_WHITE[1]), f(xyz[2] / D50_WHITE[2])]
  return [116 * fy - 16, 500 * (fx - fy), 200 * (fy - fz)]
}

function linearFromLab([l, a, b]: V3): V3 {
  const fy = (l + 16) / 116
  const fx = fy + a / 500
  const fz = fy - b / 200
  const inv = (f: number) => (f ** 3 > EPSILON ? f ** 3 : (116 * f - 16) / KAPPA)
  const y = l > KAPPA * EPSILON ? fy ** 3 : l / KAPPA
  return mul(XYZ_TO_SRGB, mul(D50_TO_D65, [inv(fx) * D50_WHITE[0], y * D50_WHITE[1], inv(fz) * D50_WHITE[2]]))
}

function oklabOf(c: Color): V3 {
  const lms = mul(SRGB_TO_LMS, linear(c))
  return mul(LMS_TO_OKLAB, [Math.cbrt(lms[0]), Math.cbrt(lms[1]), Math.cbrt(lms[2])])
}

function linearFromOklab(lab: V3): V3 {
  const lms = mul(OKLAB_TO_LMS, lab)
  return mul(LMS_TO_SRGB, [lms[0] ** 3, lms[1] ** 3, lms[2] ** 3])
}

function polar([l, a, b]: V3, grey: number): V3 {
  const c = Math.hypot(a, b)
  const h = c < grey ? 0 : (((Math.atan2(b, a) * 180) / Math.PI) % 360 + 360) % 360
  return [l, c, h]
}

const inGamut = (rgb: V3) => rgb.every((v) => v >= -1e-6 && v <= 1 + 1e-6)

function mapToSrgb(l: number, c: number, h: number, toLinear: (lab: V3) => V3): Color {
  const rad = (h * Math.PI) / 180
  const lab = (chroma: number): V3 => [l, chroma * Math.cos(rad), chroma * Math.sin(rad)]
  let rgb = toLinear(lab(c))
  if (!inGamut(rgb)) {
    let lo = 0
    let hi = c
    for (let i = 0; i < 24; i += 1) {
      const mid = (lo + hi) / 2
      if (inGamut(toLinear(lab(mid)))) lo = mid
      else hi = mid
    }
    rgb = toLinear(lab(lo))
  }
  return color(encode(rgb[0]), encode(rgb[1]), encode(rgb[2]), 1)
}

const fin = (v: number) => (Number.isFinite(v) ? v : 0)
const wrap = (h: number) => ((fin(h) % 360) + 360) % 360

/** CIE LCH: L 0..100, C 0..230, H degrees. */
export const toLch = (c: Color): V3 => polar(labOf(c), 0.02)
export const fromLch = ([l, c, h]: V3): Color => mapToSrgb(Math.min(100, Math.max(0, fin(l))), Math.min(230, Math.max(0, fin(c))), wrap(h), linearFromLab)
/** OKLCH: L 0..1, C 0..0.5, H degrees. */
export const toOklch = (c: Color): V3 => polar(oklabOf(c), 0.0002)
export const fromOklch = ([l, c, h]: V3): Color => mapToSrgb(Math.min(1, Math.max(0, fin(l))), Math.min(0.5, Math.max(0, fin(c))), wrap(h), linearFromOklab)
