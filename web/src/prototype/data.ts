/**
 * What the prototype package reads from the engine: the page's flows, settings and top-level
 * frames and the selection's interactions (`query('prototype')`), the page's layer tree
 * (`query('layers', { all })`, for names, parents, sections and "Scroll to" targets) and the
 * variables (`query('variables')`).
 */
import { useMemo } from 'react'
import { useQuery } from '../kit/hooks'
import type { Flow, Interaction, PrototypeSettings, Variables } from './model'

export type ProtoAnswer = {
  interactions: Array<{ id: number; name: string; interactions: Interaction[] | null }>
  flows: Flow[]
  settings: PrototypeSettings | null
  frames: Array<{ id: number; name: string }>
}

type Row = { id: number; name: string; type: string; children?: Row[] }

export type LayerInfo = { id: number; name: string; type: string; parent: number | null; children: number[] }

export type Tree = {
  page: number
  nodes: Map<number, LayerInfo>
  /** The page's sections, top-most first. */
  sections: number[]
}

const FRAME_LIKE = new Set(['FRAME', 'COMPONENT', 'COMPONENT_SET', 'INSTANCE'])
export const isFrameLike = (type: string | undefined) => FRAME_LIKE.has(type ?? '')

export function useTree(): Tree | null {
  const answer = useQuery<{ page: number; rows: Row[] }>('layers', { all: true })
  return useMemo(() => {
    if (!answer) return null
    const nodes = new Map<number, LayerInfo>()
    const sections: number[] = []
    const walk = (rows: Row[], parent: number | null) => {
      for (const row of rows) {
        nodes.set(row.id, { id: row.id, name: row.name, type: row.type, parent, children: (row.children ?? []).map((c) => c.id) })
        if (row.type === 'SECTION') sections.push(row.id)
        if (row.children) walk(row.children, row.id)
      }
    }
    walk(answer.rows, null)
    return { page: answer.page, nodes, sections }
  }, [answer])
}

/** The top-level frame of `id` (past any sections), as `Document::top_level_frame`. */
export function topLevelFrame(tree: Tree | null, id: number): number | null {
  if (!tree) return null
  const chain: number[] = []
  let at: number | null = id
  while (at != null && chain.length < 256) {
    chain.push(at)
    at = tree.nodes.get(at)?.parent ?? null
  }
  // The outermost that is not a section.
  for (let i = chain.length - 1; i >= 0; i -= 1) {
    const node = tree.nodes.get(chain[i]!)
    if (node?.type === 'SECTION') continue
    return isFrameLike(node?.type) ? chain[i]! : null
  }
  return null
}

/** Every layer under `root` (not itself), depth first, at most `limit`. */
export function subtree(tree: Tree | null, root: number, limit = 400): number[] {
  const out: number[] = []
  const walk = (id: number) => {
    for (const child of tree?.nodes.get(id)?.children ?? []) {
      if (out.length >= limit) return
      out.push(child)
      walk(child)
    }
  }
  walk(root)
  return out
}

export function usePrototype() {
  return useQuery<ProtoAnswer>('prototype')
}

export function useVariables() {
  return useQuery<Variables>('variables')
}

/** A layer's name for summaries (null when it is gone). */
export function nameIn(tree: Tree | null, frames: ProtoAnswer['frames'] | undefined) {
  return (id: number) => tree?.nodes.get(id)?.name ?? frames?.find((f) => f.id === id)?.name ?? null
}
