/**
 * The Actions palette (2.7 §2, ⌘K or ⌘/, the main menu's "Actions…", the toolbar's lightning):
 * a light panel as wide as the toolbar pill, 8 px above it, 450 px of content whatever the tab. A
 * search field (always focused), the tabs All and Assets (upstream's "Plugins & widgets" is
 * dropped with the plug-ins), and sections of 34 px rows: Recents and one section per top menu
 * while the search is empty; with a query, menu items and every engine command, best first (label
 * starting with the query, then shorter), at most 60, grouped by menu ("Other" for the rest).
 * Up/Down move the highlight, Enter runs it, Escape or a press outside closes.
 */
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useDesignKit, useQuery } from '../kit/hooks'
import { useKeyboardReturn } from '../kit/focus'
import type { DesignKit } from '../kit/context'
import { TOP_MENUS } from './catalog'
import { invoke, recentActions, recordRecent } from './invoke'
import { rowId, useMenuModel } from './model'
import './menus.css'

type Tab = 'all' | 'assets'
type Action = { key: string; label: string; hint: string }
type Section = { name: string; rows: Action[] }
type Component = { id: number; name: string; label?: string }
type Style = { id: string; name: string }

const MAX_HITS = 60
const CONTENT_HEIGHT = 450

function viewCentre(designKit: DesignKit) {
  const view = designKit.canvas.current?.view()
  const area = designKit.inspect.area
  return view ? { x: area.x + view.x + view.width / view.zoom / 2, y: area.y + view.y + view.height / view.zoom / 2 } : { x: 0, y: 0 }
}

/** Runs a palette row (upstream `run_action`). */
function runAction(designKit: DesignKit, key: string) {
  const [kind, value] = [key.slice(0, key.indexOf(':')), key.slice(key.indexOf(':') + 1)]
  if (kind === 'command') {
    recordRecent(key)
    invoke(designKit, value)
  } else if (kind === 'component') {
    void designKit.run('instance.place', { component: Number(value), ...viewCentre(designKit) })
  } else if (kind === 'style') {
    if (!designKit.inspect.selection.length) designKit.toast(designKit.t('Select layers to apply the style to'))
    else void designKit.run('style.apply', { style: value })
  }
}

export function ActionsPalette({ anchor, onClose }: { anchor: Element | null; onClose: () => void }) {
  const designKit = useDesignKit()
  const { t, editor, strings } = designKit
  const { rows, shown } = useMenuModel()
  const [query, setQuery] = useState('')
  const [tab, setTab] = useState<Tab>('all')
  const [hi, setHi] = useState(0)
  const panel = useRef<HTMLDivElement>(null)
  // Closed (Escape, a press outside, a run action), the keyboard is the canvas's again.
  useKeyboardReturn()
  const list = useRef<HTMLDivElement>(null)
  const input = useRef<HTMLInputElement>(null)
  const [at, setAt] = useState<{ left: number; top: number; width: number } | null>(null)
  const components = useQuery<Component[]>('components', {}, { skip: tab !== 'assets' })
  const styles = useQuery<Style[]>('styles', {}, { skip: tab !== 'assets' })

  useLayoutEffect(() => {
    const place = () => {
      const rect = anchor?.getBoundingClientRect()
      if (!rect) return
      // egui draws the frame's 1 px stroke outside the pill's width and the fixed height.
      setAt({ left: rect.left, top: rect.top - 8 - (CONTENT_HEIGHT + 16), width: rect.width + 2 })
    }
    place()
    window.addEventListener('resize', place)
    return () => window.removeEventListener('resize', place)
  }, [anchor])

  // Every top-menu item, by upstream id, with its label and top menu.
  const items = useMemo(() => {
    const out = new Map<string, { label: string; menu: string }>()
    for (const row of rows) if (row.command !== '---' && !out.has(rowId(row))) out.set(rowId(row), { label: row.label, menu: row.path[0] })
    return out
  }, [rows])

  const sections = useMemo<Section[]>(() => {
    const q = query.trim().toLowerCase()
    const matches = (label: string, extra: string) => !q || label.toLowerCase().includes(q) || extra.toLowerCase().includes(q)
    const row = (id: string, label: string): Action => ({ key: `command:${id}`, label, hint: shown.get(id) ?? '' })
    if (tab === 'assets') {
      const out: Section[] = []
      const comps = (components ?? []).filter((c) => matches(c.name, '')).map((c) => ({ key: `component:${c.id}`, label: c.name, hint: t('Component') }))
      const own = (styles ?? []).filter((s) => matches(s.name, '')).map((s) => ({ key: `style:${s.id}`, label: s.name, hint: t('Style') }))
      if (comps.length) out.push({ name: t('Components'), rows: comps })
      if (own.length) out.push({ name: t('Styles'), rows: own })
      return out
    }
    const hits: Array<{ menu: string | null; row: Action }> = []
    const seen = new Set<string>()
    for (const [id, item] of items) {
      seen.add(id)
      if (matches(item.label, id)) hits.push({ menu: item.menu, row: row(id, item.label) })
    }
    if (q) {
      for (const command of editor.commands) {
        if (seen.has(command.id) || command.id.startsWith('tool.') || !editor.enabled(command.id)) continue
        seen.add(command.id)
        const label = strings.command(command.id, command.title)
        if (matches(label, `${command.title} ${command.id}`)) hits.push({ menu: null, row: row(command.id, label) })
      }
      hits.sort((a, b) => Number(!a.row.label.toLowerCase().startsWith(q)) - Number(!b.row.label.toLowerCase().startsWith(q)) || [...a.row.label].length - [...b.row.label].length)
      hits.splice(MAX_HITS)
    }
    const out: Section[] = []
    if (!q) {
      const recents = recentActions()
        .map((key) => {
          const id = key.slice('command:'.length)
          const item = items.get(id)
          const label = item?.label ?? (editor.byId.get(id) ? strings.command(id, editor.byId.get(id)!.title) : null)
          return key.startsWith('command:') && label ? row(id, label) : null
        })
        .filter((entry): entry is Action => entry !== null)
      if (recents.length) out.push({ name: t('Recents'), rows: recents })
    }
    const order = (menu: string | null) => (menu ? (TOP_MENUS as readonly string[]).indexOf(menu) : TOP_MENUS.length)
    const groups: Section[] = []
    for (const hit of hits) {
      const name = hit.menu ? t(hit.menu) : t('Other')
      const group = groups.find((g) => g.name === name)
      if (group) group.rows.push(hit.row)
      else groups.push({ name, rows: [hit.row] })
    }
    // Without a query the menus keep their order; with one each group sits where its best row is.
    if (!q) groups.sort((a, b) => order(hits.find((h) => (h.menu ? t(h.menu) : t('Other')) === a.name)?.menu ?? null) - order(hits.find((h) => (h.menu ? t(h.menu) : t('Other')) === b.name)?.menu ?? null))
    return [...out, ...groups]
  }, [components, editor, items, query, shown, strings, styles, t, tab])

  const flat = useMemo(() => sections.flatMap((section) => section.rows), [sections])
  const highlight = Math.max(0, Math.min(hi, flat.length - 1))

  useEffect(() => {
    input.current?.focus()
    input.current?.select()
  }, [])

  // A press outside closes (not on the toolbar's Actions button, which toggles it).
  useEffect(() => {
    const down = (event: PointerEvent) => {
      const target = event.target
      if (!(target instanceof Node) || panel.current?.contains(target)) return
      if (target instanceof Element && target.closest('[data-testid="design-actions"], [data-testid="design-main-menu-actions"]')) return
      onClose()
    }
    document.addEventListener('pointerdown', down, true)
    return () => document.removeEventListener('pointerdown', down, true)
  }, [onClose])

  const run = (action: Action | undefined) => {
    if (!action) return
    onClose()
    runAction(designKit, action.key)
  }

  const scrollTo = (index: number) => {
    const el = list.current?.querySelector<HTMLElement>(`[data-index='${index}']`)
    el?.scrollIntoView({ block: 'nearest' })
  }

  let index = -1
  return createPortal(
    <div
      ref={panel}
      className="design-palette"
      role="dialog"
      aria-label={t('Actions')}
      data-testid="design-actions-palette"
      style={at ? { left: at.left, top: at.top, width: at.width } : { left: -9999, top: -9999 }}
      onKeyDown={(event) => {
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          const next = Math.max(0, Math.min(flat.length - 1, highlight + (event.key === 'ArrowDown' ? 1 : -1)))
          setHi(next)
          scrollTo(next)
        } else if (event.key === 'Enter') run(flat[highlight])
        else if (event.key === 'Escape') onClose()
        else return
        event.preventDefault()
        event.stopPropagation()
      }}
    >
      <input
        ref={input}
        className="design-palette-search"
        value={query}
        placeholder={t('Search actions, layers and tools')}
        spellCheck={false}
        autoComplete="off"
        aria-label={t('Search actions, layers and tools')}
        data-testid="design-palette-search"
        onChange={(event) => {
          setQuery(event.target.value)
          setHi(0)
        }}
      />
      <div className="design-palette-tabs" role="tablist">
        {(['all', 'assets'] as const).map((name) => (
          <button
            key={name}
            type="button"
            role="tab"
            className="design-palette-tab"
            aria-selected={tab === name}
            data-testid={`design-palette-tab-${name}`}
            onClick={() => {
              setTab(name)
              setHi(0)
              input.current?.focus()
            }}
          >
            {t(name === 'all' ? 'All' : 'Assets')}
          </button>
        ))}
      </div>
      <div ref={list} className="design-palette-list" role="listbox" data-testid="design-palette-list">
        {sections.map((section) => (
          <div key={section.name} role="group" aria-label={section.name}>
            <div className="design-palette-section">{section.name}</div>
            {section.rows.map((action) => {
              index += 1
              const mine = index
              return (
                <div
                  key={`${section.name}:${action.key}`}
                  className="design-palette-row"
                  role="option"
                  aria-selected={mine === highlight}
                  data-active={mine === highlight || undefined}
                  data-index={mine}
                  data-key={action.key}
                  data-testid={`design-palette-row-${action.key}`}
                  onPointerMove={() => mine !== highlight && setHi(mine)}
                  onClick={() => run(action)}
                >
                  <span className="design-palette-label">{action.label}</span>
                  {action.hint ? <span className="design-palette-hint">{action.hint}</span> : null}
                </div>
              )
            })}
          </div>
        ))}
      </div>
    </div>,
    document.body,
  )
}
