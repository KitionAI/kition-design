/**
 * The hooks panels use (see web/README.md, "Extension points"): the inspected document, the
 * selection, queries, commands, interactions, the View menu's checks, and number fields that write
 * layer properties.
 */
import { useCallback, useMemo, useRef } from 'react'
import type { NumberEdit } from '../ui/math'
import type { ScrubHandlers } from '../ui/NumberField'
import { useDesignKit, useQuery, type Interaction } from './context'
import type { DesignInspect } from './types'

export { useDesignKit, useQuery, useTools, useView, type Interaction } from './context'

/** The engine's `inspect()` (document, page, selection, tool, editing modes); re-renders on every change. */
export function useInspect(): DesignInspect {
  return useDesignKit().inspect
}

/** The selected layer ids, and helpers to change the selection. */
export function useSelection() {
  const { inspect, run } = useDesignKit()
  const ids = inspect.selection
  return useMemo(
    () => ({
      ids,
      count: ids.length,
      bounds: inspect.selectionBounds,
      /** Selects exactly `next` (an empty list clears it). */
      set: (next: number[]) => run('select.set', { ids: next }),
      /** Adds `id` to the selection, or removes it when selected (Shift-click). */
      toggle: (id: number) => run('select.set', { ids: ids.includes(id) ? ids.filter((other) => other !== id) : [...ids, id] }),
      clear: () => run('select.set', { ids: [] }),
    }),
    [ids, inspect.selectionBounds, run],
  )
}

/** `run(command, params)`: an engine command; answers its parsed result, or null when it failed (a toast says why). */
export function useRun() {
  return useDesignKit().run
}

/**
 * Starts interactions: `const drag = useInteraction(); const i = drag('Padding'); i.update(…); i.commit()`.
 * Each is one undo step when the binding has the interaction extras.
 */
export function useInteraction(): (label: string) => Interaction {
  return useDesignKit().interaction
}

/** The View and Preferences toggles that are on (`query('checked')`), as a set of command ids. */
export function useChecked(): ReadonlySet<string> {
  const answer = useQuery<{ ids: string[] }>('checked')
  return useMemo(() => new Set(answer?.ids ?? []), [answer])
}

/**
 * A number field's `onCommit` and `scrub` that write property `prop` of layers `ids` with
 * `node.set` (one undo step per scrub). `valueOf` gives each layer's own value, for relative edits
 * of a mixed value (`+10` to each); without it a relative edit applies to 0.
 */
export function useNumberProp(ids: readonly number[], prop: string, label: string, valueOf?: (id: number) => number | null): { onCommit: (edit: NumberEdit) => void; scrub: ScrubHandlers } {
  const { run, interaction } = useDesignKit()
  const latest = useRef({ ids, valueOf })
  latest.current = { ids, valueOf }
  const perLayer = useCallback((apply: (value: number) => number) => {
    const { ids: now, valueOf: of } = latest.current
    return now.map((id) => ({ command: 'node.set', params: { ids: [id], props: { [prop]: apply(of?.(id) ?? 0) } } }))
  }, [prop])
  const onCommit = useCallback(
    (edit: NumberEdit) => {
      const now = latest.current.ids
      if (!now.length) return
      if (edit.kind === 'set') void run('node.set', { ids: now, props: { [prop]: edit.value } })
      else void run('command.batch', { label, commands: perLayer(edit.apply) })
    },
    [label, perLayer, prop, run],
  )
  const current = useRef<Interaction | null>(null)
  // Each layer's value when the scrub began: the drag's delta applies to it, never to the live
  // value the scrub has already moved.
  const starts = useRef<Map<number, number>>(new Map())
  const scrub = useMemo<ScrubHandlers>(
    () => ({
      begin: () => {
        const { ids: now, valueOf: of } = latest.current
        starts.current = new Map(now.map((id) => [id, of?.(id) ?? 0]))
        current.current = interaction(label)
      },
      move: (value, delta) => {
        const now = latest.current.ids
        const from = starts.current
        if (latest.current.valueOf && now.length > 1) current.current?.update('command.batch', { label, commands: now.map((id) => ({ command: 'node.set', params: { ids: [id], props: { [prop]: (from.get(id) ?? 0) + delta } } })) })
        else current.current?.update('node.set', { ids: now, props: { [prop]: value } })
      },
      end: (cancelled) => {
        const active = current.current
        current.current = null
        void (cancelled ? active?.cancel() : active?.commit())
      },
    }),
    [interaction, label, prop],
  )
  return { onCommit, scrub }
}
