/** The toast (2.1 §5): one at a time, dark in both themes, bottom-centred above the toolbar, 3 s. */
import { useSyncExternalStore } from 'react'

const TOAST_MS = 3000

let current: { text: string; id: number } | null = null
let next = 0
let timer: number | undefined
const listeners = new Set<() => void>()

function emit() {
  for (const listener of listeners) listener()
}

/** Shows `text` as the toast, replacing the one shown. */
export function showToast(text: string) {
  current = { text, id: ++next }
  window.clearTimeout(timer)
  timer = window.setTimeout(() => {
    current = null
    emit()
  }, TOAST_MS)
  emit()
}

const subscribe = (listener: () => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Draws the toast; mounted once, inside the canvas cell. */
export function ToastHost() {
  const toast = useSyncExternalStore(subscribe, () => current)
  return toast ? (
    <div className="design-toast" role="status" aria-live="polite" data-testid="design-toast" key={toast.id}>
      {toast.text}
    </div>
  ) : null
}
