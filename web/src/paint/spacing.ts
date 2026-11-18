/**
 * Typography values: the line height and letter spacing parsers of upstream's `spacing.rs`
 * (`Auto`, `24`, `150%`; a bare number keeps the current unit), and the labels the engine's JSON
 * gives for them.
 */
import { evaluate } from '../ui/math'

/** A spacing value as the panel shows it: `Auto`, `150%`, `24`. */
export function spacingLabel(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (typeof value === 'string') return value.toLowerCase() === 'auto' ? 'Auto' : value
  if (typeof value === 'number') return trimNum(value)
  if (typeof value === 'object') {
    const v = value as { unit?: string; value?: number }
    if (v.unit === 'auto' || v.unit === 'Auto') return 'Auto'
    if (typeof v.value !== 'number') return null
    return v.unit === 'percent' || v.unit === 'Percent' ? `${trimNum(v.value)}%` : trimNum(v.value)
  }
  return null
}

export function trimNum(v: number): string {
  if (!Number.isFinite(v)) return '0'
  const r = Math.round(v * 100) / 100
  return String(Object.is(r, -0) ? 0 : r)
}

const isPercent = (label: string | null) => label !== null && label.endsWith('%')
const numberOf = (label: string | null) => (label === null ? null : Number.parseFloat(label))

/** `parse_line_height`: "Auto", "24", "150%" → the `lineHeight` value; null when nothing to commit. */
export function parseLineHeight(text: string, current: string | null): string | number | null {
  const s = text.trim()
  if (!s && current === null) return null
  if (!s || s.toLowerCase() === 'auto') return 'auto'
  if (s.endsWith('%')) {
    const v = evaluate(s.slice(0, -1).trim(), isPercent(current) ? (numberOf(current) ?? 0) : 0)
    return v === null ? null : `${trimNum(Math.max(0, v))}%`
  }
  if (s.endsWith('px')) {
    const v = evaluate(s.slice(0, -2).trim())
    return v === null ? null : Math.max(0, v)
  }
  const v = evaluate(s, numberOf(current) ?? 0)
  if (v === null) return null
  return isPercent(current) ? `${trimNum(Math.max(0, v))}%` : Math.max(0, v)
}

/** `parse_letter_spacing`: "2%", "1.5px", "-0.5" → the `letterSpacing` value (a bare number keeps the unit, percent by default). */
export function parseLetterSpacing(text: string, current: string | null): string | number | null {
  const s = text.trim()
  if (s.endsWith('%')) {
    const v = evaluate(s.slice(0, -1).trim())
    return v === null ? null : `${trimNum(v)}%`
  }
  if (s.endsWith('px')) return evaluate(s.slice(0, -2).trim())
  const v = evaluate(s, numberOf(current) ?? 0)
  if (v === null) return null
  return current !== null && !isPercent(current) ? v : `${trimNum(v)}%`
}
