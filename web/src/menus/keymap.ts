/**
 * The keymap (2.7 §3): the engine's bindings (`query('keymap')`) plus the editor's own (the upstream
 * bindings of commands the engine's catalog leaves to the editor: Actions, Keyboard shortcuts,
 * the panels, zoom, the context menu), for the menus' shortcut column, the shortcuts dialog and
 * the key dispatcher.
 *
 * Who runs a key: kit-editor's frame binds each catalog command's first shortcut when it has a
 * modifier (or is Delete, Page Up/Down, an F key), the engine's tools take Escape, Enter, Tab,
 * Delete and the arrows, the tools' letters choose tools, and the shell takes the zoom and panel
 * keys. `menuKeyDown` runs the rest: the second keys of a binding (Cmd+E flatten, F2 rename, Home,
 * End, `\`), plain keys the frame leaves (I pick color), and the editor's own commands. It runs in
 * the capture phase so a key is never run twice (Cmd+Delete must ungroup, not delete).
 */
import { useMemo } from 'react'
import { isMac, shortcutMatches } from '@kition/kit-editor'
import type { DesignKit } from '../kit/context'
import { useQuery } from '../kit/hooks'
import { prettyShortcut } from '../ui/shortcuts'
import { invoke, runKit } from './invoke'

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
  { group: 'View', id: 'view.leftSidebar', title: 'Show/Hide left sidebar', mac: ['Cmd+Shift+\\'], before: 'layers.collapseAll' },
  { group: 'View', id: 'view.nextRegion', title: 'Focus the next region (toolbar, layers, canvas, properties)', mac: ['F6'], other: ['F6', 'Ctrl+F6'], context: 'both', before: 'layers.collapseAll' },
  { group: 'View', id: 'view.previousRegion', title: 'Focus the previous region', mac: ['Shift+F6'], context: 'both', before: 'layers.collapseAll' },
  { group: 'View', id: 'view.contextMenu', title: 'Context menu of the selection (the focused layer row or the canvas)', mac: ['Shift+F10'], before: 'layers.collapseAll' },
  { group: 'Zoom', id: 'view.zoomIn', title: 'Zoom in', mac: ['Cmd+=', 'Shift+=', 'Cmd+Shift+=', '='], before: 'view.prevFrame' },
  { group: 'Zoom', id: 'view.zoomOut', title: 'Zoom out', mac: ['Cmd+-', '-', 'Shift+-', 'Cmd+Shift+-'], before: 'view.prevFrame' },
  { group: 'Zoom', id: 'view.zoom100', title: 'Zoom to 100%', mac: ['Shift+0', 'Cmd+0'], before: 'view.prevFrame' },
  { group: 'Zoom', id: 'view.zoomFit', title: 'Zoom to fit', mac: ['Shift+1'], before: 'view.prevFrame' },
  { group: 'Zoom', id: 'view.zoomSelection', title: 'Zoom to selection', mac: ['Shift+2'], before: 'view.prevFrame' },
  { group: 'Edit', id: 'edit.copyAsPng', title: 'Copy as PNG', mac: ['Cmd+Shift+C'], before: 'file.export' },
  { group: 'Components', id: 'view.assets', title: 'Insert a component (Assets)', mac: ['Shift+I'] },
  { group: 'Prototype', id: 'proto.present', title: 'Present', mac: ['Cmd+Alt+Enter'], before: 'proto.preview' },
]

/** The groups, in the shortcuts dialog's order. */
export const GROUPS = ['Essential', 'Tools', 'View', 'Zoom', 'Text', 'Shape', 'Selection', 'Edit', 'Transform', 'Arrange', 'Components', 'Prototype']

/** The engine's bindings with the editor's own merged in at their upstream places. */
export function mergeKeymap(engine: Keymap | null, mac = isMac()): Keymap {
  const bindings = [...(engine?.bindings ?? [])]
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
  const answered = useQuery<Keymap>('keymap', { platform: mac ? 'mac' : 'other' })
  // The keymap is static: kept once answered, so menus opened later show their keys at once.
  if (answered) knownKeymap = answered
  const engine = answered ?? knownKeymap
  return useMemo(() => mergeKeymap(engine, mac), [engine, mac])
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
  { keys: ['Cmd+Delete', 'Cmd+Backspace'], context: 'canvas', run: command('object.ungroup') },
  { keys: ['F2'], context: 'canvas', run: command('object.rename') },
  { keys: ['F8'], context: 'canvas', run: command('select.area') },
  { keys: ['Cmd+Shift+C'], context: 'canvas', run: own('edit.copyAsPng') },
  { keys: ['Cmd+Alt+Enter'], context: 'canvas', run: own('proto.present') },
]

/** A field the person types in (not kit-editor's text sink, which carries the canvas's text editing). */
function isField(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  if (target.classList.contains('kui-text-sink')) return false
  return target.isContentEditable || target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement
}

/** Matches `spec` against `event`; `?` is Shift+/ on a US layout (a `?` key without Shift elsewhere counts too). */
function matches(spec: string, event: KeyboardEvent, mac: boolean) {
  if (spec.endsWith('+?') || spec === '?') {
    const mods = spec.slice(0, -1)
    const want = { cmd: mods.includes('Cmd'), ctrl: mods.includes('Ctrl'), alt: mods.includes('Alt') }
    const command = mac ? event.metaKey : event.ctrlKey
    const control = mac ? event.ctrlKey : false
    return (event.key === '?' || (event.code === 'Slash' && event.shiftKey)) && command === want.cmd && control === want.ctrl && event.altKey === want.alt
  }
  if (spec.endsWith('Backspace')) return shortcutMatches(spec.replace('Backspace', 'Delete'), event, mac)
  return shortcutMatches(spec, event, mac, true)
}

/**
 * Runs the key if it is one of this dispatcher's (see the module comment): answers whether it
 * did. Not while typing in a field, in a dialog, or with a menu open.
 */
export function menuKeyDown(designKit: DesignKit, event: KeyboardEvent): boolean {
  if (event.isComposing || event.defaultPrevented || isField(event.target)) return false
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
