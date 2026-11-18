/**
 * Keyboard shortcuts (2.7 §3.3, Help ▸ Keyboard shortcuts, ⌃⇧?): a window 16 px above the bottom,
 * content width clamp(window − 48, 480, 1040). A tab strip of the 12 groups and a 180 px search;
 * three columns 24 apart of 26 px rows (2 apart) filled column by column, the body as tall as the
 * tallest group needs (at most 9 rows) and scrolling beyond. A row: its label, up to two keycaps
 * with "or" between. A query searches every group.
 */
import { useEffect, useMemo, useState } from 'react'
import { useDesignKit } from '../kit/hooks'
import { GROUPS, useKeymap } from '../menus/keymap'
import { Dialog } from './Dialog'

type Row = { label: string; keys: string[] }

const ROW_PITCH = 28
const MAX_ROWS = 9

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  const { t, strings } = useDesignKit()
  const keymap = useKeymap()
  const [group, setGroup] = useState(GROUPS[0])
  const [query, setQuery] = useState('')
  const [width, setWidth] = useState(() => Math.max(480, Math.min(window.innerWidth - 48, 1040)))
  useEffect(() => {
    const fit = () => setWidth(Math.max(480, Math.min(window.innerWidth - 48, 1040)))
    window.addEventListener('resize', fit)
    return () => window.removeEventListener('resize', fit)
  }, [])

  // Rows per group: a binding's label (its menu label when it has one), its keys once each.
  const byGroup = useMemo(() => {
    const out = new Map<string, Array<Row & { english: string }>>()
    for (const name of GROUPS) out.set(name, [])
    for (const binding of keymap.bindings) {
      const keys = [...new Set(binding.shown)].slice(0, 2)
      const label = strings.menuLabel(binding.id, binding.title)
      out.get(binding.group)?.push({ label, english: binding.title, keys })
    }
    for (const gesture of keymap.gestures) out.get(gesture.group)?.push({ label: t(gesture.label), english: gesture.label, keys: [gesture.keys] })
    return out
  }, [keymap, strings, t])

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return byGroup.get(group) ?? []
    return [...byGroup.values()].flat().filter((row) => row.label.toLowerCase().includes(q) || row.english.toLowerCase().includes(q))
  }, [byGroup, group, query])

  // The body keeps one height whatever the tab: the tallest group's rows per column, at most 9.
  const tallest = Math.max(1, ...[...byGroup.values()].map((list) => Math.ceil(list.length / 3)))
  const height = ROW_PITCH * Math.min(tallest, MAX_ROWS)
  const perColumn = Math.max(1, Math.ceil(rows.length / 3))
  const columns = [rows.slice(0, perColumn), rows.slice(perColumn, perColumn * 2), rows.slice(perColumn * 2)]

  return (
    <Dialog title={t('Keyboard shortcuts')} width={width} placement={{ kind: 'bottom', margin: 16 }} onClose={onClose} testId="design-dialog-shortcuts" bodyStyle={{ gap: 0 }}>
      <div className="design-shortcuts-head">
        <div className="design-tabstrip" role="tablist">
          {GROUPS.map((name) => (
            <button
              key={name}
              type="button"
              role="tab"
              aria-selected={!query && group === name}
              onClick={() => {
                setGroup(name)
                setQuery('')
              }}
              data-testid={`design-shortcuts-tab-${name.toLowerCase()}`}
            >
              {t(name)}
            </button>
          ))}
        </div>
        <div className="design-field design-field-text" style={{ width: 180, flex: 'none' }}>
          <input value={query} placeholder={t('Search shortcuts')} aria-label={t('Search shortcuts')} spellCheck={false} autoComplete="off" onChange={(event) => setQuery(event.target.value)} data-testid="design-shortcuts-search" />
        </div>
      </div>
      <div className="design-shortcuts-body" style={{ height, marginTop: 16 }} data-testid="design-shortcuts-body">
        {rows.length === 0 ? (
          <div className="design-shortcuts-empty">{t('No shortcuts match')}</div>
        ) : (
          columns.map((column, index) => (
            <div key={index} className="design-shortcuts-col">
              {column.map((row, at) => (
                <div key={`${row.label}:${at}`} className="design-shortcut-row" data-tip={row.label} data-testid="design-shortcut-row">
                  <span className="design-shortcut-label">{row.label}</span>
                  {row.keys.map((key, k) => (
                    <span key={key} style={{ display: 'contents' }}>
                      {k > 0 ? <span className="design-keycap-or">{t('or')}</span> : null}
                      <kbd className="design-keycap">{key}</kbd>
                    </span>
                  ))}
                </div>
              ))}
            </div>
          ))
        )}
      </div>
    </Dialog>
  )
}
