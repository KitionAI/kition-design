/**
 * The selection as the paint sections read it (`query('selection')`): every selected layer's
 * properties, the values they share and the keys whose values differ, and the edited text's
 * character runs.
 */
import { useMemo } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'

export type NodeJson = Record<string, unknown> & { id: number; type: string; name?: string }

export type TextRun = { start: number; end: number; style: Record<string, unknown> }

export type SelectionData = {
  ids: number[]
  count: number
  kind: string | null
  types: string[]
  nodes: NodeJson[]
  common: Record<string, unknown>
  mixed: string[]
  text: { id: number; range: [number, number] | null; runs: TextRun[] } | null
}

/** The selection's data, or null while the first answer is on its way or nothing is selected. */
export function useSelectionData(): SelectionData | null {
  const { inspect } = useDesignKit()
  const answer = useQuery<SelectionData>('selection', {}, { skip: inspect.selection.length === 0 })
  return answer && answer.count > 0 ? answer : null
}

/**
 * The value every selected layer shares for `key` (`common()` upstream): `undefined` when no
 * layer has it, `MIXED` when they differ.
 */
export const MIXED = Symbol('mixed')

export function common<T = unknown>(data: SelectionData, key: string): T | undefined | typeof MIXED {
  if (data.mixed.includes(key)) return MIXED
  return data.common[key] as T | undefined
}

/** Each selected layer's value of `key` (in selection order). */
export function each<T = unknown>(data: SelectionData, key: string): Array<T | undefined> {
  return data.nodes.map((n) => n[key] as T | undefined)
}

/** The edited text's selected range `[start, end)` (a caret is no range). */
export function textRange(inspect: { textEdit: { id: number; caret: number; anchor: number } | null }): { id: number; start: number; end: number } | null {
  const e = inspect.textEdit
  if (!e || e.caret === e.anchor) return null
  return { id: e.id, start: Math.min(e.caret, e.anchor), end: Math.max(e.caret, e.anchor) }
}

/** Memoises the ids of the selected layers of the given types. */
export function useIdsOfType(data: SelectionData | null, types: string[]): number[] {
  return useMemo(() => (data ? data.nodes.filter((n) => types.includes(n.type)).map((n) => n.id) : []), [data, types])
}
