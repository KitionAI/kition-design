/** A paint as the engine serializes it (`kition_design_color::Paint`), drawn as a CSS background for swatches. */

type Color = { r: number; g: number; b: number; a?: number }
type Paint = { type?: string; color?: Color; opacity?: number; stops?: Array<{ position: number; color: Color }> }

const channel = (v: number) => Math.round(Math.min(Math.max(v, 0), 1) * 255)

export function colorCss(c: Color | undefined, opacity = 1): string {
  if (!c) return 'transparent'
  const a = Math.min(Math.max((c.a ?? 1) * opacity, 0), 1)
  return `rgba(${channel(c.r)}, ${channel(c.g)}, ${channel(c.b)}, ${Math.round(a * 1000) / 1000})`
}

/** The swatch background for `paint` (a solid colour, a gradient's stops, or a neutral fill for images and patterns). */
export function paintCss(value: unknown): string {
  const paint = (value ?? {}) as Paint
  const opacity = typeof paint.opacity === 'number' ? paint.opacity : 1
  if (paint.type === 'solid') return colorCss(paint.color, opacity)
  if (Array.isArray(paint.stops) && paint.stops.length) {
    const stops = paint.stops.map((s) => `${colorCss(s.color, opacity)} ${Math.round(s.position * 100)}%`).join(', ')
    return `linear-gradient(90deg, ${stops})`
  }
  return 'var(--design-border-strong)'
}
