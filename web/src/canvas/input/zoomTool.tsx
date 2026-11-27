/**
 * The zoom tool while Z is held (walkthrough B11): a click on the canvas zooms in one step about
 * the point clicked, Alt+click zooms out one step, and a drag zooms to the area dragged (more
 * than 4 px each way; less is a click). The cursor says which. Letting go of Z gives the canvas
 * back to the tool that was active; a drag already under way still ends with its zoom. Not while
 * a field or a text on the canvas takes the keys, nor with Cmd or Ctrl (Cmd+Z undoes).
 */
import { useEffect, useRef, useState } from 'react'
import type { DesignKit } from '../../kit/context'
import { composingKey } from '../inputMethod'
import { canvasSurface, followPress, usePressLayer } from './order'

/** Below this many screen pixels each way a zoom drag is a click. */
export const ZOOM_AREA_MIN_PX = 4

type Area = { x0: number; y0: number; x1: number; y1: number }

/** Whether keys pressed on `target` are typed into it. */
function typing(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  if (target.classList.contains('kui-text-sink')) return false
  return target.isContentEditable || target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement
}

/**
 * Holds the zoom tool while Z is down: `held` is null when it is not, else whether it zooms out
 * (Alt); `area` is the area being dragged, in canvas pixels.
 */
export function useZoomTool(designKit: DesignKit) {
  const latest = useRef(designKit)
  latest.current = designKit
  const [held, setHeld] = useState<boolean | null>(null)
  const heldRef = useRef<boolean | null>(null)
  const [area, setArea] = useState<Area | null>(null)

  useEffect(() => {
    let z = false
    const show = (alt: boolean) => {
      const next = z ? alt : null
      if (heldRef.current !== next) {
        heldRef.current = next
        setHeld(next)
      }
    }
    const down = (event: KeyboardEvent) => {
      if (event.code === 'KeyZ' && !event.metaKey && !event.ctrlKey && !composingKey(event)) {
        const kit = latest.current
        const text = kit.tools.wantsText || kit.inspect.editing === 'text'
        if (!z && (text || typing(event.target))) return
        z = true
      } else if (event.code === 'KeyZ' && (event.metaKey || event.ctrlKey)) {
        z = false
      }
      show(event.altKey)
    }
    const up = (event: KeyboardEvent) => {
      if (event.code === 'KeyZ') z = false
      show(event.altKey)
    }
    const away = () => {
      z = false
      show(false)
    }
    window.addEventListener('keydown', down, true)
    window.addEventListener('keyup', up, true)
    window.addEventListener('blur', away)
    return () => {
      window.removeEventListener('keydown', down, true)
      window.removeEventListener('keyup', up, true)
      window.removeEventListener('blur', away)
    }
  }, [])

  usePressLayer('zoom', (event) => {
    if (heldRef.current === null) return undefined
    const surface = canvasSurface()
    if (!surface) return undefined
    const box = surface.getBoundingClientRect()
    const at = (e: { clientX: number; clientY: number }) => ({ x: e.clientX - box.left, y: e.clientY - box.top })
    const start = at(event)
    if (start.x < 0 || start.y < 0 || start.x > box.width || start.y > box.height) return undefined
    let end = start
    followPress(event, {
      move: (e) => {
        end = at(e)
        setArea({ x0: start.x, y0: start.y, x1: end.x, y1: end.y })
      },
      end: (e) => {
        end = at(e)
        setArea(null)
        const kit = latest.current
        const view = kit.canvas.current?.view()
        if (!view || !(view.zoom > 0)) return
        const doc = (p: { x: number; y: number }) => ({ x: view.x + p.x / view.zoom, y: view.y + p.y / view.zoom })
        const a = doc({ x: Math.min(start.x, end.x), y: Math.min(start.y, end.y) })
        const b = doc({ x: Math.max(start.x, end.x), y: Math.max(start.y, end.y) })
        if (Math.abs(end.x - start.x) > ZOOM_AREA_MIN_PX && Math.abs(end.y - start.y) > ZOOM_AREA_MIN_PX) {
          void kit.viewTo('view.zoomAt', { atX: a.x, atY: a.y, atWidth: b.x - a.x, atHeight: b.y - a.y })
        } else {
          const p = doc(end)
          void kit.viewTo('view.zoomAt', { atX: p.x, atY: p.y, out: e.altKey })
        }
      },
      cancel: () => setArea(null),
    })
    return 'take'
  })

  return { held, area }
}

/** The area a zoom drag will fill the view with. */
export function ZoomArea({ area }: { area: Area | null }) {
  if (!area) return null
  return (
    <div
      className="design-zoom-area"
      data-testid="design-zoom-area"
      style={{ left: Math.min(area.x0, area.x1), top: Math.min(area.y0, area.y1), width: Math.abs(area.x1 - area.x0), height: Math.abs(area.y1 - area.y0) }}
    />
  )
}
