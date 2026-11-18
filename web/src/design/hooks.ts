/**
 * The Design panel's shared state and its field hooks: the selection, `node.set` writes,
 * number fields whose typed, stepped and scrubbed values go through a command (one undo step per
 * scrub, relative edits of a mixed value per layer, as upstream's `field_of`), and shortcuts by
 * command id for tooltips.
 */
import { createContext, useCallback, useContext, useMemo, useRef } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import type { Interaction } from '../kit/hooks'
import type { NumberEdit, ScrubHandlers } from '../ui'
import { tip } from '../ui'
import type { NodeJson, SelectionAnswer } from './model'

export type Design = {
  answer: SelectionAnswer
  nodes: NodeJson[]
  ids: number[]
  /** `node.set { ids, props }` on the selection (one undo step). */
  setProps: (props: Record<string, unknown>, ids?: readonly number[]) => void
}

export const DesignContext = createContext<Design | null>(null)

export function useDesign(): Design {
  const design = useContext(DesignContext)
  if (!design) throw new Error('useDesign outside the Design panel')
  return design
}

/** The engine's shortcuts by command id (`query('keymap')`), as tooltips: `tipFor('Align left', 'align.left')`. */
export function useShortcutTip() {
  const answer = useQuery<{ bindings: Array<{ id: string; keys: string[] }> }>('keymap')
  return useMemo(() => {
    const keys = new Map<string, string>()
    for (const binding of answer?.bindings ?? []) if (!keys.has(binding.id) && binding.keys[0]) keys.set(binding.id, binding.keys[0])
    return (label: string, command: string) => tip(label, keys.get(command))
  }, [answer])
}

type Build = (value: number) => { command: string; params: Record<string, unknown> }

/**
 * A number field's `onCommit` and `scrub` through `build(value)` (the command for one value). With
 * `perLayer` (each layer's own value and its command), a relative edit of a mixed value goes to each
 * layer from its own value in one undo step.
 */
export function useCommandField(build: Build, perLayer?: { ids: readonly number[]; of: (id: number) => number | null; build: (id: number, value: number) => { command: string; params: Record<string, unknown> } }) {
  const { run, interaction } = useDesignKit()
  const latest = useRef({ build, perLayer })
  latest.current = { build, perLayer }
  const onCommit = useCallback(
    (edit: NumberEdit) => {
      const { build: one, perLayer: each } = latest.current
      if (edit.kind === 'set') {
        const { command, params } = one(edit.value)
        void run(command, params)
      } else if (each && each.ids.length > 1) {
        const commands = each.ids.map((id) => each.build(id, edit.apply(each.of(id) ?? 0)))
        void run('command.batch', { label: 'Edit', commands })
      } else {
        const first = each?.ids[0]
        const base = first !== undefined ? (each?.of(first) ?? 0) : 0
        const { command, params } = one(edit.apply(base))
        void run(command, params)
      }
    },
    [run],
  )
  const active = useRef<Interaction | null>(null)
  // Each layer's value when the scrub began: the drag's delta applies to it, never to the live
  // value the scrub has already moved.
  const starts = useRef<Map<number, number | null>>(new Map())
  const scrub = useMemo<ScrubHandlers>(
    () => ({
      begin: () => {
        const each = latest.current.perLayer
        starts.current = new Map((each?.ids ?? []).map((id) => [id, each?.of(id) ?? null]))
        active.current = interaction('Edit')
      },
      move: (value, delta) => {
        const { build: one, perLayer: each } = latest.current
        const from = starts.current
        const first = each?.ids[0]
        if (each && first !== undefined && each.ids.length > 1 && each.ids.some((id) => from.get(id) !== from.get(first))) {
          active.current?.update('command.batch', { label: 'Edit', commands: each.ids.map((id) => each.build(id, (from.get(id) ?? 0) + delta)) })
        } else {
          const { command, params } = one(value)
          active.current?.update(command, params)
        }
      },
      end: (cancelled) => {
        const current = active.current
        active.current = null
        void (cancelled ? current?.cancel() : current?.commit())
      },
    }),
    [interaction],
  )
  return { onCommit, scrub }
}

/**
 * A field writing `node.set { [key]: map(value) }` on `ids`; `of(node)` is the value each layer
 * shows (for relative edits of mixed values).
 */
export function usePropField(ids: readonly number[], key: string, options: { map?: (value: number) => unknown; of?: (node: NodeJson) => number | null; nodes?: readonly NodeJson[] } = {}) {
  const map = options.map ?? ((v: number) => v)
  const byId = useMemo(() => new Map((options.nodes ?? []).map((n) => [n.id, n])), [options.nodes])
  const of = options.of
  return useCommandField(
    (value) => ({ command: 'node.set', params: { ids, props: { [key]: map(value) } } }),
    of ? { ids, of: (id) => (byId.get(id) ? of(byId.get(id) as NodeJson) : null), build: (id, value) => ({ command: 'node.set', params: { ids: [id], props: { [key]: map(value) } } }) } : undefined,
  )
}
