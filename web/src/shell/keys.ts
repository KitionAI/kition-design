/**
 * The shell's own keys (2.7 §3), for upstream commands the kit's engine leaves to the editor (the
 * view, the panels): zoom steps and fits, Shift+E Design and Prototype, Alt+1/2 and Alt+8/9 panels,
 * Shift+I Resources, Cmd+K Actions, the number keys for opacity and the arrows that pan with
 * nothing selected. The frame gives every other key to the tools and the engine's command
 * shortcuts.
 */
import type { KeyboardEvent } from 'react'
import { isMac } from '@kition/sdk/editor'
import type { DesignKit } from '../kit/context'

/** Two digits typed this close together are one exact opacity (4 then 5: 45 %). */
const DIGITS_MS = 600
/** How far an arrow pans the view with nothing selected (Shift: five times), in screen pixels. */
const PAN_PX = 64
const PAN_BIG_PX = 320

let digits: { typed: string; at: number } | null = null

/**
 * The number keys (upstream: the preference Use number keys for opacity, on by default): 1…9 set
 * the selection's opacity to 10 %…90 %, 0 to 100 %, and a second digit within 0.6 s makes the
 * two an exact percentage (`object.opacityKeys`). Not while editing text or vector points.
 */
function opacityKey(designKit: DesignKit, event: KeyboardEvent) {
  const digit = /^Digit([0-9])$/.exec(event.code)?.[1] ?? /^Numpad([0-9])$/.exec(event.code)?.[1]
  if (digit === undefined || event.shiftKey || event.altKey || event.metaKey || event.ctrlKey) return false
  const { inspect } = designKit
  if (!inspect.selection.length || designKit.tools.wantsText || inspect.editing === 'text' || inspect.vectorEdit || designKit.ui.rightTab === 'inspect') return false
  const now = performance.now()
  const typed = digits && digits.typed.length === 1 && now - digits.at <= DIGITS_MS ? digits.typed + digit : digit
  digits = { typed, at: now }
  // Preferences ▸ Use number keys for opacity can turn them off (the digit then does nothing).
  void designKit
    .query<{ numberKeysOpacity?: boolean }>('prefs')
    .then((prefs) => (prefs.numberKeysOpacity === false ? null : designKit.run('object.opacityKeys', { digits: typed })))
    .catch(() => undefined)
  return true
}

/** The arrows with nothing selected pan the view (the same screen distance at any zoom). */
function panKey(designKit: DesignKit, event: KeyboardEvent) {
  const step = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[event.key]
  if (!step || event.altKey || event.metaKey || event.ctrlKey) return false
  const { inspect } = designKit
  if (inspect.selection.length || designKit.tools.wantsText || inspect.editing === 'text' || inspect.vectorEdit) return false
  const handle = designKit.canvas.current
  const view = handle?.view()
  if (!handle || !view || !(view.zoom > 0)) return false
  const px = event.shiftKey ? PAN_BIG_PX : PAN_PX
  handle.panTo(view.x + (step[0] * px) / view.zoom, view.y + (step[1] * px) / view.zoom)
  return true
}

export function shellKeyDown(designKit: DesignKit, event: KeyboardEvent): boolean {
  const mod = isMac() ? event.metaKey : event.ctrlKey
  const { ui } = designKit
  const code = event.code
  if (opacityKey(designKit, event) || panKey(designKit, event)) return true
  if (mod && !event.altKey && (code === 'Equal' || code === 'Minus' || code === 'NumpadAdd' || code === 'NumpadSubtract')) {
    void designKit.viewTo(code === 'Minus' || code === 'NumpadSubtract' ? 'view.zoomOut' : 'view.zoomIn')
    return true
  }
  if (mod && !event.altKey && !event.shiftKey && code === 'Digit0') {
    void designKit.viewTo('view.zoom100')
    return true
  }
  if (mod && !event.altKey && !event.shiftKey && code === 'KeyK') {
    ui.setActions(!ui.actions)
    return true
  }
  if (mod) return false
  if (!event.shiftKey && !event.altKey && code === 'KeyN' && !designKit.tools.wantsText && !designKit.inspect.vectorEdit) {
    void designKit.viewTo('view.nextFrame')
    return true
  }
  if (event.shiftKey && !event.altKey) {
    switch (code) {
      case 'Digit0':
        void designKit.viewTo('view.zoom100')
        return true
      case 'Digit1':
        void designKit.viewTo('view.zoomFit')
        return true
      case 'Digit2':
        void designKit.viewTo('view.zoomSelection')
        return true
      case 'KeyN':
        if (designKit.tools.wantsText) return false
        void designKit.viewTo('view.prevFrame')
        return true
      case 'KeyE':
        if (designKit.tools.wantsText || designKit.inspect.vectorEdit) return false
        ui.setRightTab(ui.rightTab === 'design' ? 'prototype' : 'design')
        return true
      case 'KeyI':
        if (designKit.tools.wantsText) return false
        ui.setLeftTab('assets')
        ui.setLeftOpen(true)
        return true
    }
  }
  if (event.altKey && !event.shiftKey) {
    switch (code) {
      case 'Digit1':
        ui.setLeftTab('file')
        ui.setLeftOpen(true)
        return true
      case 'Digit2':
        ui.setLeftTab('assets')
        ui.setLeftOpen(true)
        return true
      case 'Digit8':
        ui.setRightTab('design')
        return true
      case 'Digit9':
        ui.setRightTab('prototype')
        return true
    }
  }
  return false
}
