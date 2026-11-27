/**
 * The canvas: kit-editor's canvas rendering the engine (zoom and pan, wheel and pinch, the
 * engine's tools and overlays), the page's background behind the document, the canvas chrome
 * drawn over it (`chrome.ts`), the auto layout padding and gap handles (`autoLayoutHandles.tsx`),
 * the field that renames a frame on its name label, and the editor's
 * own canvas input (`input/`): the order its layers are offered presses in, the zoom tool while Z
 * is held, ruler guides, the pointer sent again for a modifier and at the edge, the wheel.
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { KitDocumentCanvas, isMac, type KitCanvasView, type RasterRect, type RasterView } from '@kition/sdk/editor'
import { useDesignKit, useTools, useView } from '../kit/hooks'
import { dispatchUiRequests, useUiRequest } from '../kit/requests'
import { returnKeyboardLater, useTextFieldReleased } from '../kit/focus'
import { RenameInput } from '../left/fields'
import type { DesignChrome } from '../kit/types'
import { chromeDecoration, readTheme, type ZoneKey } from './chrome'
import { CanvasOverlays } from './overlays'
import { setCanvasPage, setSizePill } from './engineOverlays'
import { contentBounds, raster, rememberPageView, rememberedPageView } from './view'
import { useAreaOriginKept, useCanvasInput } from './input/wheel'
import { onSurface, usePressLayer, usePressRouting } from './input/order'
import { useCanvasPointer } from './input/pointer'
import { ZoomArea, useZoomTool } from './input/zoomTool'
import { GuidePreview, useRulerGuides } from './input/guides'
import { useInputMethod } from './inputMethod'
import { DropHint, useFileDrop } from './fileDrop'
import { AutoLayoutHandles } from './autoLayoutHandles'
import { ProtoCanvasLayer } from '../prototype/ProtoCanvasLayer'
import { DevCanvasLayer } from '../dev/DevCanvasLayer'

/** Shows the page's background behind the document, as upstream's canvas does: kit-editor paints `--kit-canvas` there. */
function usePageBackdrop(background: string) {
  useLayoutEffect(() => {
    const root = document.documentElement
    if (!/^#[0-9a-f]{6}$/i.test(background)) return undefined
    const apply = () => {
      if (root.style.getPropertyValue('--kit-canvas').trim().toLowerCase() !== background.toLowerCase()) root.style.setProperty('--kit-canvas', background)
    }
    apply()
    // The host's theme writes its own `--kit-canvas`: the page's background comes back over it.
    const observer = new MutationObserver(apply)
    observer.observe(root, { attributes: true, attributeFilter: ['style'] })
    return () => observer.disconnect()
  }, [background])
}

export function DesignCanvas({ onView }: { onView: (view: KitCanvasView) => void }) {
  const designKit = useDesignKit()
  const { inspect, canvas, invoke, revision, query, doc } = designKit
  const wantsText = useTools((tools) => tools.wantsText)
  const activeTool = useTools((tools) => tools.active)
  const view = useView()
  usePageBackdrop(inspect.background)
  // The engine's overlays take the page's green and keep guide labels clear of the size pill.
  setCanvasPage(inspect.background)
  const viewMemory = useRef<RasterView | null>(null)
  // The document stays put when the engine's area moves its origin; the wheel and triple-click.
  useAreaOriginKept(designKit)
  const root = useRef<HTMLDivElement>(null)
  useCanvasInput(root, designKit, isMac())
  // One order for everything over the canvas that takes a press (input/order.ts); the handles,
  // fields and other elements laid over the canvas have their own presses.
  usePressRouting()
  usePressLayer('handles', (event) => (onSurface(event.target) ? undefined : 'own'))
  // Modifiers with the pointer still, and drags held at the edge, reach the tool.
  const handle = useCallback(() => canvas.current, [canvas])
  useCanvasPointer(handle)
  // Hold Z to zoom.
  const zoomTool = useZoomTool(designKit)
  // Files dropped on the canvas are imported where they land.
  const dropHint = useFileDrop(root, designKit)
  // Input methods compose in the text at the caret, their candidate window beside it.
  useInputMethod(designKit)
  // The text keys move as this platform's do (Alt or Ctrl by word).
  useEffect(() => {
    if (doc) void invoke('setPlatform', isMac()).catch(() => undefined)
  }, [doc, invoke])
  // The keyboard is the editor's again when text editing ends.
  useTextFieldReleased(wantsText)

  // The first view fits the page's content (upstream `zoom_to_fit` on open).
  const [fit, setFit] = useState<{ doc: unknown; area: RasterRect | null } | null>(null)
  useEffect(() => {
    if (!doc || fit?.doc === doc) return
    let live = true
    void contentBounds(query, inspect).then((area) => {
      if (live) {
        const shown = area ?? inspect.initialArea
        setFit({ doc, area: shown ? raster(shown) : null })
      }
    })
    return () => {
      live = false
    }
    // Once per document.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc])

  // Then upstream's own fit (its margins and zoom), once the engine knows the view.
  const fitted = useRef<unknown>(null)
  useEffect(() => {
    // Only the first document: a newer version of the file keeps the person's view.
    if (!doc || !view || fitted.current !== null || viewMemory.current === null) return
    fitted.current = doc
    void designKit.viewTo('view.zoomFit')
  }, [doc, designKit, view])

  // Each page keeps its view: the view of the page shown is remembered as it changes, and a page
  // shown again goes back to where it was left (one shown for the first time keeps what the page
  // list does, Zoom to fit).
  const shownPage = useRef<{ doc: unknown; page: number } | null>(null)
  useLayoutEffect(() => {
    if (!doc || !view) return
    const was = shownPage.current
    shownPage.current = { doc, page: inspect.page }
    if (was && was.doc === doc && was.page !== inspect.page) {
      const back = rememberedPageView(doc, inspect.page)
      const handle = canvas.current
      if (back && handle) {
        handle.zoomTo(back.zoom)
        handle.panTo(back.x - inspect.area.x, back.y - inspect.area.y)
      }
      return
    }
    rememberPageView(doc, inspect.page, view, inspect.area)
  }, [canvas, doc, inspect.area, inspect.page, view])

  // The chrome, asked again after every change and zoom; during a gesture, and when the hovered
  // layer changes (the labels' colour), it comes with the engine's answer (`canvas.chrome`).
  const [chrome, setChrome] = useState<DesignChrome | null>(null)
  const zoom = view ? Math.round(view.zoom * 1000) / 1000 : null
  const asking = useRef<{ busy: boolean; again: boolean }>({ busy: false, again: false })
  const latestZoom = useRef(zoom)
  latestZoom.current = zoom
  const ask = useCallback(() => {
    const state = asking.current
    if (state.busy) {
      state.again = true
      return
    }
    const at = latestZoom.current
    if (at == null) return
    state.busy = true
    void invoke<string>('chrome', at)
      .then((text) => setChrome(JSON.parse(text) as DesignChrome))
      .catch(() => undefined)
      .finally(() => {
        state.busy = false
        if (state.again) {
          state.again = false
          ask()
        }
      })
  }, [invoke])
  useEffect(() => {
    if (doc && zoom != null) ask()
  }, [ask, doc, revision, zoom])
  useUiRequest('canvas.chrome', (data) => {
    if (data && typeof data === 'object') setChrome(data as DesignChrome)
  })

  // A double-click on a frame's or section's name label renames it there (the engine asks).
  const [renaming, setRenaming] = useState<number | null>(null)
  useUiRequest('canvas.renameLabel', (data) => {
    const id = (data as { id?: unknown } | null)?.id
    if (typeof id === 'number' && !designKit.readOnly) setRenaming(id)
  })

  // The auto layout band under the pointer, or being dragged.
  const [hot, setHot] = useState<ZoneKey>(null)
  const [redraws, setRedraws] = useState(0)
  const redraw = useCallback(() => setRedraws((n) => n + 1), [])
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const theme = useMemo(readTheme, [revision, document.documentElement.dataset.colorScheme])
  setSizePill(chrome?.sizeLabel ?? null)
  // Ruler guides: dragged out of the rulers, moved, removed.
  const guides = useRulerGuides(designKit, chrome)
  const hiddenGuide = guides.hidden
  // The chrome under the engine's overlays (grids, labels) and over them (bands, size pill, rulers).
  const decorateUnder = useMemo(() => (chrome ? chromeDecoration(chrome, theme, hot, redraw, 'under') : null), [chrome, theme, hot, redraw, redraws])
  const decorate = useMemo(() => (chrome ? chromeDecoration(chrome, theme, hot, redraw, 'over', hiddenGuide) : null), [chrome, theme, hot, redraw, redraws, hiddenGuide])
  const cursor = zoomTool.held !== null ? (zoomTool.held ? 'zoom-out' : 'zoom-in') : guides.cursor

  return (
    <div
      ref={root}
      className="design-canvas-root"
      data-cursor={cursor ?? undefined}
      style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}
      onContextMenu={(event) => {
        event.preventDefault()
        dispatchUiRequests([{ kind: 'contextMenu', data: { clientX: event.clientX, clientY: event.clientY, deep: event.metaKey || event.ctrlKey } }])
      }}
    >
      {fit ? (
        <KitDocumentCanvas canvas={canvas} viewMemory={viewMemory} initialArea={fit.area} framed={false} decorate={decorate} decorateUnder={decorateUnder} animated={wantsText} onView={onView} testId="design-canvas">
          {chrome?.autoLayout && view && activeTool === 'move' ? <AutoLayoutHandles chrome={chrome} view={view} hot={hot} onHot={setHot} /> : null}
          {renaming && view && chrome ? <LabelRename id={renaming} chrome={chrome} view={view} onDone={() => setRenaming(null)} /> : null}
          <CanvasOverlays />
          {view && designKit.ui.rightTab === 'prototype' ? <ProtoCanvasLayer view={view} /> : null}
          {view && designKit.ui.rightTab === 'inspect' ? <DevCanvasLayer view={view} /> : null}
          <DropHint hint={dropHint} designKit={designKit} />
          {chrome ? <GuidePreview place={guides.place} view={view} area={inspect.area} color={chrome.colors.guide} /> : null}
          <ZoomArea area={zoomTool.area} />
        </KitDocumentCanvas>
      ) : (
        <div className="design-canvas-message">{designKit.editor.session.failure ?? designKit.t('kit.opening')}</div>
      )}
    </div>
  )
}

/**
 * The field over a frame's or section's name label (upstream: double-click the label): the name
 * selected, Enter or a press elsewhere renames (`object.rename`, one undo step), Escape keeps the
 * name; the keyboard is the canvas's again after it.
 */
function LabelRename({ id, chrome, view, onDone }: { id: number; chrome: DesignChrome; view: KitCanvasView; onDone: () => void }) {
  const { run } = useDesignKit()
  const label = chrome.labels.find((item) => item.id === id)
  // The label went (the frame was deleted, or another page is shown): nothing to rename.
  useEffect(() => {
    if (!label) onDone()
  }, [label, onDone])
  if (!label) return null
  const xs = label.quad.map((point) => point[0])
  const ys = label.quad.map((point) => point[1])
  const left = (Math.min(...xs) - view.x) * view.zoom
  const top = (Math.min(...ys) - view.y) * view.zoom
  const width = Math.max(120, Math.min(label.maxWidth, 320), (Math.max(...xs) - Math.min(...xs)) * view.zoom + 24)
  return (
    <div onPointerDown={(event) => event.stopPropagation()} onDoubleClick={(event) => event.stopPropagation()}>
      <RenameInput
        value={label.name}
        style={{ left: left - 3, top: top - 3, width, height: 20 }}
        testId="design-label-rename"
        onDone={(name) => {
          onDone()
          if (name != null) void run('object.rename', { ids: [id], name })
          returnKeyboardLater()
        }}
      />
    </div>
  )
}
