/**
 * The document's variables as `query('variables')` (`variable.list`) answers them, and what the
 * Variables window and the variable picker derive from them: values resolved through aliases,
 * group paths, display text, unique names.
 */
import { useMemo } from 'react'
import { useQuery } from '../kit/hooks'

export type Rgba = { r: number; g: number; b: number; a?: number }
export type VariableKind = 'color' | 'number' | 'string' | 'boolean'
export type VariableValue = { type: 'color'; value: Rgba } | { type: 'number'; value: number } | { type: 'string'; value: string } | { type: 'boolean'; value: boolean } | { type: 'alias'; value: string }
export type Mode = { id: string; name: string }
export type Collection = { id: string; name: string; modes: Mode[]; extends?: string }
export type Variable = {
  id: string
  name: string
  collection: string
  resolvedType: VariableKind
  values: Record<string, VariableValue>
  description?: string
  scopes?: string[]
  hiddenFromPublishing?: boolean
  extends?: string
  overrides?: string[]
  codeSyntax?: Record<string, string>
}
export type Variables = { collections: Collection[]; variables: Variable[] }

const EMPTY: Variables = { collections: [], variables: [] }

export function useVariables(): Variables {
  const answer = useQuery<Partial<Variables>>('variables')
  return useMemo(() => (answer ? { collections: answer.collections ?? [], variables: answer.variables ?? [] } : EMPTY), [answer])
}

/** `id`'s value with each collection in the mode `modes` names (its default mode otherwise), through aliases. */
export function resolve(vars: Variables, id: string, modes: Record<string, string> = {}): VariableValue | null {
  let current = vars.variables.find((v) => v.id === id)
  for (let hops = 0; current && hops < 16; hops += 1) {
    const collection = vars.collections.find((c) => c.id === current?.collection)
    const mode = modes[current.collection] ?? collection?.modes[0]?.id ?? Object.keys(current.values)[0]
    const value = current.values[mode]
    if (!value) return null
    if (value.type !== 'alias') return value
    current = vars.variables.find((v) => v.id === value.value)
  }
  return null
}

/** The group part of a name ("space/md" → "space"; "" when ungrouped). */
export const groupOf = (name: string) => {
  const at = name.lastIndexOf('/')
  return at < 0 ? '' : name.slice(0, at)
}

/** Every group path in `names` with its ancestors, sorted. */
export function groups(names: string[]): string[] {
  const out = new Set<string>()
  for (const name of names) {
    for (let path = groupOf(name); path; path = groupOf(path)) out.add(path)
  }
  return [...out].sort()
}

/** The first of `base`, `base 2`, `base 3`… not taken. */
export function unique(base: string, taken: (name: string) => boolean): string {
  for (let k = 1; k < 10000; k += 1) {
    const name = k === 1 ? base : `${base} ${k}`
    if (!taken(name)) return name
  }
  return base
}

const hex2 = (v: number) => Math.round(Math.min(Math.max(v, 0), 1) * 255).toString(16).padStart(2, '0').toUpperCase()

/** `RRGGBB` (upstream's `to_hex`). */
export const hexOf = (c: Rgba) => `${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}`

/** `#RRGGBB` or `#RRGGBBAA` (`to_css_hex`). */
export const cssHexOf = (c: Rgba) => `#${hexOf(c)}${(c.a ?? 1) < 1 ? hex2(c.a ?? 1) : ''}`

/** A value as upstream's `VariableValue::display` writes it. */
export function display(value: VariableValue | null, t: (key: string) => string): string {
  if (!value) return ''
  switch (value.type) {
    case 'color':
      return hexOf(value.value)
    case 'number':
      return String(Math.round(value.value * 100) / 100)
    case 'boolean':
      return t(value.value ? 'True' : 'False')
    case 'string':
      return value.value
    default:
      return ''
  }
}

/** A value as `variable.set` takes it. */
export function valueJson(value: VariableValue): unknown {
  if (value.type === 'color') return cssHexOf(value.value)
  if (value.type === 'alias') return { alias: value.value }
  return value.value
}

/** The scopes a variable of `kind` can be limited to (`scopes_for`): id and label. */
export const SCOPES: Record<VariableKind, Array<[string, string]>> = {
  number: [
    ['CORNER_RADIUS', 'Corner radius'],
    ['WIDTH_HEIGHT', 'Width and height'],
    ['GAP', 'Gap and padding'],
    ['OPACITY', 'Layer opacity'],
    ['STROKE_FLOAT', 'Stroke'],
    ['EFFECT_FLOAT', 'Effects'],
    ['FONT_WEIGHT', 'Font weight'],
    ['FONT_SIZE', 'Font size'],
    ['LINE_HEIGHT', 'Line height'],
    ['LETTER_SPACING', 'Letter spacing'],
    ['PARAGRAPH_SPACING', 'Paragraph spacing'],
    ['PARAGRAPH_INDENT', 'Paragraph indent'],
    ['TEXT_CONTENT', 'Text content'],
  ],
  color: [
    ['FRAME_FILL', 'Frame fill'],
    ['SHAPE_FILL', 'Shape fill'],
    ['TEXT_FILL', 'Text fill'],
    ['STROKE_COLOR', 'Stroke'],
    ['EFFECT_COLOR', 'Effects'],
  ],
  string: [
    ['TEXT_CONTENT', 'Text content'],
    ['FONT_FAMILY', 'Font family'],
    ['FONT_STYLE', 'Font style'],
  ],
  boolean: [],
}

