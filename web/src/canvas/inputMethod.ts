/**
 * Input methods (Chinese, Japanese, Korean) while a text is edited on the canvas (hand plan stage
 * 6, walkthrough D9). kit-editor's text sink, a hidden text field, is where the input method
 * composes; it hands the engine the finished text only. Here:
 *
 * - what is being composed shows in the text at the caret, underlined and laid out with it
 *   (`text.edit.preedit`), without changing the document; an empty composition (Escape) takes it
 *   away, the finished text ends it;
 * - the text sink sits at the caret (`textCaret`), so the candidate window opens there;
 * - while composing, every key is the input method's (Enter picks, Escape cancels, the arrows and
 *   Backspace move in the composition): none reaches the tools, the editor's keys or the commands.
 */
import { useEffect, useRef } from 'react'
import { useView, type DesignKit } from '../kit/context'

/** The longest composition the engine shows. */
const MOST = 1000

/** The text sink an event came from, if it did. */
function sinkOf(target: EventTarget | null): HTMLTextAreaElement | null {
  return target instanceof HTMLTextAreaElement && target.classList.contains('kui-text-sink') ? target : null
}

/** Whether a key belongs to an input method's composition (WebKit's first one says so only by its code). */
export function composingKey(event: KeyboardEvent) {
  return event.isComposing || event.keyCode === 229 || event.key === 'Process'
}

/**
 * The input method's cursor in the composition `text`, in characters (the engine's unit): where
 * `field` has it once its value is the composition, else at its end (a composition event comes
 * before the field shows what it composes).
 */
function cursorIn(field: HTMLTextAreaElement, text: string) {
  const length = Array.from(text).length
  if (field.value !== text) return length
  const end = Math.min(field.selectionEnd ?? text.length, text.length)
  return Math.min(Array.from(text.slice(0, end)).length, length)
}

/** Shows compositions in the text being edited and keeps their keys from the editor. */
export function useInputMethod(designKit: DesignKit) {
  const latest = useRef(designKit)
  latest.current = designKit
  useEffect(() => {
    // One composition update on its way to the engine at a time; the newest waits behind it.
    let sending = false
    let next: { text: string; cursor: number } | null = null
    const send = () => {
      if (sending || !next) return
      const params = next
      next = null
      sending = true
      void latest.current
        .run('text.edit.preedit', params)
        .catch(() => undefined)
        .finally(() => {
          sending = false
          send()
        })
    }
    const editing = () => latest.current.inspect.editing === 'text' || latest.current.tools.wantsText
    const update = (event: CompositionEvent) => {
      const field = sinkOf(event.target)
      if (!field || !editing()) return
      const text = Array.from(event.data ?? '').slice(0, MOST).join('')
      // A composition starts empty: nothing to show yet.
      if (event.type === 'compositionstart' && !text) return
      next = { text, cursor: cursorIn(field, text) }
      send()
    }
    const end = (event: CompositionEvent) => {
      if (!sinkOf(event.target) || !editing()) return
      // A commit types the finished text (the text sink sends it), which ends the composition;
      // a cancelled one ends with nothing to type.
      if (!event.data) {
        next = { text: '', cursor: 0 }
        send()
      } else {
        next = null
      }
    }
    const key = (event: KeyboardEvent) => {
      if (sinkOf(event.target) && composingKey(event)) event.stopPropagation()
    }
    window.addEventListener('compositionstart', update, true)
    window.addEventListener('compositionupdate', update, true)
    window.addEventListener('compositionend', end, true)
    window.addEventListener('keydown', key, true)
    window.addEventListener('keyup', key, true)
    return () => {
      window.removeEventListener('compositionstart', update, true)
      window.removeEventListener('compositionupdate', update, true)
      window.removeEventListener('compositionend', end, true)
      window.removeEventListener('keydown', key, true)
      window.removeEventListener('keyup', key, true)
    }
  }, [])

  // The text sink at the caret, after every change while a text is edited and every move of the
  // view: the input method opens its candidate window at the field.
  const { inspect, revision, query } = designKit
  const view = useView()
  const editing = inspect.editing === 'text'
  useEffect(() => {
    if (!editing || !view) return
    let live = true
    void query<{ x0: number; y0: number; x1: number; y1: number } | null>('textCaret')
      .then((caret) => {
        const field = document.querySelector<HTMLTextAreaElement>('textarea.kui-text-sink')
        const surface = document.querySelector('[data-testid="kit-canvas-surface"]')
        if (!live || !caret || !field || !surface) return
        const box = surface.getBoundingClientRect()
        const holder = (field.offsetParent ?? document.body).getBoundingClientRect()
        const x = box.left + (Math.min(caret.x0, caret.x1) - view.x) * view.zoom
        const top = box.top + (Math.min(caret.y0, caret.y1) - view.y) * view.zoom
        const height = Math.max(1, Math.abs(caret.y1 - caret.y0) * view.zoom)
        if (![x, top, height].every(Number.isFinite)) return
        field.style.left = `${Math.round(x - holder.left)}px`
        field.style.top = `${Math.round(top - holder.top)}px`
        field.style.height = `${Math.round(height)}px`
      })
      .catch(() => undefined)
    return () => {
      live = false
    }
  }, [editing, query, revision, view])
}
