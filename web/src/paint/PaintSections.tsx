/**
 * Typography, Fill, Stroke and Effects (2.4), in the Design panel's order, for the selection; the
 * colour picker (mounted with the canvas, so it also serves the page's background and the other
 * packages) opens from their swatches, or from any package with `openPicker(target)` or the UI
 * request `picker.open` (`{ target: 'paint' | 'effect' | 'selectionColor' | 'textRange' | 'page' |
 * 'variable' | 'style', … }`, the fields of `PickerTarget`).
 */
import { useEffect } from 'react'
import { registerCanvasLayer } from '../canvas/overlays'
import { useDesignKit } from '../kit/hooks'
import { onUiRequest } from '../kit/requests'
import { useSelectionData } from './data'
import { EffectsSection } from './Effects'
import { PaintSection } from './PaintList'
import { TargetColorPicker } from './PaintPicker'
import { closePicker, openPicker, paintState, type PaintSlot, type PickerTarget } from './store'
import { TypographySection } from './Typography'
import './paint.css'

export { ColorPicker, type ColorPickerProps, type ChangePhase } from './ColorPicker'
export { openPicker, closePicker, type PickerTarget } from './store'
export { PaintSwatch } from './Swatch'

registerCanvasLayer('paint.picker', TargetColorPicker)

/** `picker.open` requests from other packages: the target's kind and its fields. */
function targetOf(data: unknown): PickerTarget | null {
  if (!data || typeof data !== 'object') return null
  const d = data as Record<string, unknown>
  const ids = Array.isArray(d.ids) ? (d.ids as number[]) : []
  switch (d.target ?? d.kind) {
    case 'paint':
      return { kind: 'paint', fills: d.fills !== false, index: Number(d.index ?? 0), ids }
    case 'effect':
      return { kind: 'effect', ids, index: Number(d.index ?? 0), secondary: d.secondary === true }
    case 'selectionColor':
      return { kind: 'selectionColor', targets: (Array.isArray(d.targets) ? d.targets : []) as PaintSlot[], selection: Array.isArray(d.selection) ? (d.selection as number[]) : [] }
    case 'textRange':
      return { kind: 'textRange', id: Number(d.id), start: Number(d.start), end: Number(d.end) }
    case 'page':
    case 'pageBackground':
      return { kind: 'page' }
    case 'variable':
      return { kind: 'variable', id: String(d.id ?? d.variable), mode: String(d.mode) }
    case 'style':
      return { kind: 'style', id: String(d.id ?? d.style), index: Number(d.index ?? 0) }
    default:
      return null
  }
}

let selectionNow: number[] = []
onUiRequest('picker.open', (request) => {
  const target = targetOf(request.data)
  if (!target) return
  // A selection colour belongs to the selection it was opened for.
  if (target.kind === 'selectionColor' && !target.selection.length) target.selection = selectionNow
  openPicker(target)
})
onUiRequest('picker.close', () => closePicker())

export function PaintSections(_props: { text?: boolean } = {}) {
  const { inspect } = useDesignKit()
  const data = useSelectionData()
  selectionNow = inspect.selection
  // A selection colour's picker closes once the selection moves on.
  useEffect(() => {
    const p = paintState().picker
    if (p?.kind === 'selectionColor' && JSON.stringify(p.selection) !== JSON.stringify(inspect.selection)) closePicker()
  }, [inspect.selection])
  if (!data) return null
  const text = data.nodes.some((n) => n.type === 'TEXT')
  return (
    <div className="design-paint-sections" data-testid="design-paint-sections">
      {text ? <TypographySection data={data} /> : null}
      <PaintSection data={data} fills />
      <PaintSection data={data} fills={false} />
      <EffectsSection data={data} />
    </div>
  )
}
