/**
 * The main menu's model, for the logo's menu and kit-editor's top bar alike: the rows
 * (`catalog.ts`), their translated labels, shortcuts and check marks, and how a row runs.
 */
import { useMemo, useRef } from 'react'
import { catalogTabs, type FrameTab, type MenuRow } from '@kition/sdk/editor'
import { useDesignKit, useQuery } from '../kit/hooks'
import { checkedIds, kitId, menuRows, PRIMARY_ROWS, TOGGLES, VIEW_MENUS, type EngineRow } from './catalog'
import { prettyShortcut } from '../ui/shortcuts'
import { runMenuCommand } from './invoke'
import { useKitShortcuts, useShortcuts } from './keymap'

export type MenuModel = {
  /** Rows in order, labels translated; `shortcut` in kit notation (`Cmd+Shift+Z`). */
  rows: MenuRow[]
  /** Display shortcuts (`⇧⌘Z`) by upstream id. */
  shown: ReadonlyMap<string, string>
  /** Toggles that are on, by upstream id. */
  checked: ReadonlySet<string>
}

/** The upstream id of a row (`kit:view.zoomIn` → `view.zoomIn`). */
export const rowId = (row: MenuRow) => kitId(row.command) ?? row.command

/** The engine's catalog never changes: kept once answered, so a menu opened later draws whole at once. */
let knownRows: EngineRow[] | null = null

export function useMenuModel(): MenuModel {
  const { strings } = useDesignKit()
  const answered = useQuery<EngineRow[]>('menus')
  if (answered) knownRows = answered
  const engine = answered ?? knownRows
  const checkedAnswer = useQuery<{ ids: string[] }>('checked')
  const prefs = useQuery<Record<string, unknown>>('prefs')
  const shown = useShortcuts()
  const keys = useKitShortcuts()
  const rows = useMemo(
    () =>
      menuRows(engine ?? []).map((row) => {
        if (row.command === '---') return row
        const id = kitId(row.command) ?? row.command
        return { ...row, label: strings.menuLabel(id, row.label), shortcut: keys.get(id) ?? null }
      }),
    [engine, keys, strings],
  )
  const checked = useMemo(() => checkedIds(checkedAnswer?.ids, prefs), [checkedAnswer, prefs])
  return { rows, shown, checked }
}

/**
 * kit-editor's top-bar menus from the same model as the main menu: the same rows, shortcuts (in
 * display form) and checks, and the same entry point (`runMenuCommand`), so a dialog opens the same way from both.
 */
export function useMenuTabs(): FrameTab[] {
  const designKit = useDesignKit()
  const latest = useRef(designKit)
  latest.current = designKit
  const { rows, shown, checked } = useMenuModel()
  // The top bar draws a row's shortcut as given: the display form, as the main menu and the
  // shortcuts dialog write it (⌥⌘G on a Mac, Ctrl+Alt+G elsewhere; K2).
  const barRows = useMemo(() => rows.map((row) => (row.command === '---' || !row.shortcut ? row : { ...row, shortcut: shown.get(rowId(row)) ?? prettyShortcut(row.shortcut) })), [rows, shown])
  const run = useMemo(() => (command: string) => runMenuCommand(latest.current, command), [])
  const isChecked = useMemo(
    () => (command: string) => {
      const id = kitId(command) ?? command
      return TOGGLES.has(id) ? checked.has(id) : undefined
    },
    [checked],
  )
  return useMemo(() => catalogTabs(barRows, { phrase: designKit.strings.phrase, view: VIEW_MENUS, primary: PRIMARY_ROWS, onCommand: run, onKitCommand: run, checked: isChecked }), [barRows, designKit.strings.phrase, isChecked, run])
}
