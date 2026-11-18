/**
 * The canvas and layers context menus (2.5 §18, 2.2): the dark menu at the pointer, one model for
 * both. The rows come from the engine's `menu.context` (upstream's entries in upstream's order,
 * with the section, convert, combine and main component substitutions, Remove guide, and each
 * command's availability), so the menu lists only commands that exist; as upstream, an offered
 * command is never greyed (running one with nothing to act on does nothing or says why).
 *
 * On the canvas a right-click first picks a ruler guide under the pointer (which adds Remove
 * guide), else selects what it lands on (the engine's `hitTest`, upstream's `select_target`; Cmd
 * or Ctrl: the deepest layer) unless it is selected, and lists Select layer ▸ (every layer under
 * the point). The Layers panel opens it for the rows it names.
 */
import { useEffect, useMemo, useState } from 'react'
import { useDesignKit } from '../kit/hooks'
import type { DesignKit } from '../kit/context'
import type { DesignChrome } from '../kit/types'
import { Menu, type MenuEntry, type MenuItem } from '../ui/Menu'
import { prettyShortcut } from '../ui/shortcuts'
import { closeContextMenu, useContextMenu, type ContextMenuRequest } from './contextMenuStore'
import { isKitCommand, runKit } from './invoke'
import { useShortcuts } from './keymap'

type HitLayer = { id: number; name: string; type: string; depth: number }
type ContextItem = { separator: true } | { command: string; label: string; shortcut: string | null; enabled: boolean; checked: boolean; ui: boolean; params?: Record<string, unknown>; submenu?: ContextItem[] }
type ContextAnswer = { ids: number[]; items: ContextItem[]; selectLayer: HitLayer[] | null }

/** Upstream's Select layer ▸ lists at most this many, indented 12 px per depth up to 8. */
const MAX_LAYERS = 40
const MAX_INDENT = 8
/** How near (screen pixels) a right-click must land to a ruler guide to pick it. */
const GUIDE_SLOP = 3

/** Where a canvas point is in document pixels. */
function pointAt(designKit: DesignKit, clientX: number, clientY: number) {
  const surface = document.querySelector('[data-testid="kit-canvas-surface"]')?.getBoundingClientRect()
  const view = designKit.canvas.current?.view()
  if (!surface || !view) return null
  return { x: view.x + (clientX - surface.left) / view.zoom, y: view.y + (clientY - surface.top) / view.zoom, zoom: view.zoom }
}

/** The shown ruler guide under a document point, if any (upstream's right-click on a guide). */
async function guideAt(designKit: DesignKit, at: { x: number; y: number; zoom: number }): Promise<DesignChrome['guides'][number] | null> {
  const chrome = JSON.parse(await designKit.invoke<string>('chrome', at.zoom)) as DesignChrome
  if (!chrome.rulers) return null
  const slop = GUIDE_SLOP / at.zoom
  return chrome.guides.find((guide) => (guide.vertical ? Math.abs(guide.a[0] - at.x) : Math.abs(guide.a[1] - at.y)) <= slop) ?? null
}

/** What a right-click on the canvas does before the menu shows: picks a guide, or selects its target. */
async function pick(designKit: DesignKit, at: { x: number; y: number; zoom: number }, deep: boolean) {
  const guide = await guideAt(designKit, at).catch(() => null)
  if (guide) {
    await designKit.run('guide.select', guide.owner == null ? { index: guide.index } : { index: guide.index, frame: guide.owner })
    return
  }
  const hit = await designKit.query<{ target: number | null; selected: boolean }>('hitTest', { x: at.x, y: at.y, deep, zoom: at.zoom }).catch(() => null)
  if (hit?.target != null && !hit.selected) await designKit.run('select.set', { ids: [hit.target] })
}

export function ContextMenuHost() {
  const request = useContextMenu()
  if (!request) return null
  return <ContextMenu key={`${request.clientX},${request.clientY}`} request={request} />
}

function ContextMenu({ request }: { request: ContextMenuRequest }) {
  const designKit = useDesignKit()
  const { t, run } = designKit
  const shortcuts = useShortcuts()
  const canvas = request.source !== 'layers'
  const [answer, setAnswer] = useState<ContextAnswer | null>(null)

  // Once per opening: the pick on the canvas, then the engine's menu for what is selected.
  useEffect(() => {
    let live = true
    const point = canvas ? pointAt(designKit, request.clientX, request.clientY) : null
    void (async () => {
      if (point) await pick(designKit, point, request.deep === true)
      const params: Record<string, unknown> = {}
      if (point) params.at = { x: point.x, y: point.y }
      if (request.ids) params.ids = request.ids
      const menu = await designKit.query<ContextAnswer>('menu.context', params)
      if (live) setAnswer(menu)
    })().catch(() => live && closeContextMenu())
    return () => {
      live = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const items = useMemo<MenuEntry[] | null>(() => {
    if (!answer) return null
    const entry = (item: ContextItem): MenuEntry => {
      if ('separator' in item) return { type: 'separator' }
      const id = item.command
      if (item.submenu) {
        const sub = item.submenu.map(entry)
        const disabled = sub.every((row) => 'disabled' in row && row.disabled)
        return { type: 'submenu', label: t(item.label), disabled, items: sub, testId: id === 'copyAs' ? 'design-context-copy-as' : `design-context-${id}` }
      }
      // The editor's own commands (Show/Hide UI, Zoom to fit, Copy as and the link, which the
      // engine's catalog leaves to the editor) run here; Copy as needs a selection.
      const own = item.ui || isKitCommand(id)
      const row: MenuItem = {
        label: t(item.label),
        shortcut: shortcuts.get(id) ?? prettyShortcut(item.shortcut ?? undefined),
        checked: item.checked || undefined,
        disabled: item.ui ? false : own ? answer.ids.length === 0 : !item.enabled,
        onSelect: () => (own ? runKit(designKit, id) : void run(id, item.params ?? {})),
        testId: `design-context-${id}`,
      }
      return row
    }
    const out: MenuEntry[] = []
    const under = canvas ? (answer.selectLayer ?? []).slice(0, MAX_LAYERS) : []
    if (under.length) {
      const selected = new Set(answer.ids)
      out.push(
        {
          type: 'submenu',
          label: t('Select layer'),
          testId: 'design-context-select-layer',
          items: under.map((layer) => ({
            label: layer.name || layer.type.toLowerCase(),
            indent: Math.min(layer.depth, MAX_INDENT) * 12,
            checked: selected.has(layer.id),
            onSelect: () => void run('select.set', { ids: [layer.id] }),
            testId: `design-context-layer-${layer.id}`,
          })),
        },
        { type: 'separator' },
      )
    }
    out.push(...answer.items.map(entry))
    return out
  }, [answer, canvas, designKit, run, shortcuts, t])

  if (!items) return null
  const at = { left: request.clientX, top: request.clientY, right: request.clientX, bottom: request.clientY }
  return <Menu tone="dark" shadow items={items} anchor={at} placement="below-start" gap={0} minWidth={248} onClose={closeContextMenu} label={t('Context menu')} testId="design-context-menu" />
}
