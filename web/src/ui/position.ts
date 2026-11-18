/** Where a popup goes next to its anchor, kept inside the window. */
export type Placement = 'below-start' | 'below-end' | 'above-start' | 'above-end' | 'right-start' | 'left-start'

export type AnchorRect = { left: number; top: number; right: number; bottom: number }

export function anchorRect(anchor: Element | AnchorRect | null | undefined): AnchorRect | null {
  if (!anchor) return null
  if (anchor instanceof Element) {
    const r = anchor.getBoundingClientRect()
    return { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
  }
  return anchor
}

/** The popup's top-left for `size` at `placement` of `anchor`, `gap` px away, inside the window less `margin`. */
export function place(anchor: AnchorRect, size: { width: number; height: number }, placement: Placement, gap = 4, margin = 8) {
  const vw = window.innerWidth
  const vh = window.innerHeight
  let left = 0
  let top = 0
  switch (placement) {
    case 'below-start':
      left = anchor.left
      // egui's popups try the mirrored alignment (BOTTOM_END) before shifting into the window.
      if (left + size.width > vw - margin && anchor.right - size.width >= margin) left = anchor.right - size.width
      top = anchor.bottom + gap
      if (top + size.height > vh - margin && anchor.top - gap - size.height >= margin) top = anchor.top - gap - size.height
      break
    case 'below-end':
      left = anchor.right - size.width
      top = anchor.bottom + gap
      if (top + size.height > vh - margin && anchor.top - gap - size.height >= margin) top = anchor.top - gap - size.height
      break
    case 'above-start':
      left = anchor.left
      top = anchor.top - gap - size.height
      break
    case 'above-end':
      left = anchor.right - size.width
      top = anchor.top - gap - size.height
      break
    case 'right-start':
      left = anchor.right + gap
      top = anchor.top
      if (left + size.width > vw - margin) left = vw - margin - size.width
      break
    case 'left-start':
      left = anchor.left - gap - size.width
      top = anchor.top
      break
  }
  left = Math.max(margin, Math.min(left, vw - margin - size.width))
  top = Math.max(margin, Math.min(top, vh - margin - size.height))
  return { left: Math.round(left), top: Math.round(top) }
}
