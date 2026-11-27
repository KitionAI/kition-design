/**
 * The selected layers' position and size while a pointer gesture runs. The panels' queries wait
 * for the gesture to end (`revision` holds, context.tsx); the engine's answers carry these few
 * numbers instead (`canvas.gesture`, bindings/wasm/src/tools.rs `live_requests`), so the Design
 * panel's X, Y, W, H and rotation follow the drag.
 */
import { useMemo, useSyncExternalStore } from 'react'
import { onUiRequest } from './requests'

export type LiveGeometry = { id: number; x: number; y: number; width: number; height: number; rotation: number }

type State = { serial: number; byId: ReadonlyMap<number, LiveGeometry> }
let state: State = { serial: 0, byId: new Map() }
const listeners = new Set<() => void>()
/** The serial when each panel answer was first shown: geometry older than its answer is stale. */
const firstShown = new WeakMap<object, number>()

const finite = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value)

onUiRequest('canvas.gesture', (request) => {
  const nodes = (request.data as { nodes?: unknown } | null)?.nodes
  if (!Array.isArray(nodes)) return
  const byId = new Map<number, LiveGeometry>()
  for (const node of nodes as Array<Record<string, unknown>>) {
    if (finite(node?.id) && finite(node.x) && finite(node.y) && finite(node.width) && finite(node.height)) {
      byId.set(node.id, { id: node.id, x: node.x, y: node.y, width: node.width, height: node.height, rotation: finite(node.rotation) ? node.rotation : 0 })
    }
  }
  state = { serial: state.serial + 1, byId }
  for (const listener of [...listeners]) listener()
})

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/**
 * `nodes` (a panel answer's layers) with the position and size the engine's answers gave since
 * that answer came; the answer itself once a newer one (the one after the gesture) is shown.
 */
export function useLiveNodes<N extends { id: number; x?: number; y?: number; width?: number; height?: number; rotation?: number }>(nodes: N[]): N[] {
  const now = useSyncExternalStore(subscribe, () => state, () => state)
  if (!firstShown.has(nodes)) firstShown.set(nodes, now.serial)
  const since = firstShown.get(nodes) ?? now.serial
  const live = now.serial > since && nodes.some((node) => now.byId.has(node.id))
  return useMemo(
    () =>
      live
        ? nodes.map((node) => {
            const at = now.byId.get(node.id)
            return at ? { ...node, x: at.x, y: at.y, width: at.width, height: at.height, rotation: at.rotation } : node
          })
        : nodes,
    [live, nodes, now],
  )
}
