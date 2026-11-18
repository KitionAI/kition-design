/**
 * HTML layers other packages put over the canvas (the colour picker's gradient, crop and blur
 * handles, the eyedropper): `useCanvasOverlay(key, element)` shows `element` over the canvas while
 * the caller is mounted. Elements are positioned in canvas pixels (`useView()` maps document
 * pixels) and render inside the editor's context. `registerCanvasLayer(key, Component)` mounts a
 * component there for good (a package's floating UI that must live while its panel is closed).
 */
import { useEffect, useSyncExternalStore, type ComponentType, type ReactNode } from 'react'

let layers = new Map<string, ReactNode>()
const mounted = new Map<string, ComponentType>()

/** Mounts `Component` with the canvas, whatever panel shows (call once, at module scope). */
export function registerCanvasLayer(key: string, Component: ComponentType) {
  mounted.set(key, Component)
}
const listeners = new Set<() => void>()

function emit() {
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** Shows `element` over the canvas under `key` (null removes it) while the caller is mounted. */
export function useCanvasOverlay(key: string, element: ReactNode | null) {
  useEffect(() => {
    const next = new Map(layers)
    if (element === null) next.delete(key)
    else next.set(key, element)
    layers = next
    emit()
  }, [element, key])
  useEffect(
    () => () => {
      const next = new Map(layers)
      next.delete(key)
      layers = next
      emit()
    },
    [key],
  )
}

/** Where DesignCanvas draws the registered layers. */
export function CanvasOverlays() {
  const shown = useSyncExternalStore(subscribe, () => layers, () => layers)
  return (
    <>
      {[...mounted.entries()].map(([key, Component]) => (
        <Component key={`mounted:${key}`} />
      ))}
      {[...shown.entries()].map(([key, element]) => (
        <div key={key} style={{ display: 'contents' }}>
          {element}
        </div>
      ))}
    </>
  )
}
