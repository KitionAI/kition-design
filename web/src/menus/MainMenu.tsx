/**
 * The main menu (2.7 §1): the dark menu under the logo, "Actions…" on top, then File … Help.
 * Its rows are kit-editor's top-bar menus' (`model.ts`), so both agree: labels from the upstream
 * menu catalog, shortcuts from the keymap, check marks on the View, snapping and Preferences
 * toggles, and every item run through `runMenuCommand`.
 */
import { useMemo } from 'react'
import { useDesignKit } from '../kit/hooks'
import { Menu, type MenuEntry } from '../ui/Menu'
import { prettyShortcut } from '../ui/shortcuts'
import { TOP_MENUS, TOGGLES, kitId } from './catalog'
import { runMenuCommand } from './invoke'
import { rowId, useMenuModel } from './model'
import './menus.css'

export function MainMenu({ anchor, onClose }: { anchor: Element | null; onClose: () => void }) {
  const designKit = useDesignKit()
  const { t, editor, ui } = designKit
  const { rows, shown, checked } = useMenuModel()
  const items = useMemo<MenuEntry[]>(() => {
    const tree = new Map<string, MenuEntry[]>()
    const branch = (path: readonly string[]): MenuEntry[] => {
      const key = path.join('/')
      let entries = tree.get(key)
      if (!entries) {
        entries = []
        tree.set(key, entries)
        // A submenu sits where its first item is (upstream: Edit ▸ Copy as after the first separator).
        if (path.length > 1) branch(path.slice(0, -1)).push({ type: 'submenu', label: t(path[path.length - 1]), items: entries, testId: `design-main-menu-${path.join('-').toLowerCase().replace(/\s+/g, '-')}` })
      }
      return entries
    }
    for (const row of rows) {
      const entries = branch(row.path)
      if (row.command === '---') {
        entries.push({ type: 'separator' })
        continue
      }
      const id = rowId(row)
      const own = kitId(row.command) !== null
      entries.push({
        label: row.label,
        shortcut: shown.get(id) ?? prettyShortcut(row.shortcut),
        checked: TOGGLES.has(id) ? checked.has(id) : undefined,
        disabled: !own && !editor.enabled(row.command),
        onSelect: () => runMenuCommand(designKit, row.command),
        testId: `design-menu-${id}`,
      })
    }
    const top: MenuEntry[] = [{ type: 'search', label: t('Actions…'), shortcut: shown.get('app.actions') ?? prettyShortcut('Cmd+K'), onSelect: () => ui.setActions(true), testId: 'design-main-menu-actions' }]
    for (const name of TOP_MENUS) {
      const entries = tree.get(name)
      if (entries?.length) top.push({ type: 'submenu', label: t(name), items: entries, testId: `design-main-menu-${name.toLowerCase()}` })
    }
    return top
  }, [checked, editor, designKit, rows, shown, t, ui])
  // Upstream opens the root panel at a fixed (8, 48) under the tab strip: here under the shell's top.
  const shell = anchor?.closest('.design-shell')?.getBoundingClientRect()
  const top = (shell?.top ?? 0) + 48
  const left = (shell?.left ?? 0) + 8
  const at = anchor ? { left, top, right: left, bottom: top } : null
  return <Menu tone="dark" shadow items={items} anchor={at} placement="below-start" gap={0} minWidth={256} submenuGap={12} onClose={onClose} label={t('Main menu')} testId="design-main-menu" />
}
