/**
 * Paints as the engine's JSON gives them (`query('selection')` nodes, `selectionColors`, styles)
 * read into one shape the panels work with, and written back as the `paint` that `fill.set`,
 * `stroke.set`, `selection.setPaint` and `text.style` take.
 */
import { BLACK, type Color, color as makeColor, cssColor, lerp, readColor, toCssHex, toHex, withAlpha } from './color'

export type GradientKind = 'linear' | 'radial' | 'angular' | 'diamond'
export type Stop = { position: number; color: Color }
export type Point = [number, number]
export type Gradient = { kind: GradientKind; stops: Stop[]; from: Point; to: Point; width: Point }

export type Paint = {
  type: 'solid' | 'gradient' | 'image' | 'pattern'
  opacity: number
  visible: boolean
  /** camelCase blend mode (`normal`, `multiply`, …). */
  blend: string
  /** A solid paint's colour (alpha 1 unless the document says otherwise). */
  color: Color
  gradient: Gradient | null
  /** An image paint's own keys (`image`, `scale_mode`, `scale`, `rotation`, `crop`, `filters`). */
  image: Record<string, unknown> | null
  /** A pattern paint's own keys (`source`, `tile`, `scale`, `spacing`, `align`). */
  pattern: Record<string, unknown> | null
}

const num = (v: unknown, fallback: number) => (typeof v === 'number' && Number.isFinite(v) ? v : fallback)
const point = (v: unknown, fallback: Point): Point => (Array.isArray(v) && v.length >= 2 ? [num(v[0], fallback[0]), num(v[1], fallback[1])] : fallback)

const COMMON = new Set(['type', 'opacity', 'visible', 'blend', 'colorAlpha', 'color'])

/** Reads one paint of the engine's JSON; null when it is not one. */
export function readPaint(raw: unknown): Paint | null {
  if (!raw || typeof raw !== 'object') return null
  const v = raw as Record<string, unknown>
  const type = v.type
  const base = { opacity: num(v.opacity, 1), visible: v.visible !== false, blend: typeof v.blend === 'string' ? v.blend : 'normal' }
  if (type === 'solid') {
    const c = readColor(v.color) ?? BLACK
    return { ...base, type: 'solid', color: c, gradient: null, image: null, pattern: null }
  }
  if (type === 'gradient') {
    const stops = Array.isArray(v.stops)
      ? v.stops.map((s) => ({ position: num((s as Record<string, unknown>)?.position, 0), color: readColor((s as Record<string, unknown>)?.color) ?? BLACK }))
      : []
    const kind = (['linear', 'radial', 'angular', 'diamond'].includes(String(v.kind)) ? v.kind : 'linear') as GradientKind
    const gradient: Gradient = { kind, stops, from: point(v.from, [0.5, 0]), to: point(v.to, [0.5, 1]), width: point(v.width, [0, 0]) }
    return { ...base, type: 'gradient', color: stops[0]?.color ?? BLACK, gradient, image: null, pattern: null }
  }
  if (type === 'image' || type === 'pattern') {
    const own: Record<string, unknown> = {}
    for (const [key, value] of Object.entries(v)) if (!COMMON.has(key)) own[key] = value
    return { ...base, type, color: BLACK, gradient: null, image: type === 'image' ? own : null, pattern: type === 'pattern' ? own : null }
  }
  return null
}

export function readPaints(raw: unknown): Paint[] {
  return Array.isArray(raw) ? raw.map(readPaint).filter((p): p is Paint => p !== null) : []
}

const colorJson = (c: Color) => ({ r: c.r, g: c.g, b: c.b, a: c.a })

/** The `paint` commands take. */
export function paintJson(p: Paint): Record<string, unknown> {
  const base = { opacity: p.opacity, visible: p.visible, blend: p.blend }
  switch (p.type) {
    case 'solid':
      return { type: 'solid', color: toCssHex(withAlpha(p.color, p.color.a)), ...base }
    case 'gradient': {
      const g = p.gradient ?? defaultGradient('linear', p.color)
      return { type: 'gradient', kind: g.kind, stops: g.stops.map((s) => ({ position: s.position, color: colorJson(s.color) })), from: g.from, to: g.to, width: g.width, ...base }
    }
    case 'image':
      return { type: 'image', ...(p.image ?? {}), ...base }
    case 'pattern':
      return { type: 'pattern', ...(p.pattern ?? {}), ...base }
  }
}

export function solid(c: Color, opacity = 1): Paint {
  return { type: 'solid', opacity, visible: true, blend: 'normal', color: withAlpha(c, 1), gradient: null, image: null, pattern: null }
}

/** `Gradient::default_for`: the colour at 100 % to the same at 0 %. */
export function defaultGradient(kind: GradientKind, base: Color): Gradient {
  const [from, to, width]: Point[] = kind === 'linear' ? [[0.5, 0], [0.5, 1], [-0.5, 0]] : [[0.5, 0.5], [0.5, 1], [1, 0.5]]
  return { kind, stops: [{ position: 0, color: withAlpha(base, 1) }, { position: 1, color: withAlpha(base, 0) }], from, to, width }
}

/** `Gradient::sample`: the colour at `t` along the gradient (sRGB interpolation). */
export function sample(g: Gradient, t: number): Color {
  const stops = [...g.stops].filter((s) => Number.isFinite(s.position)).sort((a, b) => a.position - b.position)
  if (!stops.length) return BLACK
  const x = Math.min(1, Math.max(0, t))
  let prev = stops[0]
  if (x <= prev.position) return prev.color
  for (const s of stops.slice(1)) {
    if (x <= s.position) {
      const span = s.position - prev.position
      return lerp(prev.color, s.color, span > 1e-6 ? (x - prev.position) / span : 1)
    }
    prev = s
  }
  return prev.color
}

/** `Paint::preview_color`: a solid's colour, a gradient's first stop, else black. */
export function previewColor(p: Paint): Color {
  if (p.type === 'solid') return p.color
  if (p.type === 'gradient' && p.gradient?.stops[0]) return p.gradient.stops[0].color
  return BLACK
}

/** The colour the picker edits: a solid's colour with the paint's opacity as alpha. */
export function editColor(p: Paint): Color {
  return withAlpha(p.color, p.color.a * p.opacity)
}

const KIND_LABEL: Record<GradientKind, string> = { linear: 'Linear', radial: 'Radial', angular: 'Angular', diamond: 'Diamond' }

/** `paint_label`: the colour (`FFFFFF`), the gradient kind's name, "Image" or "Pattern" (English; translate with `t`). */
export function paintLabel(p: Paint): string {
  switch (p.type) {
    case 'solid':
      return toHex(p.color)
    case 'gradient':
      return KIND_LABEL[p.gradient?.kind ?? 'linear']
    case 'image':
      return 'Image'
    case 'pattern':
      return 'Pattern'
  }
}

/** `paint_name` (selection colours' dropdowns): "Gradient", "Image", "Pattern" or the hex. */
export function paintName(p: Paint): string {
  if (p.type === 'gradient') return 'Gradient'
  if (p.type === 'solid') return toHex(p.color)
  return paintLabel(p)
}

/** The CSS gradient a swatch or bar shows for a gradient, sampled in `n` strips as upstream draws it. */
export function stripes(g: Gradient, n: number, direction = 'to right'): string {
  const parts: string[] = []
  for (let i = 0; i < n; i += 1) {
    const c = cssColor(sample(g, n === 1 ? 0 : i / (n - 1)))
    parts.push(`${c} ${(i / n) * 100}% ${((i + 1) / n) * 100}%`)
  }
  return `linear-gradient(${direction}, ${parts.join(', ')})`
}

/** The swatch's colour stripes in the gradient bar: `n` strips sampled at their middles. */
export function barStripes(g: Gradient, n: number): string {
  const parts: string[] = []
  for (let i = 0; i < n; i += 1) {
    const c = cssColor(sample(g, (i + 0.5) / n))
    parts.push(`${c} ${(i / n) * 100}% ${((i + 1) / n) * 100}%`)
  }
  return `linear-gradient(to right, ${parts.join(', ')})`
}

export const BLEND_MODES = [
  ['passThrough', 'Pass through'],
  ['normal', 'Normal'],
  ['darken', 'Darken'],
  ['multiply', 'Multiply'],
  ['plusDarker', 'Plus darker'],
  ['colorBurn', 'Color burn'],
  ['lighten', 'Lighten'],
  ['screen', 'Screen'],
  ['plusLighter', 'Plus lighter'],
  ['colorDodge', 'Color dodge'],
  ['overlay', 'Overlay'],
  ['softLight', 'Soft light'],
  ['hardLight', 'Hard light'],
  ['difference', 'Difference'],
  ['exclusion', 'Exclusion'],
  ['hue', 'Hue'],
  ['saturation', 'Saturation'],
  ['color', 'Color'],
  ['luminosity', 'Luminosity'],
] as const

/** Blend modes after which `blend_menu` draws a separator. */
export const BLEND_BREAKS = new Set(['normal', 'colorBurn', 'colorDodge', 'hardLight', 'exclusion'])

export function blendLabel(id: string): string {
  return BLEND_MODES.find(([key]) => key === id)?.[1] ?? 'Normal'
}

/** Index after a list move (`moved_index`): where item `index` is once `from` moved to `to`. */
export function movedIndex(index: number, from: number, to: number): number {
  if (index === from) return to
  if (from < index && index <= to) return index - 1
  if (to <= index && index < from) return index + 1
  return index
}

/** Index after removing `removed` (`removed_index`): null when it was the one removed. */
export function removedIndex(index: number, removed: number): number | null {
  if (index === removed) return null
  return index > removed ? index - 1 : index
}

export { makeColor }
