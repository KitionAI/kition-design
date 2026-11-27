/**
 * Files dragged onto the canvas (hand plan stage 7, walkthrough L4; reference R-L38): while files
 * are over it, the canvas shows a hint ("Drop to import", or "Drop to open" for a document);
 * images and SVG files dropped are imported with their top-left where they land, each one undo
 * step (`image.place`, `file.import`). A document (`.kdesign`, `.fig`, …) is the host's to open:
 * the editor says so and leaves it. The bytes are the dropped `File`'s, which the page was given;
 * no file is read or written behind the host's back. A file dropped anywhere else on the page is
 * stopped, so the browser does not open it in place of the editor.
 */
import { useEffect, useState, type RefObject } from 'react'
import type { DesignKit } from '../kit/context'
import { importSvg, placeImage } from '../menus/clipboard'
import { refusedInDevMode } from '../menus/invoke'
import { own } from '../menus/strings'
import { showToast } from '../ui/Toast'

const IMAGES = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'tif', 'tiff']
const DOCUMENTS = ['kdesign', 'fig', 'sketch', 'json', 'penpot', 'jam']

type Hint = 'import' | 'open' | null

const extension = (name: string) => (name.includes('.') ? (name.split('.').pop() ?? '').toLowerCase() : '')
const stem = (name: string) => name.replace(/\.[^.]*$/, '') || name

/** Whether a drag carries files. */
const carriesFiles = (event: DragEvent) => Array.from(event.dataTransfer?.types ?? []).includes('Files')

/** What a drop of the dragged files would do: images import, anything else is opened (by the host). */
function hintFor(event: DragEvent): Hint {
  const items = Array.from(event.dataTransfer?.items ?? []).filter((item) => item.kind === 'file')
  return items.length && items.every((item) => !item.type || !item.type.startsWith('image/')) ? 'open' : 'import'
}

/** Imports dropped `files` with their top-left at page point `at`. */
async function importDropped(designKit: DesignKit, files: File[], at: { x: number; y: number } | null) {
  for (const file of files) {
    const type = extension(file.name)
    if (DOCUMENTS.includes(type)) {
      showToast(own(designKit, 'drop.document', { name: file.name }))
      continue
    }
    if (refusedInDevMode(designKit)) return
    if (type === 'svg' || file.type === 'image/svg+xml') await importSvg(designKit, await file.text(), file.name, at)
    else if (IMAGES.includes(type) || file.type.startsWith('image/')) await placeImage(designKit, file, stem(file.name), at)
    else showToast(own(designKit, 'drop.unsupported', { name: file.name }))
  }
}

/** The page point under client point (`x`, `y`) on the canvas, or null off it. */
function pageAt(designKit: DesignKit, x: number, y: number) {
  const surface = document.querySelector('[data-testid="kit-canvas-surface"]')?.getBoundingClientRect()
  const view = designKit.canvas.current?.view()
  if (!surface || !view || !(view.zoom > 0)) return null
  const area = designKit.inspect.area
  return { x: area.x + view.x + (x - surface.left) / view.zoom, y: area.y + view.y + (y - surface.top) / view.zoom }
}

/** Takes files dropped on `root` (the canvas); answers the hint to show while they are over it. */
export function useFileDrop(root: RefObject<HTMLElement | null>, designKit: DesignKit): Hint {
  const [hint, setHint] = useState<Hint>(null)
  useEffect(() => {
    const element = root.current
    if (!element) return
    const over = (event: DragEvent) => {
      if (!carriesFiles(event)) return
      event.preventDefault()
      if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy'
      setHint(hintFor(event))
    }
    const leave = (event: DragEvent) => {
      if (!(event.relatedTarget instanceof Node && element.contains(event.relatedTarget))) setHint(null)
    }
    const drop = (event: DragEvent) => {
      setHint(null)
      if (!carriesFiles(event)) return
      event.preventDefault()
      const files = Array.from(event.dataTransfer?.files ?? [])
      const kit = latestKit
      if (kit && files.length && !kit.readOnly) void importDropped(kit, files, pageAt(kit, event.clientX, event.clientY))
    }
    // Elsewhere on the page a dropped file would replace the editor: stopped.
    const away = (event: DragEvent) => {
      if (carriesFiles(event) && !(event.target instanceof Node && element.contains(event.target))) event.preventDefault()
    }
    element.addEventListener('dragenter', over)
    element.addEventListener('dragover', over)
    element.addEventListener('dragleave', leave)
    element.addEventListener('drop', drop)
    document.addEventListener('dragover', away)
    document.addEventListener('drop', away)
    return () => {
      element.removeEventListener('dragenter', over)
      element.removeEventListener('dragover', over)
      element.removeEventListener('dragleave', leave)
      element.removeEventListener('drop', drop)
      document.removeEventListener('dragover', away)
      document.removeEventListener('drop', away)
    }
  }, [root])
  latestKit = designKit
  return hint
}

let latestKit: DesignKit | null = null

/** The hint over the whole canvas while files are dragged over it. */
export function DropHint({ hint, designKit }: { hint: Hint; designKit: DesignKit }) {
  if (!hint) return null
  return (
    <div className="design-drop-hint" data-testid="design-drop-hint">
      <span>{own(designKit, hint === 'open' ? 'drop.open' : 'drop.import')}</span>
    </div>
  )
}
