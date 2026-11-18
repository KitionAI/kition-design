/**
 * The batch rename's names, for the Rename layers dialog's preview: a port of the engine's
 * `selection/rename.rs` (`BatchRename::apply`), since the engine has no dry run yet (requested as
 * `rename.preview`). Codes: `$&` the current name (the match when matching), `` $` `` and `$'` the
 * text before and after the match, `$1`…`$9` groups, `$n` up and `$N` down (`$nnn` padded), `$$`.
 */

const MAX_FIND = 1000
const MAX_NAME = 10000
const MAX_PAD = 12

type Ctx = { matched: string; before: string; after: string; groups: RegExpExecArray | null; up: number; down: number }

function expand(template: string, c: Ctx): string {
  let out = ''
  const chars = [...template]
  for (let i = 0; i < chars.length; i += 1) {
    const ch = chars[i]
    if (ch !== '$') {
      out += ch
      continue
    }
    const next = chars[i + 1]
    if (next === '&') out += c.matched
    else if (next === '`') out += c.before
    else if (next === "'") out += c.after
    else if (next === '$') out += '$'
    else if (next === 'n' || next === 'N') {
      let width = 0
      while (chars[i + 1] === next) {
        i += 1
        width += 1
      }
      out += String(next === 'n' ? c.up : c.down).padStart(Math.min(width, MAX_PAD), '0')
      continue
    } else if (next !== undefined && next >= '1' && next <= '9') {
      const group = Number(next)
      if (c.groups && group < c.groups.length) out += c.groups[group] ?? ''
      else out += `$${next}`
    } else {
      out += '$'
      continue
    }
    i += 1
  }
  return out
}

export type RenamePlan = { apply: (name: string, k: number, n: number) => string } | { error: string }

/** `find` (empty: none; text, or a regular expression with `regex`) replaced by `to`; without a find, `to` is the whole name's pattern. */
export function batchRename(find: string, regex: boolean, to: string, start: number): RenamePlan {
  if ([...find].length > MAX_FIND) return { error: `the match is longer than ${MAX_FIND} characters` }
  let pattern: RegExp | null = null
  if (find && regex) {
    try {
      pattern = new RegExp(find, 'gu')
    } catch (error) {
      return { error: `invalid regular expression: ${error instanceof Error ? error.message : String(error)}` }
    }
  }
  const from = Number.isFinite(start) ? Math.round(Math.min(Math.max(start, 0), 1e12)) : 1
  return {
    apply: (name, k, n) => {
      const up = from + k
      const down = from + Math.max(0, n - k - 1)
      let out = name
      if (find && pattern) {
        out = ''
        let last = 0
        pattern.lastIndex = 0
        for (let m = pattern.exec(name); m; m = pattern.exec(name)) {
          out += name.slice(last, m.index)
          out += expand(to, { matched: m[0], before: name.slice(0, m.index), after: name.slice(m.index + m[0].length), groups: m, up, down })
          last = m.index + m[0].length
          if (m[0] === '') pattern.lastIndex += 1
        }
        out += name.slice(last)
      } else if (find) {
        out = ''
        let last = 0
        for (let at = name.indexOf(find); at >= 0; at = name.indexOf(find, at + find.length)) {
          out += name.slice(last, at)
          out += expand(to, { matched: find, before: name.slice(0, at), after: name.slice(at + find.length), groups: null, up, down })
          last = at + find.length
        }
        out += name.slice(last)
      } else {
        out = expand(to, { matched: name, before: '', after: '', groups: null, up, down })
      }
      return [...out].slice(0, MAX_NAME).join('')
    },
  }
}
