/**
 * The shell's own keys (2.7 §3), for upstream commands the kit's engine leaves to the editor (the
 * view, the panels): zoom steps and fits, Shift+E Design and Prototype, Alt+1/2 and Alt+8/9 panels,
 * Shift+I Resources, Cmd+K Actions. The frame gives every other key to the tools and the engine's
 * command shortcuts.
 */
import type { KeyboardEvent } from 'react'
import { isMac } from '@kition/sdk/editor'
import type { DesignKit } from '../kit/context'

export function shellKeyDown(designKit: DesignKit, event: KeyboardEvent): boolean {
  const mod = isMac() ? event.metaKey : event.ctrlKey
  const { ui } = designKit
  const code = event.code
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
