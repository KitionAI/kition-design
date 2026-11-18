/**
 * The engine's overlays (bindings/wasm/src/overlays.rs) in the upstream editor's style (2.5 §1,
 * §2, §6, §9-§14; upstream `canvas/overlays.rs`, `measure_overlay.rs`): white handles with an
 * accent ring, the marquee's 10% fill and inside stroke, smart guide labels placed clear of each
 * other and of the size pill, and the arc and radius readouts. kit-editor draws them through
 * `useEngineTools`' `overlayStyle` and `drawOverlay`.
 *
 * The engine sends upstream's sizes, so its overlays read back as upstream's kinds:
 *
 * - anchor 8: a selection corner (8x8 square, radius 1); 7, 9, 11: a vector point or the pen's
 *   anchor (a circle); 4 filled: the Scale tool's pivot (a white disc with a dot);
 * - handle: a bezier handle's end or a corner radius handle (r 3.5), an ellipse's arc handle (r 4);
 * - line in the guide red: a smart guide (dashed 3/2), a pair of 8 px diagonals is a snap marker
 *   (1.5 px); a solid accent line with no points around is the auto layout insertion line (2 px);
 * - measure after a guide line: that line's distance label; otherwise an arc or radius readout.
 */
import { overlayCss, type EngineOverlay, type OverlayDrawContext, type OverlayDrawer, type OverlayRgba, type OverlayStylePatch } from '@kition/kit-editor'
import { FONT, readTheme, semibold } from './chrome'

/** The overlay greens (upstream `ACCENT` on light pages, `BRAND` on dark ones). */
export const ACCENT = '#00875f'
export const BRAND = '#02da96'
export const COMPONENT = '#9747ff'
export const GUIDE = '#f24822'

/** What the canvas tells the drawer between frames: the page's green and the size pill's box. */
const canvasState: { accent: string; sizePill: SizePill | null } = { accent: ACCENT, sizePill: null }

/** The size pill as chrome gives it: its anchor (centre top, less 9 px) in document pixels. */
type SizePill = { x: number; y: number; text: string }

type Box = { x: number; y: number; w: number; h: number }

function luminance(hex: string) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(hex)
  if (!m) return 1
  const channel = (v: string) => {
    const c = parseInt(v, 16) / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * channel(m[1]) + 0.7152 * channel(m[2]) + 0.0722 * channel(m[3])
}

function contrast(a: string, b: string) {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p)
  return (x + 0.05) / (y + 0.05)
}

/** The green that contrasts more with the page's background (upstream `set_canvas_page`). */
export function accentFor(background: string) {
  return /^#[0-9a-f]{6}$/i.test(background) && contrast(BRAND, background) > contrast(ACCENT, background) ? BRAND : ACCENT
}

/** Sets the page's green (from its background) for the overlays that carry no colour (the marquee). */
export function setCanvasPage(background: string) {
  canvasState.accent = accentFor(background)
}

/** The chrome's size pill, or none: guide labels keep clear of it. */
export function setSizePill(label: SizePill | null) {
  canvasState.sizePill = label
}

const hex = ([r, g, b]: OverlayRgba) => `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`

/** A fill under white text: the darker green stands in for the bright one (upstream `under_white`). */
export function underWhite(color: string) {
  return color.toLowerCase() === BRAND ? ACCENT : color
}

/** Kit-editor's style changes: what needs no reading of the overlay list. */
export const DESIGN_OVERLAY_STYLE: OverlayStylePatch = {
  accent: ACCENT,
  surface: '#ffffff',
  marquee: { fillOpacity: 0.1, dash: [], inside: true },
  path: { dash: [4, 3] },
  line: { dash: [3, 2] },
  anchor: { shape: 'square', radius: 1, size: 8, ringWidth: 1 },
  handle: { radius: 3.5, fill: '#ffffff', ringWidth: 1 },
  swatch: { size: 12, radius: 2, ringWidth: 2 },
}

/** The device pixels per CSS pixel. */
const ratio = () => (typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1)

/** A coordinate where a 1 px line falls on whole device pixels. */
function crisp(v: number) {
  const d = ratio()
  const half = d % 2 === 1 ? 0.5 : 0
  return (Math.round(v * d - half) + half) / d
}

/** A coordinate on a device pixel's edge (a filled box's side). */
const edge = (v: number) => Math.round(v * ratio()) / ratio()

/**
 * A disc of radius `r` and its ring of `width` outside it, as upstream's `circle_filled` and
 * `circle_stroke` draw them (the ring lies outside the radius).
 */
function circle(context: CanvasRenderingContext2D, x: number, y: number, r: number, fill: string | null, ring: string | null, width = 1) {
  if (fill) {
    context.beginPath()
    context.arc(x, y, r, 0, Math.PI * 2)
    context.fillStyle = fill
    context.fill()
  }
  if (ring) {
    context.beginPath()
    context.arc(x, y, r + width / 2, 0, Math.PI * 2)
    context.strokeStyle = ring
    context.lineWidth = width
    context.stroke()
  }
}

/** Whether the frame is vector edit mode's (its segments are 1.5 px or wider paths). */
function vectorEditing(overlays: readonly EngineOverlay[]) {
  return overlays.some((o) => o.kind === 'path' && o.width >= 1.5 && o.width <= 3 && !o.dashed)
}

/** White text on a pill of `fill` (upstream `pill` / `pink_label`): returns its box. */
function pill(context: CanvasRenderingContext2D, text: string, box: Box, fill: string, size: number) {
  context.fillStyle = fill
  context.beginPath()
  context.roundRect(Math.round(box.x), Math.round(box.y), box.w, box.h, 3)
  context.fill()
  context.font = `${semibold()} ${size}px ${FONT}`
  context.fillStyle = '#ffffff'
  context.textBaseline = 'middle'
  context.fillText(text, Math.round(box.x) + 4, Math.round(box.y) + box.h / 2 + 0.5)
}

function pillSize(context: CanvasRenderingContext2D, text: string, size: number) {
  context.font = `${semibold()} ${size}px ${FONT}`
  return { w: Math.ceil(context.measureText(text).width) + 8, h: Math.round(size * 1.2) + 4 }
}

const GAP = 4

/**
 * Label boxes for lines `(a, b, size)` on screen (upstream `place_labels`, verbatim): centred on
 * the line when it is long enough, else slid along it, then beside it (below a horizontal line,
 * right of a vertical one, then the other side, further out each time), clear of `placed`.
 */
export function placeLabel(a: { x: number; y: number }, b: { x: number; y: number }, size: { w: number; h: number }, placed: Box[]): Box {
  const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }
  const d = { x: b.x - a.x, y: b.y - a.y }
  const horizontal = Math.abs(d.x) >= Math.abs(d.y)
  const dir = horizontal ? { x: 1, y: 0 } : { x: 0, y: 1 }
  const out = horizontal ? { x: 0, y: 1 } : { x: 1, y: 0 }
  const [along, across] = horizontal ? [size.w, size.h] : [size.h, size.w]
  const room = (Math.hypot(d.x, d.y) - along) / 2 - GAP
  const centres: { x: number; y: number }[] = []
  if (room >= 0) {
    centres.push(mid)
    for (let n = 1; n * (along + GAP) <= room && centres.length < 32; n += 1) {
      const k = n * (along + GAP)
      centres.push({ x: mid.x + dir.x * k, y: mid.y + dir.y * k }, { x: mid.x - dir.x * k, y: mid.y - dir.y * k })
    }
  }
  for (let n = 0; n < 32; n += 1) {
    const off = across / 2 + GAP + n * (across + GAP)
    centres.push({ x: mid.x + out.x * off, y: mid.y + out.y * off }, { x: mid.x - out.x * off, y: mid.y - out.y * off })
  }
  const box = (c: { x: number; y: number }): Box => ({ x: c.x - size.w / 2, y: c.y - size.h / 2, w: size.w, h: size.h })
  const hits = (p: Box, r: Box) => p.x - GAP / 2 < r.x + r.w && r.x < p.x + p.w + GAP / 2 && p.y - GAP / 2 < r.y + r.h && r.y < p.y + p.h + GAP / 2
  const found = centres.map(box).find((r) => !placed.some((p) => hits(p, r)))
  if (found) return found
  const bottom = placed.reduce((m, p) => Math.max(m, p.y + p.h), mid.y)
  return { x: mid.x - size.w / 2, y: bottom + GAP, w: size.w, h: size.h }
}

const isGuide = (color: OverlayRgba) => hex(color) === GUIDE

/** The selection's colour in this frame: its handles' (green, or purple for components). */
function selectionColour(overlays: readonly EngineOverlay[]) {
  for (const o of overlays) if ((o.kind === 'anchor' || o.kind === 'handle') && !isGuide(o.color)) return hex(o.color)
  for (const o of overlays) if (o.kind === 'path' && !isGuide(o.color)) return hex(o.color)
  return canvasState.accent
}

/** Whether the frame shows pen or vector edit points (their handles' lines are 1 px). */
function hasPoints(overlays: readonly EngineOverlay[]) {
  return overlays.some((o) => o.kind === 'anchor' && o.size !== 8 && o.size !== 4)
}

/** Whether the selection outline in this frame is curved (an ellipse: arc handles are r 4). */
function curvedSelection(overlays: readonly EngineOverlay[]) {
  return overlays.some((o) => o.kind === 'path' && /[CQ]/.test(o.d) && !isGuide(o.color))
}

type Frame = { labels?: Box[] }

/** An SVG path of straight, axis-aligned lines only (`M`, `L`, `Z`), as runs of points; else null. */
export function polyline(d: string): { points: [number, number][]; closed: boolean }[] | null {
  const tokens = d.match(/[MLZmlz]|-?\d*\.?\d+(?:e-?\d+)?/g)
  if (!tokens || /[^MLZ\d\s,.\-e]/.test(d)) return null
  const runs: { points: [number, number][]; closed: boolean }[] = []
  for (let i = 0; i < tokens.length; ) {
    const t = tokens[i]
    if (t === 'Z') {
      if (!runs.length) return null
      runs[runs.length - 1].closed = true
      i += 1
    } else if (t === 'M' || t === 'L') {
      const x = Number(tokens[i + 1])
      const y = Number(tokens[i + 2])
      if (!Number.isFinite(x) || !Number.isFinite(y)) return null
      if (t === 'M') runs.push({ points: [[x, y]], closed: false })
      else {
        const run = runs[runs.length - 1]
        const last = run?.points[run.points.length - 1]
        if (!run || !last || (Math.abs(last[0] - x) > 1e-6 && Math.abs(last[1] - y) > 1e-6)) return null
        run.points.push([x, y])
      }
      i += 3
    } else return null
  }
  return runs.length ? runs : null
}

function guideLabel(overlay: Extract<EngineOverlay, { kind: 'measure' }>, at: OverlayDrawContext) {
  const { context, toScreen, overlays, index } = at
  const frame = at.frame as Frame
  if (!frame.labels) {
    frame.labels = []
    const size = canvasState.sizePill
    if (size) {
      // As chrome.ts draws it: centred under the selection, 9 px down.
      const at = toScreen(size.x, size.y)
      const box = pillSize(context, size.text, 11)
      frame.labels.push({ x: Math.round(at.x - box.w / 2), y: Math.round(at.y + 9), ...box })
    }
  }
  // The line it labels: the guide line before it whose midpoint it sits on.
  const prev = overlays[index - 1]
  const p = toScreen(overlay.x, overlay.y)
  let a = p
  let b = p
  if (prev?.kind === 'line' && isGuide(prev.color)) {
    a = toScreen(prev.ax, prev.ay)
    b = toScreen(prev.bx, prev.by)
  }
  const size = pillSize(context, overlay.text, 10)
  const box = placeLabel(a, b, size, frame.labels)
  frame.labels.push(box)
  pill(context, overlay.text, box, GUIDE, 10)
}

/** Draws the overlays upstream draws differently from kit-editor's style; the rest go to the style. */
export const drawDesignOverlay: OverlayDrawer = (overlay, at) => {
  const { context, toScreen, overlays, index } = at
  switch (overlay.kind) {
    case 'path': {
      // Straight outlines (boxes, selections) on whole pixels, as upstream's are; curves as given.
      const points = polyline(overlay.d)
      if (!points || overlay.dashed || overlay.width !== 1) return false
      context.strokeStyle = overlayCss(overlay.color)
      context.beginPath()
      for (const run of points) {
        run.points.forEach(([x, y], i) => {
          const q = toScreen(x, y)
          if (i === 0) context.moveTo(crisp(q.x), crisp(q.y))
          else context.lineTo(crisp(q.x), crisp(q.y))
        })
        if (run.closed) context.closePath()
      }
      context.stroke()
      return true
    }
    case 'marquee': {
      const accent = canvasState.accent
      const a = toScreen(overlay.x0, overlay.y0)
      const b = toScreen(overlay.x1, overlay.y1)
      const [x0, x1] = [Math.round(Math.min(a.x, b.x)), Math.round(Math.max(a.x, b.x))]
      const [y0, y1] = [Math.round(Math.min(a.y, b.y)), Math.round(Math.max(a.y, b.y))]
      context.globalAlpha = 0.1
      context.fillStyle = accent
      context.fillRect(x0, y0, x1 - x0, y1 - y0)
      context.globalAlpha = 1
      context.strokeStyle = accent
      context.strokeRect(x0 + 0.5, y0 + 0.5, Math.max(0, x1 - x0 - 1), Math.max(0, y1 - y0 - 1))
      return true
    }
    case 'line': {
      const ink = hex(overlay.color)
      if ((ink === '#1e1e1e' || ink === '#ffffff') && overlay.color[3] === 255) {
        // The text caret (or the IME underline): the theme's text colour, as upstream draws it
        // (the engine picks it by the page instead), blinking at 1 Hz unless characters are selected.
        const frame = at.frame as Frame & { text?: string }
        frame.text ??= readTheme().text
        const selected = overlays.some((o) => o.kind === 'highlight')
        const caret = Math.abs(overlay.ax - overlay.bx) < 1e-9
        if (caret && !selected && Math.floor(performance.now() / 500) % 2 === 1) return true
        const a = toScreen(overlay.ax, overlay.ay)
        const b = toScreen(overlay.bx, overlay.by)
        context.strokeStyle = frame.text
        context.beginPath()
        context.moveTo(caret ? crisp(a.x) : a.x, a.y)
        context.lineTo(caret ? crisp(b.x) : b.x, b.y)
        context.stroke()
        return true
      }
      const a = toScreen(overlay.ax, overlay.ay)
      const b = toScreen(overlay.bx, overlay.by)
      const color = overlayCss(overlay.color)
      const guide = isGuide(overlay.color)
      const dx = Math.abs(b.x - a.x)
      const dy = Math.abs(b.y - a.y)
      let width = 1
      // A snap marker: two 8 px diagonals.
      if (guide && Math.abs(dx - dy) < 0.5 && Math.abs(dx - 8) < 0.5) width = 1.5
      // The rotation origin's ticks: 5 px on an axis.
      else if (!guide && Math.abs(Math.hypot(dx, dy) - 5) < 0.5 && (dx < 0.01 || dy < 0.01)) width = 1.5
      // The auto layout insertion line.
      else if (!guide && !overlay.dashed && !hasPoints(overlays) && (dx < 0.01 || dy < 0.01)) width = 2
      context.strokeStyle = color
      context.lineWidth = width
      if (overlay.dashed) context.setLineDash(guide ? [3, 2] : [4, 3])
      context.beginPath()
      if (width === 1 && dx < 0.01) {
        context.moveTo(crisp(a.x), a.y)
        context.lineTo(crisp(b.x), b.y)
      } else if (width === 1 && dy < 0.01) {
        context.moveTo(a.x, crisp(a.y))
        context.lineTo(b.x, crisp(b.y))
      } else {
        context.moveTo(a.x, a.y)
        context.lineTo(b.x, b.y)
      }
      context.stroke()
      return true
    }
    case 'anchor': {
      const p = toScreen(overlay.x, overlay.y)
      const color = overlayCss(overlay.color)
      const alpha = overlay.color[3] / 255
      if (overlay.size === 8) {
        // A selection corner: 8x8, radius 1, white with a 1 px inside ring, upright, centred on
        // the outline's pixel line.
        const x = edge(crisp(p.x) - 4)
        const y = edge(crisp(p.y) - 4)
        context.fillStyle = '#ffffff'
        context.beginPath()
        context.roundRect(x, y, 8, 8, 1)
        context.fill()
        context.strokeStyle = color
        context.beginPath()
        context.roundRect(x + 0.5, y + 0.5, 7, 7, 0.5)
        context.stroke()
        return true
      }
      if (overlay.size === 4 && overlay.filled) {
        // The Scale tool's pivot: a white disc (its ring follows as a path) with a dot.
        circle(context, p.x, p.y, 4, '#ffffff', null)
        circle(context, p.x, p.y, 1.5, color, null)
        return true
      }
      if (overlay.size === 11 && overlay.filled) {
        // The midpoint a press adds, under the pointer: accent with a white ring.
        circle(context, p.x, p.y, 5.5, color, '#ffffff', 1.2)
        return true
      }
      const r = overlay.size / 2
      const fill = overlay.filled ? color : alpha < 1 ? 'rgba(255, 255, 255, 0.7)' : '#ffffff'
      if (!vectorEditing(overlays)) {
        // The pen's anchors: discs of r 3.5 with a ring outside.
        circle(context, p.x, p.y, r, fill, color, 1)
        return true
      }
      // A vector point: a round square (radius 5, so a circle), white or filled, with a 1 px
      // inside ring; the faint ones (the point a press would add) on a translucent white.
      circle(context, p.x, p.y, r, fill, null)
      circle(context, p.x, p.y, Math.max(0, r - 1), null, color, 1)
      return true
    }
    case 'handle': {
      const p = toScreen(overlay.x, overlay.y)
      // An ellipse's arc handles are r 4 (Start with a dot); bezier and radius handles r 3.5.
      const arc = curvedSelection(overlays) && !hasPoints(overlays)
      const r = arc ? 4 : 3.5
      circle(context, p.x, p.y, r, '#ffffff', overlayCss(overlay.color), 1)
      if (arc) {
        // Start is the second of the three (Ratio, Start, Sweep), drawn once the ellipse has an arc.
        const handles = overlays.filter((o) => o.kind === 'handle')
        if (handles.length === 3 && handles[1] === overlay) circle(context, p.x, p.y, 1.5, overlayCss(overlay.color), null)
      }
      return true
    }
    case 'measure': {
      const prev = overlays[index - 1]
      if (prev?.kind === 'line' && isGuide(prev.color) && Math.abs((prev.ax + prev.bx) / 2 - overlay.x) < 1e-6 && Math.abs((prev.ay + prev.by) / 2 - overlay.y) < 1e-6) {
        guideLabel(overlay, at)
        return true
      }
      // An arc or radius readout: left-top at the point (the engine put it 12 px off the handle).
      const p = toScreen(overlay.x, overlay.y)
      const size = pillSize(context, overlay.text, 11)
      pill(context, overlay.text, { x: p.x, y: p.y, ...size }, underWhite(selectionColour(overlays)), 11)
      return true
    }
    default:
      return false
  }
}
