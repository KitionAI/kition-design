/**
 * The main menu's tree (2.7 §1), shared by the logo's dark menu and kit-editor's top bar so both
 * list the same items in the same order and run them the same way.
 *
 * Engine rows come from the engine's catalog (`query('menus')`: the upstream main menu without the
 * host's and the upstream app's own items). Upstream's main menu also holds items its editor runs
 * itself (`menus::invoke`): Copy as, the Panels and zoom items, Preferences…, Open libraries,
 * Publish library… (the engine's `libraries.open` and `libraries.publish` only ask for the
 * libraries view, so the kit's catalog leaves them to the editor) and Keyboard shortcuts. The engine's catalog leaves them out, so they are added here as `kit:` rows at their
 * upstream places. Host and dropped items (new, open, save, close, theme, full screen,
 * plug-ins, live sessions, About) stay out.
 */
import type { MenuRow } from '@kition/kit-editor'

/** kit-editor's prefix for a catalog row the kit runs itself. */
export const KIT_COMMAND_PREFIX = 'kit:'

/** The top-level menus, in upstream order (Plugins is dropped: the kit has no plug-ins). */
export const TOP_MENUS = ['File', 'Edit', 'View', 'Object', 'Text', 'Arrange', 'Vector', 'Preferences', 'Libraries', 'Help'] as const

/** Menus a read-only document keeps in the top bar. */
export const VIEW_MENUS = ['View', 'Help']

/** The tab the frame opens on in Single line and Classic. */
export const DEFAULT_MENU = 'Edit'

/**
 * The rows each menu's tab shows in Single line's row (kit-editor's `primary`), the most used first;
 * the rest wait in its groups' popovers. Undo and Redo are left out: the top bar has them.
 */
export const PRIMARY_ROWS: Readonly<Record<string, readonly string[]>> = {
  File: ['file.import', 'file.placeImage', 'file.export', 'file.exportSvg', 'file.versionHistory'],
  Edit: ['edit.cut', 'edit.copy', 'edit.paste', 'edit.duplicate', 'edit.delete', 'edit.findReplace', 'select.all'],
  View: ['view.rulers', 'view.pixelGrid', 'view.layoutGrids', 'kit:view.zoomIn', 'kit:view.zoomOut', 'kit:view.zoomFit', 'kit:view.zoomSelection'],
  Object: ['object.frameSelection', 'object.group', 'object.ungroup', 'layout.add', 'component.create', 'object.mask', 'arrange.front', 'arrange.back', 'object.flipH', 'object.flipV'],
  Text: ['text.bold', 'text.italic', 'text.underline', 'text.strikethrough', 'text.link', 'text.bulletList', 'text.numberList'],
  Arrange: ['align.left', 'align.hcenter', 'align.right', 'align.top', 'align.vcenter', 'align.bottom', 'distribute.horizontal', 'distribute.vertical', 'arrange.tidy'],
  Vector: ['vector.join', 'vector.smoothJoin', 'vector.deleteHeal', 'vector.simplify', 'vector.reverse', 'vector.offset'],
  Preferences: ['view.snapGeometry', 'view.snapObjects', 'view.snapPixel', 'kit:app.preferences'],
  Libraries: ['kit:libraries.open', 'kit:libraries.publish'],
  Help: ['kit:help.shortcuts'],
}

/** An engine catalog row, as `query('menus')` answers it. */
export type EngineRow = { path: string[]; label: string; command: string }

/** A row the editor runs itself; `id` is upstream's command id. */
type KitRow = { path: string[]; label: string; id: string }

const kit = (path: string, label: string, id: string): KitRow => ({ path: path.split('/'), label, id })

/**
 * Where each group of the editor's own rows goes: before an engine row (`before`), or after the
 * first separator following one (`afterSeparatorOf`), or at the end of its menu.
 */
const KIT_ROWS: Array<{ rows: KitRow[]; before?: string; afterSeparatorOf?: string; menu?: string }> = [
  {
    // Edit ▸ Copy as ▸ sits after the first separator, before Cut.
    before: 'edit.cut',
    rows: [
      kit('Edit/Copy as', 'Copy as text', 'edit.copyAsText'),
      kit('Edit/Copy as', 'Copy as CSS', 'edit.copyAsCss'),
      kit('Edit/Copy as', 'Copy as SVG', 'edit.copyAsSvg'),
      kit('Edit/Copy as', 'Copy as PNG', 'edit.copyAsPng'),
      kit('Edit/Copy as', 'Copy link to selection', 'edit.copyLink'),
    ],
  },
  {
    // View ▸ Panels ▸ after the separator that follows Soft-proof sRGB (Full screen is the host's).
    afterSeparatorOf: 'view.proofSrgb',
    rows: [
      kit('View/Panels', 'Show/Hide UI', 'view.toggleUi'),
      kit('View/Panels', 'Reset panel widths', 'view.panelWidths'),
      kit('View/Panels', 'Layers', 'view.layers'),
      kit('View/Panels', 'Assets', 'view.assets'),
      kit('View/Panels', 'Design', 'view.design'),
      kit('View/Panels', 'Prototype', 'view.prototype'),
    ],
  },
  {
    before: 'view.prevFrame',
    rows: [
      kit('View', 'Zoom in', 'view.zoomIn'),
      kit('View', 'Zoom out', 'view.zoomOut'),
      kit('View', 'Zoom to 100%', 'view.zoom100'),
      kit('View', 'Zoom to 50%', 'view.zoom50'),
      kit('View', 'Zoom to 200%', 'view.zoom200'),
      kit('View', 'Zoom to fit', 'view.zoomFit'),
      kit('View', 'Zoom to selection', 'view.zoomSelection'),
    ],
  },
  { menu: 'Preferences', rows: [kit('Preferences', 'Preferences…', 'app.preferences')] },
  { before: 'component.hideWhenPublishing', rows: [kit('Libraries', 'Open libraries', 'libraries.open'), kit('Libraries', 'Publish library…', 'libraries.publish')] },
  { menu: 'Help', rows: [kit('Help', 'Keyboard shortcuts', 'help.shortcuts')] },
]

/** The editor's own commands (upstream ids the engine's catalog leaves to the editor). */
export const KIT_IDS: ReadonlySet<string> = new Set(KIT_ROWS.flatMap((group) => group.rows.map((row) => row.id)))

/** A `kit:` row's upstream id (`kit:view.zoomIn` → `view.zoomIn`), or null for an engine row. */
export function kitId(command: string): string | null {
  return command.startsWith(KIT_COMMAND_PREFIX) ? command.slice(KIT_COMMAND_PREFIX.length) : null
}

/** The whole tree as catalog rows, in order: engine rows with the editor's own rows at their upstream places. */
export function menuRows(engine: readonly EngineRow[]): MenuRow[] {
  const out: MenuRow[] = []
  const asRow = (row: KitRow): MenuRow => ({ path: row.path, label: row.label, command: `${KIT_COMMAND_PREFIX}${row.id}` })
  const pending = [...KIT_ROWS]
  const take = (test: (group: (typeof KIT_ROWS)[number]) => boolean) => {
    for (let i = 0; i < pending.length; i += 1) {
      if (test(pending[i])) {
        out.push(...pending[i].rows.map(asRow))
        pending.splice(i, 1)
        i -= 1
      }
    }
  }
  let separatorOf: string | null = null
  for (let index = 0; index < engine.length; index += 1) {
    const row = engine[index]
    take((group) => group.before === row.command)
    out.push({ path: row.path, label: row.label, command: row.command })
    if (row.command === '---' && separatorOf) {
      const of = separatorOf
      separatorOf = null
      take((group) => group.afterSeparatorOf === of)
    } else if (row.command !== '---' && pending.some((group) => group.afterSeparatorOf === row.command)) separatorOf = row.command
    // The end of a top menu: its trailing rows.
    const next = engine[index + 1]
    if (!next || next.path[0] !== row.path[0]) take((group) => group.menu === row.path[0])
  }
  // Menus the engine lists nothing in (Help), and anchors it no longer has.
  for (const group of pending) out.push(...group.rows.map(asRow))
  // Top menus in upstream order.
  const order = (row: MenuRow) => {
    const at = (TOP_MENUS as readonly string[]).indexOf(row.path[0])
    return at < 0 ? TOP_MENUS.length : at
  }
  return out.map((row, index) => ({ row, index })).sort((a, b) => order(a.row) - order(b.row) || a.index - b.index).map(({ row }) => row)
}

/** The engine's `checked` ids plus the Preferences toggles that are on (upstream checks only the View ones; the port checks prefs too). */
export function checkedIds(checked: readonly string[] | undefined, prefs: Record<string, unknown> | null): Set<string> {
  const out = new Set(checked ?? [])
  const pref: Record<string, string> = {
    'pref.keepTool': 'keepTool',
    'pref.highlightHover': 'highlightOnHover',
    'pref.renameDuplicates': 'renameDuplicates',
    'pref.showDimensions': 'showDimensions',
    'pref.numberKeysOpacity': 'numberKeysOpacity',
  }
  for (const [id, key] of Object.entries(pref)) if (prefs?.[key] === true) out.add(id)
  return out
}

/** Ids that toggle: they show a check mark when on (View toggles, snapping, Preferences toggles). */
export const TOGGLES: ReadonlySet<string> = new Set([
  'view.pixelGrid',
  'view.layoutGrids',
  'view.rulers',
  'view.slices',
  'view.comments',
  'view.outlines',
  'view.outlinesSelection',
  'view.outlinesHiddenLayers',
  'view.outlinesObjectBounds',
  'view.pixelPreview',
  'view.maskOutlines',
  'view.frameOutlines',
  'view.proofSrgb',
  'view.snapGeometry',
  'view.snapObjects',
  'view.snapPixel',
  'pref.keepTool',
  'pref.highlightHover',
  'pref.renameDuplicates',
  'pref.showDimensions',
  'pref.numberKeysOpacity',
])
