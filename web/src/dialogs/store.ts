/**
 * Which dialogs are open (2.7 §4). Upstream's dialogs are non-modal windows that several can share
 * the screen; the newest is on top and takes Escape first. Anything opens one with
 * `openDialog(kind, data)`; `DialogHost` draws them.
 */
import { useSyncExternalStore } from 'react'

export type DialogKind =
  | 'about'
  | 'preferences'
  | 'shortcuts'
  | 'findReplace'
  | 'batchRename'
  | 'link'
  | 'offset'
  | 'moveToPage'
  | 'colorProfile'
  | 'missingFonts'
  | 'importReport'
  | 'export'

export type OpenDialog = { kind: DialogKind; data: Record<string, unknown>; key: number }

let open: readonly OpenDialog[] = []
let next = 1
const listeners = new Set<() => void>()
const emit = () => listeners.forEach((listener) => listener())

/** Opens `kind` on top (a dialog already open moves on top with the new data, keeping what is typed in it). */
export function openDialog(kind: DialogKind, data: Record<string, unknown> = {}) {
  const was = open.find((dialog) => dialog.kind === kind)
  open = [...open.filter((dialog) => dialog.kind !== kind), { kind, data, key: was?.key ?? next++ }]
  emit()
}

export function closeDialog(kind: DialogKind) {
  if (!open.some((dialog) => dialog.kind === kind)) return
  open = open.filter((dialog) => dialog.kind !== kind)
  emit()
}

/** Opens `kind`, or closes it when open (the Keyboard shortcuts toggle). */
export function toggleDialog(kind: DialogKind, data?: Record<string, unknown>) {
  if (open.some((dialog) => dialog.kind === kind)) closeDialog(kind)
  else openDialog(kind, data)
}

export function isDialogOpen(kind: DialogKind) {
  return open.some((dialog) => dialog.kind === kind)
}

/** The open dialogs, oldest first. */
export function useDialogs(): readonly OpenDialog[] {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => open,
  )
}
