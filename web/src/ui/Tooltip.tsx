/**
 * Tooltips (2.1 §7): any element with `data-tip="Label  ⇧L"` shows it after 400 ms of hover, in
 * the popup frame, 4 px below the element and starting at its left (above it when there is no room
 * below), as egui places a widget's tooltip. One layer serves the whole editor (`TooltipLayer`).
 */
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'

const DELAY_MS = 400

type Shown = { text: string; x: number; y: number; bottom: number }

/** egui's popup gap between a widget and its tooltip. */
const GAP = 4

export function TooltipLayer() {
  const [shown, setShown] = useState<Shown | null>(null)
  const box = useRef<HTMLDivElement>(null)
  const [at, setAt] = useState<{ left: number; top: number } | null>(null)

  useEffect(() => {
    let timer: number | undefined
    let target: HTMLElement | null = null
    const hide = () => {
      window.clearTimeout(timer)
      target = null
      setShown(null)
    }
    const over = (event: PointerEvent) => {
      const found = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-tip]') : null
      if (found === target) return
      window.clearTimeout(timer)
      target = found
      setShown(null)
      if (!found) return
      timer = window.setTimeout(() => {
        // Not over a pressed or open control, and only while it is still in the page.
        if (!found.isConnected || found.getAttribute('aria-expanded') === 'true') return
        const text = found.dataset.tip
        if (!text) return
        const rect = found.getBoundingClientRect()
        setShown({ text, x: rect.left, y: rect.top, bottom: rect.bottom })
      }, DELAY_MS)
    }
    document.addEventListener('pointerover', over, true)
    document.addEventListener('pointerdown', hide, true)
    document.addEventListener('keydown', hide, true)
    window.addEventListener('blur', hide)
    return () => {
      window.clearTimeout(timer)
      document.removeEventListener('pointerover', over, true)
      document.removeEventListener('pointerdown', hide, true)
      document.removeEventListener('keydown', hide, true)
      window.removeEventListener('blur', hide)
    }
  }, [])

  useLayoutEffect(() => {
    const el = box.current
    if (!shown || !el) {
      setAt(null)
      return
    }
    const width = el.offsetWidth
    const height = el.offsetHeight
    let top = shown.bottom + GAP
    if (top + height > window.innerHeight - 4) top = shown.y - GAP - height
    const left = Math.max(4, Math.min(shown.x, window.innerWidth - width - 4))
    setAt({ left, top })
  }, [shown])

  if (!shown) return null
  return createPortal(
    <div ref={box} className="design-tooltip" role="tooltip" data-testid="design-tooltip" style={at ?? { left: -9999, top: -9999 }}>
      {shown.text}
    </div>,
    document.body,
  )
}
