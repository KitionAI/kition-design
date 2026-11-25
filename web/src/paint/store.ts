/**
 * The paint package's shared UI state: which paint the colour picker edits (`PickerTarget`
 * upstream), the selected gradient stop (shared with the canvas handles), the open effect
 * settings, the eyedropper, and the colours picked this session ("Recent", newest first, 16 at
 * most, one per 8-bit colour). Other packages open the picker with `openPicker(target)`.
 */
import { useSyncExternalStore } from 'react'
import { type Color, sameColor } from './color'

/** One frozen paint slot of `select.colors` (`PaintTarget`). */
export type PaintSlot = { id: number; fills: boolean; index: number; range: [number, number] | null } & Record<string, unknown>

export type PickerTarget =
  | { kind: 'paint'; fills: boolean; index: number; ids: number[] }
  | { kind: 'effect'; ids: number[]; index: number; secondary: boolean }
  | { kind: 'selectionColor'; targets: PaintSlot[]; selection: number[] }
  | { kind: 'textRange'; id: number; start: number; end: number }
  | { kind: 'page' }
  | { kind: 'variable'; id: string; mode: string }
  | { kind: 'style'; id: string; index: number }

type State = {
  picker: PickerTarget | null
  /** The gradient stop the picker and the canvas handles edit. */
  stop: number
  /** Effects (by index) whose settings are open under their rows. */
  effectsOpen: ReadonlySet<number>
  /** Advanced stroke settings are open (one flag for every layer, as upstream). */
  strokeAdvanced: boolean
  /** The eyedropper samples the canvas for the picker. */
  eyedropper: boolean
  recent: Color[]
}

let state: State = { picker: null, stop: 0, effectsOpen: new Set(), strokeAdvanced: false, eyedropper: false, recent: [] }
const listeners = new Set<() => void>()

function set(next: Partial<State>) {
  state = { ...state, ...next }
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function usePaintState(): State {
  return useSyncExternalStore(subscribe, () => state, () => state)
}

export function paintState(): State {
  return state
}

/** Opens the colour picker on `target` (closing it on another). */
export function openPicker(target: PickerTarget) {
  set({ picker: target, eyedropper: false, stop: samePicker(state.picker, target) ? state.stop : 0 })
}

export function closePicker() {
  set({ picker: null, eyedropper: false })
}

export function updatePicker(target: PickerTarget | null) {
  set({ picker: target })
}

export function setStop(stop: number) {
  if (stop !== state.stop) set({ stop })
}

export function setEyedropper(on: boolean) {
  set({ eyedropper: on })
}

export function setStrokeAdvanced(on: boolean) {
  set({ strokeAdvanced: on })
}

export function setEffectsOpen(open: ReadonlySet<number>) {
  set({ effectsOpen: open })
}

export function toggleEffectOpen(index: number) {
  const next = new Set(state.effectsOpen)
  if (next.has(index)) next.delete(index)
  else next.add(index)
  set({ effectsOpen: next })
}

/** Remembers a chosen colour first in Recent (`remember_recent`). */
export function rememberRecent(c: Color) {
  const recent = [c, ...state.recent.filter((x) => !sameColor(x, c))].slice(0, 16)
  set({ recent })
}

export function samePicker(a: PickerTarget | null, b: PickerTarget | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}
