/**
 * Where the keyboard goes when a control lets go of it (plan stage 1, R3). The editor's keys (the
 * tools, the shortcuts, the commands) are kit-editor's frame's (`.kui-app`): they only run while
 * the focus is in it. A field's Enter or Escape, a closed popup, menu, palette, comment composer
 * or window, and a page just opened all used to leave the focus on the page's `<body>`, and then
 * no key worked until the canvas was clicked. Here the focus goes back to the frame instead, and
 * a key that still finds the focus on the body is given to the frame (`keyboardFallback`).
 */
import { useEffect } from 'react'

/** kit-editor's frame, which holds the keyboard when no control in it does. */
export function editorRoot(): HTMLElement | null {
  return document.querySelector<HTMLElement>('.kui-app')
}

/** Whether no element holds the keyboard (the page's body has it). */
export function keyboardLost() {
  const active = document.activeElement
  return !active || active === document.body || active === document.documentElement || !active.isConnected
}

/** Gives the keyboard to the editor's frame; answers whether it has it now. */
export function focusEditor() {
  const root = editorRoot()
  if (!root) return false
  root.focus({ preventScroll: true })
  return document.activeElement === root
}

/**
 * A field let go of the keyboard (Enter or Escape): the editor has it next, as a person expects of
 * a panel field. A field in a dialog only blurs, so the dialog's own keys stay its own.
 */
export function releaseField(field: HTMLElement) {
  if (field.closest('.design-dialog, .kui-dialog, [role="dialog"]') || !focusEditor()) field.blur()
}

/**
 * Once the focus settled after something closed: when nothing has the keyboard, the editor gets
 * it. Later, not at once, so a control the closing opened (a dialog's field) keeps it; never while
 * the page is in the background (the host has the keyboard then).
 */
export function returnKeyboardLater() {
  window.setTimeout(() => {
    if (keyboardLost() && document.hasFocus()) focusEditor()
  }, 0)
}

/** For a popup, menu, palette or window: when it goes, the keyboard goes back to the editor. */
export function useKeyboardReturn() {
  useEffect(() => returnKeyboardLater, [])
}

/**
 * The safety net, for a document `keydown` listener in the capture phase, before any other: a key
 * pressed while the body has the keyboard gives the frame the keyboard and the key itself, once, so
 * a way of losing the focus nobody thought of never leaves the keys dead. Answers whether it
 * re-sent the key (the original is then stopped).
 */
export function keyboardFallback(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing || !keyboardLost()) return false
  if (event.target !== document.body && event.target !== document.documentElement) return false
  if (!focusEditor()) return false
  const root = editorRoot()
  if (!root) return false
  event.preventDefault()
  event.stopImmediatePropagation()
  const copy = new KeyboardEvent('keydown', {
    key: event.key,
    code: event.code,
    location: event.location,
    repeat: event.repeat,
    shiftKey: event.shiftKey,
    altKey: event.altKey,
    ctrlKey: event.ctrlKey,
    metaKey: event.metaKey,
    bubbles: true,
    cancelable: true,
    composed: true,
  })
  root.dispatchEvent(copy)
  return true
}

/**
 * When text editing on the canvas ends (Escape, a click elsewhere, an undo), kit-editor's hidden
 * text field keeps the keyboard, and its keys are text, never the tools' letters: T, R or V did
 * nothing until the canvas was clicked. The editor's frame takes the keyboard back then.
 */
export function useTextFieldReleased(takingText: boolean) {
  useEffect(() => {
    if (takingText) return
    const active = document.activeElement
    if (active instanceof HTMLElement && active.classList.contains('kui-text-sink')) focusEditor()
  }, [takingText])
}
