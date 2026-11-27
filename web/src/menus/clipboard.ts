/**
 * The system clipboard (hand plan stage 7, walkthrough L5, D8, R7). Copy, Cut and Paste used to
 * be the engine's own clipboard only: text or an image copied in another app never pasted, and
 * nothing the editor copied reached another app or another document.
 *
 * Cmd+C, Cmd+X and Cmd+V keep the browser's own action, so the page gets its `copy`, `cut` and
 * `paste` events, which carry the system clipboard and need no permission:
 *
 * - Copy and Cut of layers put this editor's mark on the clipboard at once (`kition-design://
 *   clipboard/<n>`) and run `edit.copy` / `edit.cut`; then the clipboard gets what another app can
 *   use: the text of the layers when all are text layers (a line each), else their SVG. A copy
 *   while a text is edited puts the selected characters on it (no selection: nothing changes).
 * - Paste reads the clipboard in the reference's order: layers another design app copied (in its
 *   HTML) → `edit.pasteDesignLayers`; something this editor did not put there → an image becomes an
 *   image layer, SVG text an editable frame, other text a text layer (in the middle of the view, or
 *   where Paste here was asked); otherwise the engine's own copy (`edit.paste`). While a text is
 *   edited, Paste types plain text: the clipboard's, or the characters this editor copied.
 * - Shift+Cmd+V (Paste over selection) and Alt+Cmd+V (Paste properties) stay the engine's own.
 *
 * The menus' Copy and Cut run the same way (`document.execCommand` gives the page the event); the
 * menus' Paste reads the clipboard only where the browser already allows it without asking, else
 * pastes the engine's copy. A key whose event the browser never sends runs as the command.
 */
import { useEffect, useRef } from 'react'
import { isMac } from '@kition/sdk/editor'
import type { DesignKit } from '../kit/context'
import { composingKey } from '../canvas/inputMethod'
import { keepCopiedText, namesLayers, selectedText, textCopying } from '../kit/textEditing'
import { showToast } from '../ui/Toast'
import { refusedInDevMode, toBase64 } from './invoke'
import { own } from './strings'

type Kind = 'copy' | 'cut' | 'paste'
type PlainRun = (id: string, params?: Record<string, unknown>) => Promise<string | null>
/** What a paste carries: the clipboard's plain text and HTML, and the images on it. */
type Pasted = { text: string; html: string; images: Blob[] }
type Point = { x: number; y: number }

/** The start of the text this editor puts on the clipboard for copied layers. */
export const CLIPBOARD_MARK = 'kition-design://clipboard/'
/** The most an image or a text pasted may be (the engine refuses more). */
const MOST_BYTES = 256 * 1024 * 1024
const MOST_TEXT = 1_000_000

let latest: DesignKit | null = null
/** The editor's run without this module (App.tsx): the clipboard's own commands go through it. */
let plainRun: PlainRun | null = null
/**
 * What this editor put on the clipboard last: the texts it wrote there, and whether they stand
 * for copied layers (a paste of them is the engine's copy) or for characters of a text.
 */
let mine: { texts: string[]; layers: boolean } | null = null
/** A copy wrote nothing to the clipboard (the browser refused): the next paste is the engine's. */
let unwritten = false
/** The copy or cut on its way to the engine: a paste waits for it. */
let copying: Promise<unknown> = Promise.resolve()
/** The command a copy or cut event started, for the menu's Copy that sent the event. */
let eventCommand: Promise<string | null> | null = null
/** Keys whose clipboard event has not come yet (the browser may never send it). */
const waiting: Partial<Record<Kind, number>> = {}
/** The characters selected in the text being edited, as the engine answered them for `key`. */
let selection: { key: string; text: string } | null = null

/** For App.tsx: the editor's run that does not come back here. */
export function setPlainRun(run: PlainRun) {
  plainRun = run
}

const editingText = (designKit: DesignKit) => designKit.inspect.editing === 'text' || designKit.tools.wantsText

const textEditKey = (designKit: DesignKit) => {
  const edit = designKit.inspect.textEdit
  return edit ? `${edit.id}:${Math.min(edit.anchor, edit.caret)}:${Math.max(edit.anchor, edit.caret)}` : ''
}

/** A field that takes text itself (its copy and paste are its own), not kit-editor's text sink. */
function ownField(target: EventTarget | null) {
  if (target instanceof HTMLInputElement) return !['checkbox', 'radio', 'button', 'submit', 'reset', 'range', 'color', 'file', 'image'].includes(target.type)
  if (target instanceof HTMLTextAreaElement) return !target.classList.contains('kui-text-sink')
  return target instanceof HTMLSelectElement || (target instanceof HTMLElement && target.isContentEditable)
}

/** Whether an event at `target` is the editor's: not a field's, not a dialog's. */
function editorsEvent(target: EventTarget | null) {
  if (!latest?.doc || ownField(target)) return false
  if (document.querySelector('.kui-dialog-backdrop')) return false
  return !(target instanceof Element && target.closest('.kui-dialog'))
}

/** Text selected on the page itself (a panel's text): its copy is the browser's. */
function pageTextSelected() {
  const range = document.getSelection()
  return !!range && !range.isCollapsed && range.toString() !== ''
}

const run = (id: string, params?: Record<string, unknown>) => (plainRun ? plainRun(id, params) : Promise.resolve(null))

function parse<R>(answer: string | null): R | null {
  if (answer == null) return null
  try {
    return JSON.parse(answer) as R
  } catch {
    return null
  }
}

/** Remembers `text` as one this editor wrote. */
function remember(text: string) {
  if (mine && !mine.texts.includes(text)) mine.texts.push(text)
}

/**
 * Puts `text` (when it comes) on the system clipboard after the event's own data: from inside the
 * event, so the browser counts it as the person's copy. Answers whether it got there.
 */
function writeLater(text: Promise<string | null>): Promise<boolean> {
  const clipboard = navigator.clipboard
  const blob = text.then((value) => {
    if (!value) throw new Error('nothing to write')
    remember(value)
    return new Blob([value], { type: 'text/plain' })
  })
  // Nothing to write is no error: the clipboard keeps the mark.
  blob.catch(() => undefined)
  try {
    if (clipboard?.write && typeof ClipboardItem !== 'undefined') return clipboard.write([new ClipboardItem({ 'text/plain': blob })]).then(() => true, () => false)
    if (clipboard?.writeText) return text.then((value) => (value ? clipboard.writeText(value).then(() => true) : false)).catch(() => false)
  } catch {
    // A browser without the asynchronous clipboard.
  }
  return Promise.resolve(false)
}

/** The text another app gets for layers `ids`: their characters when all are text layers, else their SVG. */
async function layersText(designKit: DesignKit, ids: number[]): Promise<{ text: string | null; characters: string | null }> {
  const nodes = await Promise.all(ids.map((id) => designKit.query<{ type?: string; characters?: string }>('node', { id }).catch(() => null)))
  if (nodes.length && nodes.every((node) => node?.type === 'TEXT' && typeof node.characters === 'string')) {
    const characters = nodes.map((node) => node?.characters ?? '').join('\n')
    return { text: characters, characters }
  }
  const answer = parse<{ data?: string }>(await run('file.export', { ids, format: 'svg' }).catch(() => null))
  if (typeof answer?.data !== 'string') return { text: null, characters: null }
  try {
    return { text: new TextDecoder().decode(Uint8Array.from(atob(answer.data), (c) => c.charCodeAt(0))), characters: null }
  } catch {
    return { text: null, characters: null }
  }
}

/** A `copy` or `cut` event the editor takes (`kind`): fills its clipboard data, runs the command. */
function copyEvent(event: ClipboardEvent, kind: 'copy' | 'cut') {
  window.clearTimeout(waiting[kind])
  delete waiting[kind]
  const designKit = latest
  if (!designKit || !editorsEvent(event.target) || pageTextSelected()) return
  if (kind === 'cut' && designKit.readOnly) return
  const command = kind === 'cut' ? 'edit.cut' : 'edit.copy'
  if (editingText(designKit)) {
    const edit = designKit.inspect.textEdit
    if (edit && edit.anchor === edit.caret) return
    // The characters as the engine last answered for this selection; the engine's newest are
    // read by the command (they go on the clipboard too when they differ).
    const known = selection && selection.key === textEditKey(designKit) ? selection.text : ''
    event.preventDefault()
    event.clipboardData?.setData('text/plain', known)
    mine = { texts: known ? [known] : [], layers: false }
    const started = run(command)
    eventCommand = started
    const fresh = started.then(
      () => textCopying(),
      () => '',
    )
    copying = fresh
    void writeLater(fresh.then((text) => (text && text !== known ? text : null)))
    return
  }
  const ids = designKit.inspect.selection
  if (!ids.length) return
  event.preventDefault()
  const mark = `${CLIPBOARD_MARK}${ids.length}`
  event.clipboardData?.setData('text/plain', mark)
  mine = { texts: [mark], layers: true }
  unwritten = false
  // What other apps get (vector points copy as the engine's own only); a paste in a text types
  // the characters of copied text layers, nothing for other layers.
  const content = (designKit.inspect.editing === 'vector' ? Promise.resolve({ text: null, characters: null }) : layersText(designKit, ids)).then((read) => {
    keepCopiedText(read.characters)
    return read
  })
  // A cut takes the layers away: after they were read.
  const started = kind === 'cut' ? content.then(() => run(command)) : run(command)
  eventCommand = started
  copying = Promise.all([started, content]).catch(() => undefined)
  void writeLater(content.then(({ text }) => text))
}

/** The clipboard data of a paste event. */
function pastedFrom(data: DataTransfer | null): Pasted {
  const images: Blob[] = []
  for (const file of Array.from(data?.files ?? [])) if (file.type.startsWith('image/') && file.type !== 'image/svg+xml') images.push(file)
  if (!images.length)
    for (const item of Array.from(data?.items ?? [])) {
      if (item.kind !== 'file' || !item.type.startsWith('image/') || item.type === 'image/svg+xml') continue
      const file = item.getAsFile()
      if (file) images.push(file)
    }
  return { text: data?.getData('text/plain') ?? '', html: data?.getData('text/html') ?? '', images }
}

/** What the system clipboard holds, where the browser lets the page read it without asking; else null. */
async function readClipboard(): Promise<Pasted | null> {
  try {
    const state = await navigator.permissions?.query({ name: 'clipboard-read' as PermissionName })
    if (state?.state !== 'granted' || !navigator.clipboard?.read) return null
    const items = await navigator.clipboard.read()
    const pasted: Pasted = { text: '', html: '', images: [] }
    for (const item of items) {
      for (const type of item.types) {
        if (type === 'text/plain' && !pasted.text) pasted.text = await (await item.getType(type)).text()
        else if (type === 'text/html' && !pasted.html) pasted.html = await (await item.getType(type)).text()
        else if (type.startsWith('image/') && type !== 'image/svg+xml') pasted.images.push(await item.getType(type))
      }
    }
    return pasted
  } catch {
    return null
  }
}

/** The middle of the view, in page coordinates. */
export function viewCentre(designKit: DesignKit): Point | null {
  const view = designKit.canvas.current?.view()
  if (!view || !(view.zoom > 0)) return null
  const area = designKit.inspect.area
  return { x: area.x + view.x + view.width / view.zoom / 2, y: area.y + view.y + view.height / view.zoom / 2 }
}

/** An image's size as the engine places it (at most 1000 on its longer side), or null when it can't be read. */
async function placedSize(image: Blob): Promise<{ width: number; height: number } | null> {
  try {
    const bitmap = await createImageBitmap(image)
    const { width, height } = bitmap
    bitmap.close()
    if (!(width > 0 && height > 0)) return null
    const scale = Math.min(1000 / Math.max(width, height), 1)
    return { width: width * scale, height: height * scale }
  } catch {
    return null
  }
}

/** Places image `image` named `name`: its top-left at `at`, or centred in the view. */
export async function placeImage(designKit: DesignKit, image: Blob, name: string, at: Point | null) {
  if (image.size > MOST_BYTES) {
    showToast(own(designKit, 'paste.tooLarge'))
    return null
  }
  const data = toBase64(new Uint8Array(await image.arrayBuffer()))
  let point = at
  if (!point) {
    const middle = viewCentre(designKit)
    const size = middle ? await placedSize(image) : null
    point = middle && size ? { x: Math.round(middle.x - size.width / 2), y: Math.round(middle.y - size.height / 2) } : middle
  }
  return run('image.place', { data, name, ...(point ?? {}) })
}

/** Imports SVG `text` as an editable frame: its top-left at `at`, or centred in the view. */
export async function importSvg(designKit: DesignKit, text: string, name: string, at: Point | null) {
  const data = toBase64(new TextEncoder().encode(text))
  const middle = at ? null : viewCentre(designKit)
  const answer = await run('file.import', { data, name, ...(at ?? middle ?? {}), ...(at ? {} : { center: true }) })
  const warnings = parse<{ warnings?: unknown[] }>(answer)?.warnings
  if (warnings?.length) showToast(own(designKit, 'import.partly', { n: warnings.length }))
  return answer
}

const looksLikeSvg = (text: string) => /^\s*(<\?xml[^>]*>\s*)?(<!--[\s\S]*?-->\s*)*(<!DOCTYPE[^>]*>\s*)?<svg[\s>]/i.test(text)

/**
 * Pastes `pasted` (a paste event's, or the clipboard read for a menu's Paste): at `at` (page
 * coordinates) for Paste here. Answers the command's answer.
 */
async function pasteFrom(designKit: DesignKit, pasted: Pasted, at: Point | null): Promise<string | null> {
  await copying
  const { text, html, images } = pasted
  const written = text !== '' && !!mine?.texts.includes(text)
  const position = at ? { x: at.x, y: at.y } : {}
  if (editingText(designKit)) {
    // Plain text only: the characters this editor copied when the clipboard is its own (also when
    // the clipboard could not be read), the clipboard's text when another app put it there.
    if (written || (!text && !images.length)) return run('edit.paste')
    if (!text || text.startsWith(CLIPBOARD_MARK)) return null
    return run('text.edit.insert', { text: Array.from(text.replace(/\r\n?/g, '\n')).slice(0, MOST_TEXT).join('') })
  }
  // Layers another design app copied travel in the clipboard's HTML.
  if (html.includes('(figma)')) {
    if (refusedInDevMode(designKit)) return null
    return run('edit.pasteDesignLayers', { html, ...position })
  }
  // Characters copied from a text paste as a text layer, like text from another app.
  const layers = text.startsWith(CLIPBOARD_MARK) || (written && mine?.layers === true) || (unwritten && mine?.layers === true)
  if (!layers && images[0]) {
    if (refusedInDevMode(designKit)) return null
    return placeImage(designKit, images[0], 'Pasted image', at)
  }
  if (!layers && text.trim()) {
    if (refusedInDevMode(designKit)) return null
    if (looksLikeSvg(text)) return importSvg(designKit, text, 'Pasted.svg', at)
    const point = at ?? viewCentre(designKit)
    return run('text.add', { text: Array.from(text.replace(/\r\n?/g, '\n')).slice(0, MOST_TEXT).join(''), ...(point ?? {}) })
  }
  return run('edit.paste', position)
}

/** A `paste` event the editor takes. */
function pasteEvent(event: ClipboardEvent) {
  window.clearTimeout(waiting.paste)
  delete waiting.paste
  const designKit = latest
  if (!designKit || !editorsEvent(event.target)) return
  event.preventDefault()
  if (designKit.readOnly) return
  void pasteFrom(designKit, pastedFrom(event.clipboardData), null)
}

/** The page's clipboard events while the editor shows a document. */
function beforeClipboard(event: Event) {
  // WebKit sends the copy, cut and paste events of a page without a text selection only when its
  // `before…` event is cancelled.
  if (editorsEvent(event.target)) event.preventDefault()
}

/**
 * For the editor's run (App.tsx): Copy, Cut and Paste of the selection (not of named layers) go
 * through the clipboard. Answers the run's answer, or null when `id` runs as usual.
 */
export function clipboardCommand(id: string, params: Record<string, unknown> | undefined): Promise<string | null> | null {
  const designKit = latest
  if (!designKit || namesLayers(params)) return null
  if (id === 'edit.copy' || id === 'edit.cut') {
    if (params && Object.keys(params).length) return null
    // The browser's copy event, which writes the clipboard (allowed after a person's click or key).
    eventCommand = null
    let sent = false
    try {
      sent = document.execCommand(id === 'edit.copy' ? 'copy' : 'cut')
    } catch {
      sent = false
    }
    const started = eventCommand as Promise<string | null> | null
    eventCommand = null
    if (sent && started) return started
    // No event: the engine's copy, and the clipboard keeps what it had (a paste here then is this copy).
    if (!editingText(designKit) && designKit.inspect.selection.length) {
      mine = { texts: [], layers: true }
      unwritten = true
    }
    return null
  }
  if (id === 'edit.paste') {
    const at = params && typeof params.x === 'number' && typeof params.y === 'number' ? { x: params.x, y: params.y } : null
    return readClipboard().then((pasted) => pasteFrom(designKit, pasted ?? { text: '', html: '', images: [] }, at))
  }
  return null
}

/**
 * For the key listener (MenusHost, document capture phase, before kit-editor's shortcuts):
 * Cmd+C, Cmd+X and Cmd+V on the editor keep their default, so the browser sends the clipboard
 * event; kit-editor's shortcut (which cancels it) does not run. Answers whether it took the key.
 */
export function clipboardKeyDown(event: KeyboardEvent): boolean {
  if (!latest || event.altKey || event.shiftKey || event.defaultPrevented || composingKey(event)) return false
  if (isMac() ? !event.metaKey || event.ctrlKey : !event.ctrlKey || event.metaKey) return false
  const key = event.key.toLowerCase()
  const kind: Kind | null = key === 'c' || event.code === 'KeyC' ? 'copy' : key === 'x' || event.code === 'KeyX' ? 'cut' : key === 'v' || event.code === 'KeyV' ? 'paste' : null
  if (!kind || !editorsEvent(event.target)) return false
  event.stopPropagation()
  window.clearTimeout(waiting[kind])
  // The browser sends the event right after the key, before this runs: if it did not, the command.
  waiting[kind] = window.setTimeout(() => {
    delete waiting[kind]
    const designKit = latest
    if (designKit) void designKit.run(kind === 'copy' ? 'edit.copy' : kind === 'cut' ? 'edit.cut' : 'edit.paste')
  }, 0)
  return true
}

/** Mounts the clipboard: the page's clipboard events, and the text selection a copy in a text puts there. */
export function useClipboard(designKit: DesignKit) {
  latest = designKit
  useEffect(() => {
    const copy = (event: ClipboardEvent) => copyEvent(event, 'copy')
    const cut = (event: ClipboardEvent) => copyEvent(event, 'cut')
    document.addEventListener('copy', copy, true)
    document.addEventListener('cut', cut, true)
    document.addEventListener('paste', pasteEvent, true)
    for (const name of ['beforecopy', 'beforecut', 'beforepaste']) document.addEventListener(name, beforeClipboard, true)
    return () => {
      document.removeEventListener('copy', copy, true)
      document.removeEventListener('cut', cut, true)
      document.removeEventListener('paste', pasteEvent, true)
      for (const name of ['beforecopy', 'beforecut', 'beforepaste']) document.removeEventListener(name, beforeClipboard, true)
    }
  }, [])

  // The characters selected in the text being edited, asked after every change of the selection,
  // so a copy has them at once (a clipboard event cannot wait for the engine).
  const key = editingText(designKit) ? textEditKey(designKit) : ''
  const revision = designKit.revision
  const asked = useRef('')
  useEffect(() => {
    if (!key || !latest) {
      selection = null
      return
    }
    asked.current = key
    void latest
      .invoke<string>('query', 'textSelection', '{}')
      .then((answer) => {
        if (asked.current === key) selection = { key, text: selectedText(answer) }
      })
      .catch(() => undefined)
    // Asked again whenever the document changes too (the characters typed over a selection).
  }, [key, revision])
}
