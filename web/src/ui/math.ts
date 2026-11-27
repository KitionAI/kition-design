/**
 * Number field math (2.1 §7): `10+5`, `100/3`, `(2+3)*4`, `50%` of the current value, and, starting
 * with an operator (`+10`, `*2`, `/2`, or `-5` on a mixed value), an edit relative to each value.
 */

export type NumberEdit =
  /** Sets every value to `value`. */
  | { kind: 'set'; value: number }
  /** Applies `apply` to each value (a mixed selection gets it per layer). */
  | { kind: 'relative'; apply: (current: number) => number; text: string }

type Token = { kind: 'num'; value: number } | { kind: 'op'; value: string }

function tokenize(text: string): Token[] | null {
  const tokens: Token[] = []
  let i = 0
  while (i < text.length) {
    const c = text[i]
    if (c === ' ') {
      i += 1
      continue
    }
    const number = /^(\d+(?:[.,]\d*)?|[.,]\d+)(?:e[+-]?\d+)?/i.exec(text.slice(i))
    if (number) {
      tokens.push({ kind: 'num', value: Number(number[0].replace(',', '.')) })
      i += number[0].length
      continue
    }
    if ('+-*/()%x×÷'.includes(c)) {
      tokens.push({ kind: 'op', value: c === 'x' || c === '×' ? '*' : c === '÷' ? '/' : c })
      i += 1
      continue
    }
    return null
  }
  return tokens
}

/** Evaluates an arithmetic expression (`%` is a percentage of `base`); null when it is not one. */
export function evaluate(text: string, base = 0): number | null {
  const tokens = tokenize(text.trim())
  if (!tokens || !tokens.length) return null
  let at = 0
  const peek = () => tokens[at]
  const expression = (): number | null => {
    let left = term()
    while (left !== null && peek()?.kind === 'op' && (peek()!.value === '+' || peek()!.value === '-')) {
      const op = tokens[at++].value
      const right = term()
      if (right === null) return null
      left = op === '+' ? left + right : left - right
    }
    return left
  }
  const term = (): number | null => {
    let left = factor()
    while (left !== null && peek()?.kind === 'op' && (peek()!.value === '*' || peek()!.value === '/')) {
      const op = tokens[at++].value
      const right = factor()
      if (right === null) return null
      left = op === '*' ? left * right : left / right
    }
    return left
  }
  const factor = (): number | null => {
    const token = tokens[at++]
    if (!token) return null
    let value: number | null
    if (token.kind === 'num') value = token.value
    else if (token.value === '-') value = (() => {
      const inner = factor()
      return inner === null ? null : -inner
    })()
    else if (token.value === '+') value = factor()
    else if (token.value === '(') {
      value = expression()
      if (tokens[at++]?.value !== ')') return null
    } else return null
    if (value !== null && peek()?.kind === 'op' && peek()!.value === '%') {
      at += 1
      value = (value / 100) * base
    }
    return value
  }
  const value = expression()
  return value !== null && at === tokens.length && Number.isFinite(value) ? value : null
}

/**
 * What typing `text` into a field holding `current` (a number, or null when mixed) does: a value,
 * a relative edit, or nothing (null) when it is not a number.
 */
export function parseEdit(text: string, current: number | null): NumberEdit | null {
  const trimmed = text.trim()
  if (!trimmed) return null
  const lead = trimmed[0]
  const relative = lead === '+' || lead === '*' || lead === '/' || lead === '×' || lead === '÷' || (lead === '-' && current === null)
  if (relative) {
    const rest = trimmed.slice(1)
    const operand = evaluate(rest, current ?? 0)
    if (operand === null) return null
    const op = lead === '×' ? '*' : lead === '÷' ? '/' : lead
    const raw = (value: number) => (op === '+' ? value + operand : op === '-' ? value - operand : op === '*' ? value * operand : operand === 0 ? value : value / operand)
    // A result that overflows (`*1e308`) leaves the value as it was.
    const apply = (value: number) => {
      const next = raw(value)
      return Number.isFinite(next) ? next : value
    }
    if (current !== null) {
      const next = raw(current)
      return Number.isFinite(next) ? { kind: 'set', value: next } : null
    }
    return { kind: 'relative', apply, text: trimmed }
  }
  const value = evaluate(trimmed, current ?? 0)
  return value === null ? null : { kind: 'set', value }
}

/**
 * Two numbers typed into one field, `10, 30` or `10 30`: separated by a comma with a space after
 * or before it, or by spaces alone. A comma between digits (`10,5`) is a decimal comma, not a pair.
 */
export function parsePair(text: string): [number, number] | null {
  const match = /^\s*(-?\d+(?:\.\d+)?)(?:\s*,\s+|\s+,\s*|\s+)(-?\d+(?:\.\d+)?)\s*$/.exec(text)
  if (!match) return null
  const first = Number(match[1])
  const second = Number(match[2])
  return Number.isFinite(first) && Number.isFinite(second) ? [first, second] : null
}

/** A number as fields show it: up to `decimals` places, no trailing zeros. */
export function formatNumber(value: number, decimals = 2) {
  if (!Number.isFinite(value)) return ''
  const factor = 10 ** decimals
  const rounded = Math.round(value * factor) / factor
  return String(Object.is(rounded, -0) ? 0 : rounded)
}
