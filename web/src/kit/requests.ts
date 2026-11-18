/**
 * UI requests: engine commands only ask for UI (`Session::request_ui`); their answers carry
 * `uiRequests: [{ kind, data }]` and the tools' answers `dialog` requests. Whoever shows that UI
 * listens here: `useUiRequest('panel.devMode', () => …)`.
 */
import { useEffect, useRef } from 'react'

export type UiRequest = { kind: string; data: unknown }
type Listener = (request: UiRequest) => void

const listeners = new Map<string, Set<Listener>>()

/** Gives each request to the listeners of its kind (and of `*`). */
export function dispatchUiRequests(requests: readonly UiRequest[]) {
  for (const request of requests) {
    for (const listener of [...(listeners.get(request.kind) ?? []), ...(listeners.get('*') ?? [])]) listener(request)
  }
}

/** Dispatches the `uiRequests` of a command's answer (a JSON string or a parsed value). */
export function dispatchFromAnswer(answer: unknown) {
  let value = answer
  if (typeof value === 'string') {
    if (!value.includes('uiRequests')) return
    try {
      value = JSON.parse(value)
    } catch {
      return
    }
  }
  const requests = (value as { uiRequests?: unknown } | null)?.uiRequests
  if (Array.isArray(requests)) dispatchUiRequests(requests.filter((request): request is UiRequest => typeof request?.kind === 'string'))
}

export function onUiRequest(kind: string, listener: Listener) {
  let set = listeners.get(kind)
  if (!set) listeners.set(kind, (set = new Set()))
  set.add(listener)
  return () => {
    set.delete(listener)
  }
}

/** Calls `handler` for every UI request of `kind` (`*` for all) while mounted. */
export function useUiRequest(kind: string, handler: (data: unknown, request: UiRequest) => void) {
  const latest = useRef(handler)
  latest.current = handler
  useEffect(() => onUiRequest(kind, (request) => latest.current(request.data, request)), [kind])
}
