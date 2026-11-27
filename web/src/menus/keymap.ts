/**
 * The keymap (2.7 §3): the engine's bindings (`query('keymap')`) plus the editor's own (the upstream
 * bindings of commands the engine's catalog leaves to the editor: Actions, Keyboard shortcuts,
 * the panels, zoom, the context menu), for the menus' shortcut column, the shortcuts dialog and
 * the key dispatcher.
 *
 * Who runs a key: kit-editor's frame binds each catalog command's first shortcut when it has a
 * modifier (or is Delete, Page Up/Down, an F key), the engine's tools take Escape, Enter, Tab,
 * Delete and the arrows, the tools' letters choose tools, and the shell takes the zoom and panel
 * keys. `menuKeyDown` runs the keys that must come before the tools (Cmd+Delete must ungroup, not
 * delete; Home and End go to a frame with a selection too), plain keys the frame leaves (I pick
 * color), and the editor's own commands, in the capture phase so a key is never run twice.
 * `keymapKeyDown`, the frame's own key handler (before its command shortcuts), runs every other
 * second or later key of a binding (A for the Frame tool), and off the Mac a first key the frame
 * was not given (it binds the Mac's), so none of them needs a row here.
 */
import { useMemo } from 'react'
import { isMac, shortcutMatches } from '@kition/sdk/editor'
import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import type { DesignKit } from '../kit/context'
import { composingKey } from '../canvas/inputMethod'
import { useInspect, useQuery } from '../kit/hooks'
import { prettyShortcut } from '../ui/shortcuts'
import { invoke, isKitCommand, runKit } from './invoke'

export type Binding = { group: string; id: string; title: string; keys: string[]; shown: string[]; context: 'canvas' | 'text' | 'both' }
export type Gesture = { group: string; label: string; keys: string }
export type Keymap = { bindings: Binding[]; gestures: Gesture[] }

type Own = { group: string; id: string; title: string; mac: string[]; other?: string[]; context?: Binding['context']; before?: string; after?: string }

/** The editor's own bindings (upstream `keymap::KEYS` rows whose commands the engine's catalog leaves out), with where they go. */
const OWN: Own[] = [
  { group: 'Essential', id: 'app.actions', title: 'Actions…', mac: ['Cmd+K', 'Cmd+/'], before: 'edit.pickColor' },
  { group: 'Essential', id: 'help.shortcuts', title: 'Keyboard shortcuts', mac: ['Ctrl+Shift+?'], context: 'both', before: 'edit.pickColor' },
  { group: 'Essential', id: 'view.toggleUi', title: 'Show/Hide UI', mac: ['Cmd+\\'], before: 'edit.pickColor' },
  { group: 'View', id: 'view.layers', title: 'Layers panel', mac: ['Alt+1'], before: 'view.devMode' },
  { group: 'View', id: 'view.assets', title: 'Assets panel', mac: ['Alt+2'], before: 'view.devMode' },
  { group: 'View', id: 'libraries.open', title: 'Open libraries', mac: ['Alt+3'], before: 'view.devMode' },
  { group: 'View', id: 'view.design', title: 'Design panel', mac: ['Alt+8'], before: 'view.devMode' },
  { group: 'View', id: 'view.prototype', title: 'Prototype panel', mac: ['Alt+9'], before: 'view.devMode' },
  { group: 'View', id: 'view.designPrototype', title: 'Switch between Design and Prototype', mac: ['Shift+E'], before: 'view.devMode' },
  { group: 'View', id: 'view.leftSidebar', title: 'Show/Hide left sidebar', mac: ['Cmd+Shift+\\'], before: 'layer.collapseAll' },
  { group: 'View', id: 'view.nextRegion', title: 'Focus the next region (toolbar, layers, canvas, properties)', mac: ['F6'], other: ['F6', 'Ctrl+F6'], context: 'both', before: 'layer.collapseAll' },
  { group: 'View', id: 'view.previousRegion', title: 'Focus the previous region', mac: ['Shift+F6'], context: 'both', before: 'layer.collapseAll' },
  { group: 'View', id: 'view.contextMenu', title: 'Context menu of the selection (the focused layer row or the canvas)', mac: ['Shift+F10'], before: 'layer.collapseAll' },
  { group: 'Zoom', id: 'view.zoomIn', title: 'Zoom in', mac: ['Cmd+=', 'Shift+=', 'Cmd+Shift+=', '='], before: 'view.prevFrame' },
  { group: 'Zoom', id: 'view.zoomOut', title: 'Zoom out', mac: ['Cmd+-', '-', 'Shift+-', 'Cmd+Shift+-'], before: 'view.prevFrame' },
  { group: 'Zoom', id: 'view.zoom100', title: 'Zoom to 100%', mac: ['Shift+0', 'Cmd+0'], before: 'view.prevFrame' },
  { group: 'Zoom', id: 'view.zoomFit', title: 'Zoom to fit', mac: ['Shift+1'], before: 'view.prevFrame' },
  { group: 'Zoom', id: 'view.zoomSelection', title: 'Zoom to selection', mac: ['Shift+2'], before: 'view.prevFrame' },
  { group: 'Edit', id: 'edit.copyAsPng', title: 'Copy as PNG', mac: ['Cmd+Shift+C'], before: 'file.export' },
  { group: 'Components', id: 'view.assets', title: 'Insert a component (Assets)', mac: ['Shift+I'] },
  { group: 'Prototype', id: 'proto.present', title: 'Present', mac: ['Cmd+Alt+Enter'], before: 'proto.preview' },
]

/**
 * Engine bindings the editor does not offer: the keyboard marquee (`select.area`, Alt+Space and F8)
 * has no box to draw here, so its keys are neither bound nor listed.
 */
export const NOT_OFFERED: ReadonlySet<string> = new Set(['select.area'])

/** The whiteboard's tools: their keys work on a board only, so a design file neither lists nor offers them. */
export const BOARD_ONLY: ReadonlySet<string> = new Set(['tool.sticky', 'tool.shape', 'tool.connector', 'tool.marker', 'tool.highlighter', 'tool.eraser', 'tool.stamp', 'board.nextSticky'])

/** The groups, in the shortcuts dialog's order. */
export const GROUPS = ['Essential', 'Tools', 'View', 'Zoom', 'Text', 'Shape', 'Selection', 'Edit', 'Transform', 'Arrange', 'Components', 'Prototype']

/** The engine's bindings with the editor's own merged in at their upstream places (for a document of `kind`). */
export function mergeKeymap(engine: Keymap | null, mac = isMac(), kind = 'design'): Keymap {
  const bindings = (engine?.bindings ?? []).filter((binding) => !NOT_OFFERED.has(binding.id) && (kind === 'board' || !BOARD_ONLY.has(binding.id)))
  for (const own of OWN) {
    const keys = mac ? own.mac : (own.other ?? own.mac)
    const binding: Binding = { group: own.group, id: own.id, title: own.title, keys, shown: keys.map((key) => prettyShortcut(key, mac)), context: own.context ?? 'canvas' }
    const before = own.before ? bindings.findIndex((other) => other.id === own.before && other.group === own.group) : -1
    if (before >= 0) {
      // After the editor's own rows already placed before the same anchor.
      bindings.splice(before, 0, binding)
    } else {
      const last = bindings.map((other) => other.group).lastIndexOf(own.group)
      bindings.splice(last >= 0 ? last + 1 : bindings.length, 0, binding)
    }
  }
  return { bindings, gestures: engine?.gestures ?? [] }
}

let knownKeymap: Keymap | null = null

/** The keymap for this platform (engine and editor bindings). */
export function useKeymap(): Keymap {
  const mac = isMac()
  const kind = useInspect().kind
  const answered = useQuery<Keymap>('keymap', { platform: mac ? 'mac' : 'other' })
  // The keymap is static: kept once answered, so menus opened later show their keys at once.
  if (answered) knownKeymap = answered
  const engine = answered ?? knownKeymap
  return useMemo(() => mergeKeymap(engine, mac, kind), [engine, mac, kind])
}

/** Each command's first shortcut, as the menus show it (upstream: the first binding's first key). */
export function useShortcuts(): ReadonlyMap<string, string> {
  const keymap = useKeymap()
  return useMemo(() => {
    const out = new Map<string, string>()
    for (const binding of keymap.bindings) if (!out.has(binding.id) && binding.shown[0]) out.set(binding.id, binding.shown[0])
    return out
  }, [keymap])
}

/** Each command's first shortcut in kit notation (`Cmd+Shift+Z`), for kit-editor's top bar. */
export function useKitShortcuts(): ReadonlyMap<string, string> {
  const keymap = useKeymap()
  return useMemo(() => {
    const out = new Map<string, string>()
    for (const binding of keymap.bindings) if (!out.has(binding.id) && binding.keys[0]) out.set(binding.id, binding.keys[0])
    return out
  }, [keymap])
}

type Key = { keys: string[]; context: Binding['context']; run: (designKit: DesignKit) => void; mac?: boolean }

const command = (id: string) => (designKit: DesignKit) => invoke(designKit, id)
const own = (id: string) => (designKit: DesignKit) => runKit(designKit, id)

/**
 * The keys this dispatcher runs (the rest are the frame's, the tools' and the shell's). Cmd+K is
 * Create link while editing text and Actions elsewhere, as upstream (`text.link` comes first in
 * the text context).
 */
const KEYS: Key[] = [
  { keys: ['Cmd+K'], context: 'text', run: command('text.link') },
  { keys: ['Cmd+K', 'Cmd+/'], context: 'canvas', run: own('app.actions') },
  { keys: ['Ctrl+Shift+?'], context: 'both', run: own('help.shortcuts') },
  { keys: ['Cmd+Shift+\\'], context: 'canvas', run: own('view.leftSidebar') },
  { keys: ['Shift+F10'], context: 'canvas', run: own('view.contextMenu') },
  { keys: ['Alt+3'], context: 'canvas', run: own('libraries.open') },
  { keys: ['=', 'Shift+='], context: 'canvas', run: own('view.zoomIn') },
  { keys: ['-', 'Shift+-'], context: 'canvas', run: own('view.zoomOut') },
  { keys: ['Home'], context: 'canvas', run: (designKit) => void designKit.viewTo('view.prevFrame') },
  { keys: ['End'], context: 'canvas', run: (designKit) => void designKit.viewTo('view.nextFrame') },
  { keys: ['I'], context: 'canvas', run: command('edit.pickColor') },
  { keys: ['Ctrl+C'], context: 'canvas', run: command('edit.pickColor'), mac: true },
  { keys: ["Cmd+'"], context: 'canvas', run: command('view.pixelGrid') },
  { keys: ['Ctrl+G'], context: 'canvas', run: command('view.layoutGrids'), mac: true },
  { keys: ['Cmd+E'], context: 'canvas', run: command('vector.flatten') },
  { keys: ['\\'], context: 'canvas', run: command('select.parent') },
  { keys: ['Cmd+Delete', 'Cmd+Backspace'], context: 'canvas', run: command('arrange.ungroup') },
  { keys: ['F2'], context: 'canvas', run: command('object.rename') },
  { keys: ['Cmd+Shift+C'], context: 'canvas', run: own('edit.copyAsPng') },
  { keys: ['Cmd+Alt+Enter'], context: 'canvas', run: own('proto.present') },
  // Before kit-editor's Space (the hand), which would take it.
  { keys: ['Shift+Space'], context: 'canvas', run: command('proto.preview') },
]

/** A field the person types in (not kit-editor's text sink, which carries the canvas's text editing). */
function isField(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  if (target.classList.contains('kui-text-sink')) return false
  return target.isContentEditable || target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement
}

/**
 * Matches `spec` against `event`; `?` is Shift+/ on a US layout (a `?` key without Shift elsewhere
 * counts too). Off the Mac, Cmd and Ctrl are both the Ctrl key.
 */
function matches(spec: string, event: KeyboardEvent, mac: boolean) {
  // Off the Mac the keymap's Ctrl is the Ctrl key, which kit-editor calls Cmd there.
  if (!mac && /(^|\+)Ctrl\+/.test(spec)) spec = spec.replace(/(^|\+)Ctrl\+/, '$1Cmd+').replace(/Cmd\+(.*)Cmd\+/, 'Cmd+$1')
  if (spec.endsWith('+?') || spec === '?') {
    const mods = spec.slice(0, -1)
    const want = { cmd: mods.includes('Cmd'), ctrl: mods.includes('Ctrl'), alt: mods.includes('Alt') }
    const modifiers = mac ? event.metaKey === want.cmd && event.ctrlKey === want.ctrl : !event.metaKey && event.ctrlKey === (want.cmd || want.ctrl)
    return (event.key === '?' || (event.code === 'Slash' && event.shiftKey)) && modifiers && event.altKey === want.alt
  }
  if (spec.endsWith('Backspace')) return shortcutMatches(spec.replace('Backspace', 'Delete'), event, mac)
  // kit-editor compares a named key with `event.key`, which is " " for Space.
  if (spec === 'Space' || spec.endsWith('+Space')) return shortcutMatches(`${spec.slice(0, -'Space'.length)} `, event, mac, true)
  return shortcutMatches(spec, event, mac, true)
}

/**
 * Runs the key if it is one of this dispatcher's (see the module comment): answers whether it
 * did. Not while typing in a field, in a dialog, or with a menu open.
 */
export function menuKeyDown(designKit: DesignKit, event: KeyboardEvent): boolean {
  if (composingKey(event) || event.defaultPrevented || isField(event.target)) return false
  const target = event.target instanceof Element ? event.target : null
  if (target?.closest('.design-dialog, .kui-dialog, .design-menu, .design-popover')) return false
  const mac = isMac()
  const text = designKit.inspect.editing === 'text' || designKit.tools.wantsText
  for (const key of KEYS) {
    if (key.mac && !mac) continue
    if (key.context === 'text' && !text) continue
    if (key.context === 'canvas' && text) continue
    if (!key.keys.some((spec) => matches(spec, event, mac))) continue
    key.run(designKit)
    return true
  }
  return false
}

/** Keys the tools, the fields and the lists keep as their own: never a second key's command here. */
const EDITING_KEYS = /(^|\+)(Escape|Enter|Tab|Delete|Backspace|Space|ArrowLeft|ArrowRight|ArrowUp|ArrowDown|Home|End|PageUp|PageDown)$/

/** Runs binding `id` as a key does: a tool is chosen, a frame step moves the canvas, a command runs. */
function runBinding(designKit: DesignKit, id: string) {
  const tool = id.startsWith('tool.') ? id.slice(5) : null
  if (tool && designKit.tools.tools.some((item) => item.id === tool)) {
    designKit.tools.select(tool)
    return true
  }
  if (id === 'view.prevFrame' || id === 'view.nextFrame') {
    void designKit.viewTo(id)
    return true
  }
  if (!isKitCommand(id) && !designKit.editor.byId.has(id)) return false
  invoke(designKit, id)
  return true
}

/**
 * The second and later keys of every binding (kit-editor's frame binds a command's first key only,
 * and the tools their one letter): for the frame's `onKeyDown`, which runs after the tools had the
 * key and before the command shortcuts. Answers whether it ran one.
 */
export function keymapKeyDown(designKit: DesignKit, keymap: Keymap, event: ReactKeyboardEvent | KeyboardEvent): boolean {
  const native = 'nativeEvent' in event ? event.nativeEvent : event
  if (composingKey(native) || event.defaultPrevented) return false
  const mac = isMac()
  const text = designKit.inspect.editing === 'text' || designKit.tools.wantsText
  for (const binding of keymap.bindings) {
    if (binding.context === 'text' && !text) continue
    if (binding.context === 'canvas' && text) continue
    // kit-editor's frame binds the Mac's first key (the engine's catalog); off the Mac a binding
    // whose first key differs from it (Alt+Shift+H for Ctrl+Alt+H) is run here too.
    const frameKey = designKit.editor.byId.get(binding.id)?.shortcut
    const from = !mac && frameKey !== undefined && binding.keys[0] !== frameKey ? 0 : 1
    const later = binding.keys.slice(from).filter((spec) => !EDITING_KEYS.test(spec))
    if (!later.some((spec) => matches(spec, native, mac))) continue
    if (runBinding(designKit, binding.id)) return true
  }
  return false
}
