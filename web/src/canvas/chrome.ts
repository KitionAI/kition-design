/**
 * Draws the canvas chrome (`chrome(zoom)`, bindings/wasm/src/chrome.rs) in the upstream editor's
 * style (2.5 §3, §4, §7, §8): pixel grid, layout grids, frame labels and section pills with
 * status badges, the size pill, auto layout bands and grid tracks, ruler guides, rulers. Drawn
 * over the rendered page as a kit-editor decoration, so it follows every pan and zoom.
 */
import type { RasterDecoration } from '@kition/sdk/editor'
import { ICON_SVG_ATTRS, iconMarkup, iconMetrics, type IconName } from '../icons/icons'
import type { DesignChrome } from '../kit/types'

export const PINK = '#f2488c'
const ACCENT = '#00875f'
export const FONT = 'Inter, "Droid Sans Fallback", ui-sans-serif, system-ui, sans-serif'
/** The editor's semibold weight (`--design-semibold`, app.css), so canvas labels match the panels. */
export function semibold() {
  return getComputedStyle(document.documentElement).getPropertyValue('--design-semibold').trim() || '500'
}

/** The theme colours the chrome takes (2.5 §0.1): read from the CSS tokens. */
export type ChromeTheme = { textSecondary: string; textTertiary: string; text: string; hover: string; panel: string; border: string }

export function readTheme(): ChromeTheme {
  const css = getComputedStyle(document.documentElement)
  const get = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback
  return {
    textSecondary: get('--design-text-secondary', '#6e6e6e'),
    textTertiary: get('--design-text-tertiary', '#8a8a8a'),
    text: get('--design-text', '#1e1e1e'),
    hover: get('--design-hover', '#f5f5f5'),
    panel: get('--design-panel', '#ffffff'),
    border: get('--design-border', '#e6e6e6'),
  }
}

/** Which auto layout band the pointer is over or drags (`padding:<side>` or `gap:<index>`). */
export type ZoneKey = string | null

// Icons drawn on the canvas, as images per colour.
const ICON_IMAGES = new Map<string, HTMLImageElement>()

function iconImage(name: IconName, size: number, color: string, onLoad: () => void) {
  const ratio = window.devicePixelRatio || 1
  const key = `${name}:${size}:${color}:${ratio}`
  let image = ICON_IMAGES.get(key)
  if (!image) {
    const m = iconMetrics(size, ratio)
    const attrs = Object.entries({ ...ICON_SVG_ATTRS, strokeWidth: m.stroke })
      .map(([k, v]) => `${k.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}="${v}"`)
      .join(' ')
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size * ratio}" height="${size * ratio}" viewBox="${m.viewBox}" ${attrs} color="${color}" style="color:${color}">${iconMarkup(name, m, `c${ICON_IMAGES.size}`)}</svg>`
    image = new Image()
    image.onload = onLoad
    image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg.replace(/currentColor/g, color))}`
    ICON_IMAGES.set(key, image)
  }
  return image.complete && image.naturalWidth > 0 ? image : null
}

function ellipsize(context: CanvasRenderingContext2D, text: string, width: number) {
  if (context.measureText(text).width <= width) return text
  let low = 0
  let high = text.length
  while (low < high) {
    const mid = Math.ceil((low + high) / 2)
    if (context.measureText(`${text.slice(0, mid)}…`).width <= width) low = mid
    else high = mid - 1
  }
  return `${text.slice(0, low)}…`
}

/** A pill with white text (status badges, pink and guide labels, the size pill). */
function pill(context: CanvasRenderingContext2D, text: string, x: number, y: number, fill: string, font: string, padX: number, padY: number, radius: number, align: 'left' | 'center' = 'left') {
  context.font = font
  const width = Math.ceil(context.measureText(text).width) + padX * 2
  const height = Math.round(parseFloat(font.split(' ')[1]) * 1.2) + padY * 2
  const left = Math.round(align === 'center' ? x - width / 2 : x)
  const top = Math.round(y)
  context.fillStyle = fill
  context.beginPath()
  context.roundRect(left, top, width, height, radius)
  context.fill()
  context.fillStyle = '#ffffff'
  context.textBaseline = 'middle'
  context.textAlign = 'left'
  context.fillText(text, left + padX, top + height / 2 + 0.5)
  return { left, top, width, height }
}

const fmt = (value: number) => String(Math.round(value * 100) / 100)

/**
 * A decoration that draws `chrome`. `hot` is the auto layout band under the pointer. The engine
 * tools' overlays go between the two parts, as upstream draws them: the `under` part (pixel grid,
 * layout grids, frame labels) is kit-editor's `decorateUnder`, the `over` part (auto layout bands,
 * the size pill, ruler guides and rulers) its `decorate`.
 */
export function chromeDecoration(chrome: DesignChrome, theme: ChromeTheme, hot: ZoneKey, redraw: () => void, part: 'under' | 'over', hiddenGuide: { owner: number; index: number } | null = null): RasterDecoration {
  return (context, toScreen, view) => {
    const zoom = view.zoom
    const sb = semibold()
    const width = context.canvas.clientWidth || context.canvas.width
    const height = context.canvas.clientHeight || context.canvas.height
    context.save()

    // Pixel grid: 1 px lines at every whole canvas unit.
    if (part === 'under' && chrome.pixelGrid && zoom >= 4) {
      context.strokeStyle = 'rgba(0, 0, 0, 0.07)'
      context.lineWidth = 1
      context.beginPath()
      const first = toScreen(Math.floor(view.x), Math.floor(view.y))
      for (let x = first.x; x <= width; x += zoom) {
        const px = Math.round(x - 0.5) + 0.5
        context.moveTo(px, 0)
        context.lineTo(px, height)
      }
      for (let y = first.y; y <= height; y += zoom) {
        const py = Math.round(y - 0.5) + 0.5
        context.moveTo(0, py)
        context.lineTo(width, py)
      }
      context.stroke()
    }

    // Layout grids: column and row bands, or grid lines, in each grid's colour.
    for (const grid of part === 'under' ? chrome.grids : []) {
      context.fillStyle = grid.color
      context.strokeStyle = grid.color
      context.lineWidth = 1
      for (const r of grid.rects) {
        const a = toScreen(r.x, r.y)
        const b = toScreen(r.x + r.width, r.y + r.height)
        if (grid.lines) {
          context.beginPath()
          context.moveTo(Math.round(a.x) + 0.5, Math.round(a.y) + 0.5)
          context.lineTo(Math.round(b.x) + 0.5, Math.round(b.y) + 0.5)
          context.stroke()
        } else context.fillRect(a.x, a.y, b.x - a.x, b.y - a.y)
      }
    }

    // Frame labels and section pills.
    for (const label of part === 'under' ? chrome.labels : []) {
      const at = toScreen(label.x, label.y)
      if (at.x < -600 || at.y < -40 || at.x > width + 40 || at.y > height + 60) continue
      context.save()
      context.translate(Math.round(at.x), Math.round(at.y))
      if (label.angle) context.rotate(label.angle)
      if (label.kind === 'section') {
        const font = `${sb} 11px ${FONT}`
        context.font = font
        const eye = label.contentsHidden ? 16 : 0
        const limit = Math.max(20, Math.min(480, label.maxWidth - 16 - eye))
        const text = ellipsize(context, label.name, limit)
        const w = Math.ceil(context.measureText(text).width) + 16 + eye
        const h = 13 + 8
        context.fillStyle = label.selected ? ACCENT : theme.hover
        context.beginPath()
        context.roundRect(0, 0, w, h, 4)
        context.fill()
        if (eye) {
          const image = iconImage('EyeOff', 12, label.selected ? '#ffffff' : theme.textSecondary, redraw)
          if (image) context.drawImage(image, 8, h / 2 - 6, 12, 12)
        }
        context.fillStyle = label.selected ? '#ffffff' : theme.text
        context.textBaseline = 'middle'
        context.fillText(text, 8 + eye, h / 2 + 0.5)
        if (label.status) pill(context, label.status.label, w + 6, h / 2 - 8, label.status.color, `${sb} 10px ${FONT}`, 5, 2, 4)
      } else {
        const color = label.color ?? theme.textSecondary
        const component = label.kind === 'component' || label.kind === 'componentSet'
        const shift = component ? 14 : 0
        context.font = `380 11px ${FONT}`
        // Slack: the browser measures Inter a little wider than upstream's rasterizer.
        const text = ellipsize(context, label.name, Math.max(20, label.maxWidth - shift) + 2.5)
        if (component) {
          const image = iconImage('Component', 12, color, redraw)
          if (image) context.drawImage(image, 0, -15, 12, 12)
        }
        context.fillStyle = color
        context.textBaseline = 'alphabetic'
        context.fillText(text, shift, -5)
        if (label.status) {
          const end = shift + Math.ceil(context.measureText(text).width) + 6
          pill(context, label.status.label, end, -9 - 8, label.status.color, `${sb} 10px ${FONT}`, 5, 2, 4)
        }
      }
      context.restore()
    }

    if (part === 'under') {
      context.restore()
      return
    }

    // Auto layout: grid tracks always, a band only while hovered or dragged.
    const al = chrome.autoLayout
    if (al) {
      context.strokeStyle = 'rgba(242, 72, 140, 0.6)'
      context.lineWidth = 1
      context.setLineDash([3, 3])
      for (const [a, b] of al.tracks) {
        const p = toScreen(a[0], a[1])
        const q = toScreen(b[0], b[1])
        context.beginPath()
        context.moveTo(Math.round(p.x) + 0.5, Math.round(p.y) + 0.5)
        context.lineTo(Math.round(q.x) + 0.5, Math.round(q.y) + 0.5)
        context.stroke()
      }
      context.setLineDash([])
      const band = (key: string, rect: { x: number; y: number; width: number; height: number }, value: number, gap: boolean) => {
        if (key !== hot) return
        const a = toScreen(rect.x, rect.y)
        const b = toScreen(rect.x + rect.width, rect.y + rect.height)
        context.fillStyle = gap ? 'rgba(242, 72, 140, 0.3)' : 'rgba(242, 72, 140, 0.25)'
        context.fillRect(a.x, a.y, b.x - a.x, b.y - a.y)
        const cx = (a.x + b.x) / 2
        const cy = (a.y + b.y) / 2
        if (gap) {
          context.strokeStyle = PINK
          context.beginPath()
          context.moveTo(Math.round(cx - 3), Math.round(cy) + 0.5)
          context.lineTo(Math.round(cx + 3), Math.round(cy) + 0.5)
          context.stroke()
        }
        const font = `${sb} 10px ${FONT}`
        context.font = font
        const w = Math.ceil(context.measureText(fmt(value)).width) + 8
        pill(context, fmt(value), cx - w / 2, cy - 8, PINK, font, 4, 2, 3)
      }
      al.padding.forEach((zone) => band(`padding:${zone.side}`, zone.rect, zone.value, false))
      al.gaps.forEach((zone, index) => band(`gap:${index}`, zone.rect, zone.value, true))
    }

    // The size pill under the selection.
    if (chrome.sizeLabel) {
      const at = toScreen(chrome.sizeLabel.x, chrome.sizeLabel.y)
      pill(context, chrome.sizeLabel.text, at.x, at.y + 9, chrome.sizeLabel.color, `${sb} 11px ${FONT}`, 4, 2, 3, 'center')
    }

    // Rulers and their guides.
    const rulers = chrome.rulers
    if (rulers) {
      context.lineWidth = 1
      for (const guide of chrome.guides) {
        // The guide being dragged shows where it will land instead (input/guides.tsx).
        if (hiddenGuide && guide.owner === hiddenGuide.owner && guide.index === hiddenGuide.index) continue
        const a = toScreen(guide.a[0], guide.a[1])
        const b = toScreen(guide.b[0], guide.b[1])
        context.strokeStyle = chrome.colors.guide
        context.lineWidth = guide.selected ? 2 : 1
        context.beginPath()
        if (guide.vertical) {
          const x = Math.round(a.x) + (guide.selected ? 0 : 0.5)
          context.moveTo(x, Math.max(20, a.y))
          context.lineTo(x, b.y)
        } else {
          const y = Math.round(a.y) + (guide.selected ? 0 : 0.5)
          context.moveTo(Math.max(20, a.x), y)
          context.lineTo(b.x, y)
        }
        context.stroke()
        if (guide.selected) pill(context, fmt(guide.offset), Math.max(20, a.x) + 4, Math.max(20, a.y) + 4, chrome.colors.guide, `${sb} 10px ${FONT}`, 4, 2, 3)
      }
      const step = rulers.step
      context.fillStyle = theme.panel
      context.fillRect(0, 0, width, 20)
      context.fillRect(0, 0, 20, height)
      if (rulers.selection) {
        const s = rulers.selection
        const a = toScreen(s.x, s.y)
        const b = toScreen(s.x + s.width, s.y + s.height)
        context.fillStyle = hexAlpha(chrome.colors.accent, 0.15)
        context.fillRect(a.x, 0, b.x - a.x, 20)
        context.fillRect(0, a.y, 20, b.y - a.y)
      }
      context.strokeStyle = theme.textTertiary
      context.fillStyle = theme.textSecondary
      context.font = `380 9px ${FONT}`
      context.textBaseline = 'top'
      context.beginPath()
      // Canvas unit u is document pixel u + origin.
      const firstX = Math.floor((view.x - rulers.originX) / step) * step
      for (let u = firstX; ; u += step) {
        const x = toScreen(u + rulers.originX, 0).x
        if (x > width) break
        if (x < 20) continue
        context.moveTo(Math.round(x) + 0.5, 14)
        context.lineTo(Math.round(x) + 0.5, 20)
        context.fillText(fmt(u), Math.round(x) + 2, 3)
      }
      const firstY = Math.floor((view.y - rulers.originY) / step) * step
      for (let u = firstY; ; u += step) {
        const y = toScreen(0, u + rulers.originY).y
        if (y > height) break
        if (y < 20) continue
        context.moveTo(14, Math.round(y) + 0.5)
        context.lineTo(20, Math.round(y) + 0.5)
        context.fillText(fmt(u), 3, Math.round(y) + 2)
      }
      context.stroke()
      context.fillStyle = theme.border
      context.fillRect(0, 20, width, 1)
      context.fillRect(20, 0, 1, height)
      context.fillStyle = theme.panel
      context.fillRect(0, 0, 20, 20)
    }
    context.restore()
  }
}

function hexAlpha(hex: string, alpha: number) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})/i.exec(hex)
  return m ? `rgba(${parseInt(m[1], 16)}, ${parseInt(m[2], 16)}, ${parseInt(m[3], 16)}, ${alpha})` : hex
}
