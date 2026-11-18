/**
 * The dialog window (2.7 §0.2, `dialogs::window`): fill `panel`, 1 px `border`, radius 13, the
 * window shadow; a 56 px title band (#DCDCDC light) with the centred title (semibold 13) and a
 * close X 28 px from the right; a 20 px padded body; buttons left-aligned 8 apart. Non-modal: no
 * scrim, the canvas stays live. The newest dialog is on top and takes Escape first (after any open
 * popup). Placement: centred (default), bottom-centred (Keyboard shortcuts), or at a point.
 */
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { Button } from '../ui'
import '../menus/menus.css'
import './dialogs.css'

export type DialogPlacement = { kind: 'center' } | { kind: 'bottom'; margin: number } | { kind: 'at'; left: number; top: number }

export type DialogProps = {
  title: string
  /** The content width (`set_width`); the window is this plus the 20 px margins (it grows for a wider row). */
  width: number
  placement?: DialogPlacement
  onClose: () => void
  children: ReactNode
  /** The buttons row at the bottom (left-aligned, 8 apart). */
  footer?: ReactNode
  testId?: string
  /** Upstream's plain egui window (Find and replace): a 12 px margin and a slimmer title. */
  plain?: boolean
  bodyStyle?: CSSProperties
}

/** Whether `el` is the newest open dialog. */
function topmost(el: HTMLElement) {
  const all = document.querySelectorAll('.design-dialog')
  return all[all.length - 1] === el
}

export function Dialog({ title, width, placement = { kind: 'center' }, onClose, children, footer, testId, plain = false, bodyStyle }: DialogProps) {
  const ref = useRef<HTMLDivElement>(null)
  // A plain egui window folds to its title bar with the triangle at its left.
  const [folded, setFolded] = useState(false)
  const [at, setAt] = useState<{ left: number; top: number } | null>(null)
  const latest = useRef(onClose)
  latest.current = onClose

  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return undefined
    const fit = () => {
      const vw = window.innerWidth
      const vh = window.innerHeight
      const w = el.offsetWidth
      const h = el.offsetHeight
      let left = (vw - w) / 2
      let top = (vh - h) / 2
      if (placement.kind === 'bottom') top = vh - placement.margin - h
      if (placement.kind === 'at') {
        left = placement.left
        top = placement.top
      }
      setAt({ left: Math.round(Math.max(8, Math.min(left, vw - w - 8))), top: Math.round(Math.max(8, Math.min(top, vh - h - 8))) })
    }
    fit()
    const observer = new ResizeObserver(fit)
    observer.observe(el)
    window.addEventListener('resize', fit)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', fit)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [placement.kind])

  useEffect(() => {
    const el = ref.current
    if (!el) return undefined
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented || !topmost(el)) return
      // An open popup (a dropdown, a menu) takes Escape first.
      if (document.querySelector('.design-layer')) return
      event.preventDefault()
      event.stopPropagation()
      latest.current()
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [])

  return createPortal(
    <div ref={ref} className="design-dialog" data-plain={plain || undefined} role="dialog" aria-label={title} data-testid={testId} style={at ?? { left: -9999, top: -9999 }}>
      <div className="design-dialog-title" data-folded={folded || undefined}>
        {plain ? (
          <button type="button" className="design-dialog-fold" aria-label={folded ? 'Expand' : 'Collapse'} aria-expanded={!folded} onClick={() => setFolded((now) => !now)} data-testid={testId ? `${testId}-fold` : undefined}>
            <svg width="11" height="10" viewBox="0 0 11 10" aria-hidden="true">
              <path d={folded ? 'M1 0 L10 5 L1 10 Z' : 'M0 0 L11 0 L5.5 10 Z'} fill="currentColor" />
            </svg>
          </button>
        ) : null}
        <span>{title}</span>
        <button type="button" className="design-dialog-close" aria-label="Close" onClick={() => latest.current()} data-testid={testId ? `${testId}-close` : undefined}>
          {/* egui's window close: a 12 px X of 1 px lines. */}
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <path d="M0.5 0.5 L11.5 11.5 M11.5 0.5 L0.5 11.5" fill="none" stroke="currentColor" strokeWidth="1" />
          </svg>
        </button>
      </div>
      {folded ? null : (
        <div className="design-dialog-body" style={{ width, ...bodyStyle }}>
          {children}
          {footer ? <div className="design-dialog-buttons">{footer}</div> : null}
        </div>
      )}
    </div>,
    document.body,
  )
}

/** A caption (`widgets::label`, regular 11), `secondary`, `danger` or `success` coloured. */
export function Caption({ children, tone, style }: { children: ReactNode; tone?: 'secondary' | 'danger' | 'success' | 'tertiary'; style?: CSSProperties }) {
  return (
    <div className="design-dialog-caption" data-tone={tone} style={style}>
      {children}
    </div>
  )
}

export { Button }
