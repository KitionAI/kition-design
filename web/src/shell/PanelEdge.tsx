/** A panel's resize edge (2.1 §1 `panel_edge`): a 10 px strip over the seam; double-click resets to 240. */
import { useState } from 'react'
import { PANEL_DEFAULT } from './layout'

export function PanelEdge({ x, side, width, onWidth, label, testId }: { x: number; side: 'left' | 'right'; width: number; onWidth: (width: number, done: boolean) => void; label: string; testId?: string }) {
  const [dragging, setDragging] = useState(false)
  return (
    <div
      className="design-edge"
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={Math.round(width)}
      data-dragging={dragging || undefined}
      data-testid={testId}
      style={{ left: x }}
      onDoubleClick={() => onWidth(PANEL_DEFAULT, true)}
      onPointerDown={(event) => {
        if (event.button !== 0) return
        event.preventDefault()
        const target = event.currentTarget
        target.setPointerCapture(event.pointerId)
        setDragging(true)
        const start = event.clientX
        const from = width
        const move = (e: PointerEvent) => onWidth(from + (e.clientX - start) * (side === 'left' ? 1 : -1), false)
        const up = (e: PointerEvent) => {
          target.removeEventListener('pointermove', move)
          target.removeEventListener('pointerup', up)
          setDragging(false)
          onWidth(from + (e.clientX - start) * (side === 'left' ? 1 : -1), true)
        }
        target.addEventListener('pointermove', move)
        target.addEventListener('pointerup', up)
      }}
    />
  )
}
