/**
 * Running a menu, shortcut or palette item (upstream `menus::invoke`): items that need the editor
 * (dialogs, panels, the view, the clipboard) are handled here; the rest go to the engine through
 * the editor's `run`. The main menu, kit-editor's top bar, the Actions palette, the context menus
 * and the keys all come through here, so they behave the same.
 *
 * `interceptCommand` is also called by the editor's `run` (App.tsx) for an engine command asked
 * without values, so the top bar's items and kit-editor's shortcuts open the same dialogs.
 */
import type { DesignKit } from '../kit/context'
import { dispatchUiRequests } from '../kit/requests'
import { openDialog, toggleDialog } from '../dialogs/store'
import { openVersionHistory } from '../dev/store'
import { showToast } from '../ui/Toast'
import { TOGGLES, kitId } from './catalog'
import { openContextMenu } from './contextMenuStore'
import { own } from './strings'

let current: DesignKit | null = null

/** The editor's state for `interceptCommand` (set by `MenusHost` on every render). */
export function setMenusDesignKit(designKit: DesignKit) {
  current = designKit
}

const RECENTS_KEY = 'kit.design.recentActions'
/** Upstream `MAX_RECENT_ACTIONS`. */
export const MAX_RECENTS = 5

/** The palette's Recents (keys such as `command:edit.copy`), newest first. */
export function recentActions(): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(RECENTS_KEY) ?? '[]') as unknown
    return Array.isArray(value) ? value.filter((key): key is string => typeof key === 'string').slice(0, MAX_RECENTS) : []
  } catch {
    return []
  }
}

/** Remembers an action run from the palette (upstream `record_recent`). */
export function recordRecent(key: string) {
  const list = [key, ...recentActions().filter((other) => other !== key)].slice(0, MAX_RECENTS)
  try {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(list))
  } catch {
    // No storage in this frame: Recents last for the session only.
  }
}

/** Engine commands Dev Mode still runs (upstream `devinspect::dev_mode_allows`, for the edits the catalog marks). */
const DEV_MODE_OWN = ['annotation.', 'dev.', 'devMode.', 'devResource.', 'codeConnect.', 'comment.']

function devModeRefuses(designKit: DesignKit, id: string) {
  if (designKit.ui.rightTab !== 'inspect') return false
  const command = designKit.editor.byId.get(id)
  if (!command?.mutates || id === 'edit.undo' || id === 'edit.redo' || DEV_MODE_OWN.some((prefix) => id.startsWith(prefix))) return false
  return refusedInDevMode(designKit)
}

/** In Dev Mode an edit is refused with a note (a paste from the clipboard, a dropped file): answers whether it was. */
export function refusedInDevMode(designKit: DesignKit) {
  if (designKit.ui.rightTab !== 'inspect') return false
  showToast(designKit.t('Dev Mode only inspects: switch to Design (Shift+D) to edit'))
  return true
}

/** Engine commands whose values a dialog asks for (upstream opens its dialog instead of running them). */
const DIALOG_COMMANDS: Record<string, (designKit: DesignKit) => void> = {
  'edit.findReplace': () => openDialog('findReplace'),
  'object.batchRename': (designKit) => {
    if (designKit.inspect.selection.length) openDialog('batchRename', { ids: designKit.inspect.selection })
  },
  'text.link': () => openDialog('link'),
  'vector.offset': () => openDialog('offset'),
  'object.moveToPage': () => openDialog('moveToPage'),
  'file.export': () => openDialog('export'),
  'file.exportSvg': () => openDialog('export'),
  'file.exportFramesPdf': (designKit) => void exportFramesPdf(designKit),
  // File ▸ Save to version history (⌥⌘S): the version panel's save form, as upstream.
  'file.saveVersion': () => openVersionHistory(true),
  'file.import': (designKit) => void importFile(designKit, false),
  'file.placeImage': (designKit) => void importFile(designKit, true),
  // Values kept in Preferences: the palette and menus open it rather than reset them.
  'prefs.nudge': () => openDialog('preferences'),
}

/**
 * For the editor's `run`: handles engine command `id` asked without values when the editor shows
 * a dialog for it (or Dev Mode refuses it). Answers whether it was handled.
 */
export function interceptCommand(id: string, params: Record<string, unknown> | undefined): boolean {
  const designKit = current
  if (!designKit || (params && Object.keys(params).length)) return false
  // The editor's own commands (Copy as, the panels, zoom…), asked for by id from any menu.
  if (isKitCommand(id) && !designKit.editor.byId.has(id)) {
    runKit(designKit, id)
    return true
  }
  if (devModeRefuses(designKit, id)) return true
  const open = DIALOG_COMMANDS[id]
  if (!open) return false
  open(designKit)
  return true
}

/** Runs a menu row's command: a `kit:` row in the editor, an engine row through the editor's `run`. */
export function runMenuCommand(designKit: DesignKit, command: string) {
  const own = kitId(command)
  if (own) runKit(designKit, own)
  // A toggle changes no document: the panels and check marks ask again.
  else
    void designKit.editor.run(command).then(() => {
      if (TOGGLES.has(command)) void designKit.refresh()
    })
}

/** Whether `id` is one the editor runs itself (upstream ids the engine's catalog leaves out). */
export function isKitCommand(id: string) {
  return id in KIT_ACTIONS
}

const ask = (kind: string, data: Record<string, unknown> = {}) => dispatchUiRequests([{ kind, data }])

const KIT_ACTIONS: Record<string, (designKit: DesignKit) => void> = {
  'edit.copyAsText': (designKit) => void copyAsText(designKit),
  'edit.copyAsCss': (designKit) => void copyAsCss(designKit),
  'edit.copyAsSvg': (designKit) => void copyAsSvg(designKit),
  'edit.copyAsPng': (designKit) => void copyAsPng(designKit),
  'edit.copyLink': (designKit) => {
    const id = designKit.inspect.selection[0]
    if (id === undefined) return
    void writeText(`kition-design://node/${id}`).then((ok) => ok && showToast(designKit.t('Link copied')))
  },
  // View > Show/Hide UI: kit-editor's Focus display.
  'view.toggleUi': (designKit) => designKit.ui.toggleFocus(),
  'view.panelWidths': () => ask('panel.widths'),
  'view.layers': () => ask('panel.layers'),
  'view.assets': () => ask('panel.assets'),
  'view.design': () => ask('panel.design'),
  'view.prototype': () => ask('panel.prototype'),
  'view.designPrototype': () => ask('panel.designPrototype'),
  'view.leftSidebar': () => ask('panel.leftSidebar'),
  'view.zoomIn': (designKit) => void designKit.viewTo('view.zoomIn'),
  'view.zoomOut': (designKit) => void designKit.viewTo('view.zoomOut'),
  'view.zoom100': (designKit) => void designKit.viewTo('view.zoom100'),
  'view.zoom50': (designKit) => void designKit.viewTo('view.zoom50'),
  'view.zoom200': (designKit) => void designKit.viewTo('view.zoom200'),
  'view.zoomFit': (designKit) => void designKit.viewTo('view.zoomFit'),
  'view.zoomSelection': (designKit) => void designKit.viewTo('view.zoomSelection'),
  'view.contextMenu': (designKit) => openContextMenuAtSelection(designKit),
  'app.preferences': () => openDialog('preferences'),
  'app.actions': (designKit) => designKit.ui.setActions(!designKit.ui.actions),
  'libraries.open': () => ask('panel.libraries'),
  // Upstream opens the libraries view with its publish form: the kit's view always shows it.
  'libraries.publish': () => ask('panel.libraries', { publish: true }),
  'help.shortcuts': () => toggleDialog('shortcuts'),
  'proto.present': () => ask('present'),
}

/** Runs one of the editor's own commands (`view.zoomIn`, `help.shortcuts`, …). */
export function runKit(designKit: DesignKit, id: string) {
  KIT_ACTIONS[id]?.(designKit)
}

/** Runs `id` as the palette and keys do: the editor's own command, or the engine's through `run` (dialogs included). */
export function invoke(designKit: DesignKit, id: string) {
  if (isKitCommand(id)) runKit(designKit, id)
  else void designKit.editor.run(id)
}

/** Shift+F10 (upstream `view.contextMenu`): the canvas context menu at the selection's centre, else the canvas's. */
export function openContextMenuAtSelection(designKit: DesignKit) {
  const surface = document.querySelector('[data-testid="kit-canvas-surface"]')?.getBoundingClientRect()
  const view = designKit.canvas.current?.view()
  const bounds = designKit.inspect.selectionBounds
  let x = surface ? surface.left + surface.width / 2 : window.innerWidth / 2
  let y = surface ? surface.top + surface.height / 2 : window.innerHeight / 2
  if (surface && view && bounds) {
    x = surface.left + (bounds.x + bounds.width / 2 - view.x) * view.zoom
    y = surface.top + (bounds.y + bounds.height / 2 - view.y) * view.zoom
  }
  openContextMenu({ clientX: x, clientY: y, source: 'canvas' })
}

// ---------------------------------------------------------------------------------------------
// Clipboard and files

async function writeText(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    // A frame without clipboard access: the fallback copy through a hidden field.
    const field = document.createElement('textarea')
    field.value = text
    field.style.position = 'fixed'
    field.style.opacity = '0'
    document.body.appendChild(field)
    field.select()
    const ok = document.execCommand('copy')
    field.remove()
    return ok
  }
}

export function fromBase64(data: string): Uint8Array {
  const text = atob(data)
  const bytes = new Uint8Array(text.length)
  for (let index = 0; index < text.length; index += 1) bytes[index] = text.charCodeAt(index)
  return bytes
}

export function toBase64(bytes: Uint8Array) {
  let text = ''
  for (let index = 0; index < bytes.length; index += 0x8000) text += String.fromCharCode(...bytes.subarray(index, index + 0x8000))
  return btoa(text)
}

type NodeJson = { id: number; name?: string; type?: string; characters?: string }

async function copyAsText(designKit: DesignKit) {
  const nodes = await Promise.all(designKit.inspect.selection.map((id) => designKit.query<NodeJson>('node', { id }).catch(() => null)))
  const text = nodes.filter((node): node is NodeJson => typeof node?.characters === 'string').map((node) => node.characters as string)
  if (await writeText(text.join('\n'))) showToast(designKit.t('Copied as text'))
}

async function copyAsCss(designKit: DesignKit) {
  const id = designKit.inspect.selection[0]
  if (id === undefined) return
  const answer = await designKit.run<{ code?: string }>('inspect.code', { id, language: 'css' })
  if (typeof answer?.code === 'string' && (await writeText(answer.code))) showToast(designKit.t('Copied as CSS'))
}

async function copyAsSvg(designKit: DesignKit) {
  const ids = designKit.inspect.selection
  if (!ids.length) return
  const answer = await designKit.run<{ data?: string }>('file.export', { ids, format: 'svg' })
  if (typeof answer?.data !== 'string') return
  if (await writeText(new TextDecoder().decode(fromBase64(answer.data)))) showToast(designKit.t('Copied as SVG'))
}

async function copyAsPng(designKit: DesignKit) {
  const ids = designKit.inspect.selection
  if (!ids.length) return
  const answer = await designKit.run<{ data?: string }>('file.export', { ids, format: 'png', scale: 1 })
  if (typeof answer?.data !== 'string') return
  try {
    const blob = new Blob([fromBase64(answer.data) as BlobPart], { type: 'image/png' })
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])
    showToast(designKit.t('Copied as PNG'))
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error))
  }
}

/** Hands exported bytes to the host (a new file next to the document); answers the name it wrote. */
export async function saveExport(designKit: DesignKit, name: string, bytes: Uint8Array) {
  try {
    const path = await designKit.editor.host.exportFile(name, bytes)
    return path.split('/').pop() ?? path
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error))
    return null
  }
}

/** A file name from a layer or file name: no slashes or characters file systems refuse. */
export function fileName(name: string, extension: string) {
  const base = name.replace(/[\\/:*?"<>|]+/g, '-').trim() || 'Untitled'
  return `${base}.${extension}`
}

async function exportFramesPdf(designKit: DesignKit) {
  const answer = await designKit.run<{ data?: string }>('file.exportFramesPdf', {})
  if (typeof answer?.data !== 'string') return
  const written = await saveExport(designKit, fileName(designKit.inspect.name || 'Frames', 'pdf'), fromBase64(answer.data))
  if (written) showToast(designKit.t('Exported {name}', { name: written }))
}

const IMPORT_TYPES = ['kdesign', 'fig', 'json', 'svg', 'sketch', 'png', 'jpg', 'jpeg', 'gif', 'webp']
const IMAGE_TYPES = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg']

/**
 * A file the person picks through the host (its picker, then `host/read`): its name and bytes, or
 * null when they picked none or the host can't pick (a toast says so). The editor has no file
 * field of its own: files go only through the host.
 */
export async function pickFile(designKit: DesignKit, purpose: string, accept: string[]): Promise<{ name: string; bytes: Uint8Array } | null> {
  const { host } = designKit.editor
  if (!host.init.capabilities.includes('pick-file')) {
    showToast(own(designKit, 'import.noPicker'))
    return null
  }
  try {
    const path = await host.pick(purpose, accept)
    if (!path) return null
    const { bytes } = await host.files.read(path)
    return { name: path.split('/').pop() ?? path, bytes: new Uint8Array(bytes) }
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error))
    return null
  }
}

/** File > Import from file… and Place image…: the host's picker, then `file.import` / `file.placeImage` with the bytes. */
async function importFile(designKit: DesignKit, image: boolean) {
  const picked = await pickFile(designKit, image ? 'image' : 'import', image ? IMAGE_TYPES : IMPORT_TYPES)
  if (!picked) return
  const { name } = picked
  try {
    const data = toBase64(picked.bytes)
    const view = designKit.canvas.current?.view()
    // The view's centre, in page coordinates (document pixels plus the area's origin).
    const area = designKit.inspect.area
    const at = view ? { x: area.x + view.x + view.width / view.zoom / 2, y: area.y + view.y + view.height / view.zoom / 2 } : {}
    const answer = await designKit.run<{ warnings?: unknown[]; summary?: Record<string, unknown> }>(image ? 'file.placeImage' : 'file.import', { data, name, ...at })
    if (!answer) return
    if (!image && (answer.summary || answer.warnings?.length)) openDialog('importReport', { name, summary: answer.summary ?? null, warnings: answer.warnings ?? [], how: 'pages' })
  } catch (error) {
    showToast(error instanceof Error ? error.message : String(error))
  }
}
