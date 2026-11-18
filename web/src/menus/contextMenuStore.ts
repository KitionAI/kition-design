/**
 * The open context menu (2.5 §18 canvas, 2.2 layers): where it opens and for what. The canvas
 * asks with the `contextMenu` UI request; the Layers panel calls `openContextMenu` (source
 * `layers`) from a row's right-click, naming the rows it is for.
 */
import { useSyncExternalStore } from 'react'

export type ContextMenuRequest = {
  clientX: number
  clientY: number
  /** `canvas` (the default) adds Select layer ▸ and pastes at the point; `layers` is the Layers panel's. */
  source?: 'canvas' | 'layers'
  /** Cmd/Ctrl right-click: the deepest layer under the point. */
  deep?: boolean
  /** The layers the menu is for (a Layers row right-clicked outside the selection: just itself); the selection by default. */
  ids?: number[]
}

let open: ContextMenuRequest | null = null
const listeners = new Set<() => void>()

export function openContextMenu(request: ContextMenuRequest) {
  open = { ...request }
  listeners.forEach((listener) => listener())
}

export function closeContextMenu() {
  if (!open) return
  open = null
  listeners.forEach((listener) => listener())
}

export function useContextMenu(): ContextMenuRequest | null {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => open,
  )
}
