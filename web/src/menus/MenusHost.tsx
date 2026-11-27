/**
 * The menus package's mount point (the shell draws it once, in the canvas cell): the key
 * dispatcher, the dialogs, the context menus and the comments layer, and the UI requests they
 * answer (`dialog.*`, `panel.shortcuts`, `contextMenu`, `layers.rename` for several layers,
 * `comment.*`).
 */
import { useEffect, useRef } from 'react'
import { useDesignKit } from '../kit/hooks'
import { useUiRequest } from '../kit/requests'
import { DialogHost } from '../dialogs/DialogHost'
import { openDialog, toggleDialog } from '../dialogs/store'
import { CommentsLayer } from '../comments/CommentsLayer'
import { ContextMenuHost } from './ContextMenu'
import { openContextMenu } from './contextMenuStore'
import { openContextMenuAtSelection, setMenusDesignKit } from './invoke'
import { focusEditor, keyboardFallback, keyboardLost } from '../kit/focus'
import { menuKeyDown } from './keymap'

export function MenusHost() {
  const designKit = useDesignKit()
  setMenusDesignKit(designKit)
  const latest = useRef(designKit)
  latest.current = designKit

  // The keys the frame, the tools and the shell leave (keymap.ts), before any of them; first, a
  // key the page's body has goes to the editor's frame (kit/focus.ts).
  useEffect(() => {
    const down = (event: KeyboardEvent) => {
      if (keyboardFallback(event)) return
      if (menuKeyDown(latest.current, event)) {
        event.preventDefault()
        event.stopPropagation()
      }
    }
    document.addEventListener('keydown', down, true)
    return () => document.removeEventListener('keydown', down, true)
  }, [])

  // The editor has the keyboard as soon as it opens, and whenever the host gives the page the focus
  // with nothing in it focused (K6).
  useEffect(() => {
    const take = () => {
      if (keyboardLost() && document.hasFocus()) focusEditor()
    }
    take()
    window.addEventListener('focus', take)
    return () => window.removeEventListener('focus', take)
  }, [])

  // Upstream checks the fonts after a file opens: Missing fonts opens itself when the editor lacks some.
  const checked = useRef<unknown>(null)
  useEffect(() => {
    const doc = designKit.doc
    if (!doc || checked.current === doc) return
    checked.current = doc
    void designKit
      .query<{ families?: unknown[] }>('missingFonts')
      .then((answer) => {
        if (answer.families?.length) openDialog('missingFonts')
      })
      .catch(() => undefined)
  }, [designKit])

  useUiRequest('dialog.preferences', () => openDialog('preferences'))
  useUiRequest('panel.shortcuts', () => toggleDialog('shortcuts'))
  useUiRequest('dialog.about', () => openDialog('about'))
  useUiRequest('dialog.colorProfile', () => openDialog('colorProfile'))
  useUiRequest('dialog.missingFonts', () => openDialog('missingFonts'))
  // Rename with several layers is the batch rename (one layer renames in the Layers panel).
  useUiRequest('layers.rename', (data) => {
    const ids = Array.isArray((data as { ids?: unknown })?.ids) ? ((data as { ids: number[] }).ids) : []
    if (ids.length > 1) openDialog('batchRename', { ids })
  })
  useUiRequest('contextMenu', (data) => {
    const at = data as { clientX?: number; clientY?: number; source?: 'canvas' | 'layers'; deep?: boolean } | null
    if (typeof at?.clientX === 'number' && typeof at.clientY === 'number') openContextMenu({ clientX: at.clientX, clientY: at.clientY, source: at.source, deep: at.deep })
    else openContextMenuAtSelection(latest.current)
  })

  return (
    <>
      <CommentsLayer />
      <ContextMenuHost />
      <DialogHost />
    </>
  )
}
