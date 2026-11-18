/**
 * Floating layers: `Layer` (a portal positioned next to an anchor, closed by a press outside it or
 * Escape, the topmost first) and `Popover` (the light popup frame, 2.7 §0.3).
 */
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { anchorRect, place, type AnchorRect, type Placement } from './position'

/** Whether `layer` is the last floating layer in the page (the one Escape closes). */
function topmost(layer: HTMLElement) {
  const layers = document.querySelectorAll('.design-layer')
  return layers[layers.length - 1] === layer
}

/** Whether a press on `target` is outside `layer`, its anchor, and the layers opened after it. */
export function pressedOutside(layer: HTMLElement, target: EventTarget | null, anchor: Element | null) {
  if (!(target instanceof Node)) return true
  if (layer.contains(target) || anchor?.contains(target)) return false
  const other = target instanceof Element ? target.closest('.design-layer') : target.parentElement?.closest('.design-layer')
  return !(other && layer.compareDocumentPosition(other) & Node.DOCUMENT_POSITION_FOLLOWING)
}

export type LayerProps = {
  anchor: Element | AnchorRect | null
  placement?: Placement
  gap?: number
  onClose?: () => void
  /** Closes on a press outside (default true). */
  dismissable?: boolean
  className?: string
  style?: CSSProperties
  children: ReactNode
  testId?: string
  role?: string
  label?: string
  layerRef?: RefObject<HTMLDivElement | null>
}

export function Layer({ anchor, placement = 'below-start', gap = 4, onClose, dismissable = true, className = '', style, children, testId, role, label, layerRef }: LayerProps) {
  const own = useRef<HTMLDivElement>(null)
  const ref = layerRef ?? own
  const [at, setAt] = useState<{ left: number; top: number } | null>(null)
  const latest = useRef(onClose)
  latest.current = onClose

  useLayoutEffect(() => {
    const el = ref.current
    const rect = anchorRect(anchor)
    if (!el || !rect) return
    setAt(place(rect, { width: el.offsetWidth, height: el.offsetHeight }, placement, gap))
  }, [anchor, gap, placement, ref, children])

  useEffect(() => {
    const el = ref.current
    if (!el) return undefined
    const anchorElement = anchor instanceof Element ? anchor : null
    const down = (event: PointerEvent) => {
      if (dismissable && pressedOutside(el, event.target, anchorElement)) latest.current?.()
    }
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || !topmost(el)) return
      event.preventDefault()
      event.stopPropagation()
      latest.current?.()
    }
    document.addEventListener('pointerdown', down, true)
    window.addEventListener('keydown', key, true)
    return () => {
      document.removeEventListener('pointerdown', down, true)
      window.removeEventListener('keydown', key, true)
    }
  }, [anchor, dismissable, ref])

  return createPortal(
    <div ref={ref} className={`design-layer ${className}`} style={{ ...style, ...(at ?? { left: -9999, top: -9999 }) }} data-testid={testId} role={role} aria-label={label}>
      {children}
    </div>,
    document.body,
  )
}

export type PopoverProps = Omit<LayerProps, 'className'> & { open: boolean; width?: number; className?: string }

/** The light popup (fill `panel`, 1 px `border`, radius 13, padding 8, popup shadow). */
export function Popover({ open, width, className = '', style, ...props }: PopoverProps) {
  if (!open) return null
  return <Layer {...props} className={`design-popover ${className}`} style={{ ...style, width }} role={props.role ?? 'dialog'} />
}
