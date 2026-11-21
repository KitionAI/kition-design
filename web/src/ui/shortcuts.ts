/** Shortcuts as the upstream editor shows them (`keymap::display`): ⌃⌥⇧⌘ then the key on macOS, `Ctrl+Shift+K` elsewhere. */
import { isMac } from '@kition/sdk/editor'

const GLYPHS: Record<string, string> = {
  Enter: '↵',
  Return: '↵',
  Backspace: '⌫',
  Delete: '⌫',
  Tab: '⇥',
  PageUp: '⇞',
  PageDown: '⇟',
  Escape: 'Esc',
  ArrowLeft: '←',
  ArrowRight: '→',
  ArrowUp: '↑',
  ArrowDown: '↓',
  Left: '←',
  Right: '→',
  Up: '↑',
  Down: '↓',
  '=': '+',
}

/** `Cmd+Shift+Z` (kit notation) → `⇧⌘Z` on macOS, `Ctrl+Shift+Z` elsewhere. */
export function prettyShortcut(shortcut: string | null | undefined, mac = isMac()): string {
  if (!shortcut) return ''
  const parts = shortcut.split('+').filter(Boolean)
  const key = parts.pop() ?? ''
  const mods = new Set(parts.map((part) => part.toLowerCase()))
  if (mac) {
    const glyph = GLYPHS[key] ?? (key.length === 1 ? key.toUpperCase() : key)
    return `${mods.has('ctrl') ? '⌃' : ''}${mods.has('alt') ? '⌥' : ''}${mods.has('shift') ? '⇧' : ''}${mods.has('cmd') ? '⌘' : ''}${glyph}`
  }
  const names = [mods.has('cmd') || mods.has('ctrl') ? 'Ctrl' : '', mods.has('alt') ? 'Alt' : '', mods.has('shift') ? 'Shift' : ''].filter(Boolean)
  return [...names, key.length === 1 ? key.toUpperCase() : key].join('+')
}

/** A tooltip with its shortcut: `"Label  ⇧L"` (two spaces, as upstream). */
export function tip(label: string, shortcut?: string | null) {
  const keys = prettyShortcut(shortcut)
  return keys ? `${label}  ${keys}` : label
}
