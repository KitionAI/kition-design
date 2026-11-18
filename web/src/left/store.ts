/**
 * The left panel's own state, kept while the panel is collapsed or switches tabs (upstream keeps
 * it in `app.ui`): which layers are open, the searches, renames, the pages list's height, the
 * layer drag, and the Assets folders that are folded away.
 */
import { useSyncExternalStore } from 'react'

export type LayerDrag = {
  /** The row the drag started on. */
  from: number
  /** What it moves: the selection when the row is in it, else the row. */
  ids: number[]
  label: string
  x: number
  y: number
  /** The page row under the pointer (a drop moves the layers there). */
  page: number | null
}

export type LeftState = {
  expanded: ReadonlySet<number>
  /** The selection the tree last revealed (its ancestors opened once, its row scrolled to). */
  seenSelection: string
  /** Where a Shift-click range starts. */
  anchor: number | null
  layerSearch: boolean
  layerFilter: string
  pagesOpen: boolean
  pageSearch: boolean
  pageFilter: string
  /** The pages list's height asked by its splitter (null: 40 % of the room). */
  pagesHeight: number | null
  /** The layer being renamed; `start` while its field has yet to take the keyboard. */
  rename: { id: number; start: boolean } | null
  renamePage: number | null
  /** The Pages list has the keyboard (Delete with nothing selected deletes the current page). */
  pagesFocus: boolean
  drag: LayerDrag | null
  assetFilter: string
  /** Assets folders (and libraries) folded away. */
  folded: ReadonlySet<string>
  /** The Assets tab shows the libraries view. */
  libraries: boolean
}

const HEIGHT_KEY = 'kit.design.pagesHeight'

function storedHeight(): number | null {
  try {
    const value = JSON.parse(localStorage.getItem(HEIGHT_KEY) ?? 'null') as unknown
    return typeof value === 'number' && Number.isFinite(value) ? value : null
  } catch {
    return null
  }
}

let state: LeftState = {
  expanded: new Set(),
  seenSelection: '',
  anchor: null,
  layerSearch: false,
  layerFilter: '',
  pagesOpen: true,
  pageSearch: false,
  pageFilter: '',
  pagesHeight: storedHeight(),
  rename: null,
  renamePage: null,
  pagesFocus: false,
  drag: null,
  assetFilter: '',
  folded: new Set(),
  libraries: false,
}

const listeners = new Set<() => void>()

export function leftState(): LeftState {
  return state
}

/** Changes the state (a partial, or a function of the current state). */
export function setLeft(change: Partial<LeftState> | ((now: LeftState) => Partial<LeftState>)) {
  const next = typeof change === 'function' ? change(state) : change
  state = { ...state, ...next }
  if ('pagesHeight' in next) {
    try {
      localStorage.setItem(HEIGHT_KEY, JSON.stringify(state.pagesHeight))
    } catch {
      // No storage in this frame: the height lasts for the session.
    }
  }
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useLeft(): LeftState {
  return useSyncExternalStore(subscribe, leftState, leftState)
}

/** Opens or closes `id`'s row. */
export function toggleExpanded(id: number, open?: boolean) {
  setLeft((now) => {
    const expanded = new Set(now.expanded)
    const on = open ?? !expanded.has(id)
    if (on) expanded.add(id)
    else expanded.delete(id)
    return { expanded }
  })
}
