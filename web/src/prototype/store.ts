/**
 * The prototype package's UI state, shared by the Prototype tab, its canvas layer and the player:
 * which interaction's editor is open (upstream `open_editor`), which action in it is open, whether
 * a key is being recorded, and the preview pane and presenter.
 */
import { useSyncExternalStore } from 'react'

export type EditorTarget = { node: number; index: number }
export type PlayerState = { mode: 'preview' | 'present'; start: number | null; key: number }

type State = {
  editor: EditorTarget | null
  /** The open action's path per interaction (`node:index`), `[0]` by default. */
  openPaths: Record<string, number[]>
  capturing: boolean
  player: PlayerState | null
  /** The preview pane stays open under the presenter (Present from the pane). */
  pane: PlayerState | null
}

let state: State = { editor: null, openPaths: {}, capturing: false, player: null, pane: null }
const listeners = new Set<() => void>()
let keys = 0

function set(next: Partial<State>) {
  state = { ...state, ...next }
  for (const listener of listeners) listener()
}

const subscribe = (listener: () => void) => {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useProtoState<R>(select: (s: State) => R): R {
  return useSyncExternalStore(subscribe, () => select(state))
}

export const protoState = () => state

export function openEditor(target: EditorTarget | null) {
  const same = target && state.editor && target.node === state.editor.node && target.index === state.editor.index
  if (same || (!target && !state.editor)) return
  set({ editor: target, capturing: false })
}

export const openPathKey = (t: EditorTarget) => `${t.node}:${t.index}`

export function setOpenPath(target: EditorTarget, path: number[]) {
  set({ openPaths: { ...state.openPaths, [openPathKey(target)]: path } })
}

export function setCapturing(on: boolean) {
  if (state.capturing !== on) set({ capturing: on })
}

/** Opens the preview pane (`preview`) or the presenter (`present`) at frame `start`. */
export function openPlayer(mode: 'preview' | 'present', start: number | null) {
  keys += 1
  const next = { mode, start, key: keys }
  if (mode === 'preview') set({ pane: next })
  else set({ player: next })
}

export function closePlayer(mode: 'preview' | 'present') {
  if (mode === 'preview') set({ pane: null })
  else set({ player: null })
}
