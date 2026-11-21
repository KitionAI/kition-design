/**
 * The canvas: kit-editor's canvas rendering the engine (zoom and pan, wheel and pinch, the
 * engine's tools and overlays), the page's background behind the document, the canvas chrome
 * drawn over it (`chrome.ts`), and the auto layout padding and gap handles, which drag with
 * `node.set` as one undo step.
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { KitDocumentCanvas, type KitCanvasView, type RasterRect, type RasterView } from '@kition/sdk/editor'
import { useDesignKit, useView } from '../kit/hooks'
import { dispatchUiRequests } from '../kit/requests'
import type { DesignChrome } from '../kit/types'
import { chromeDecoration, readTheme, type ZoneKey } from './chrome'
import { CanvasOverlays } from './overlays'
import { setCanvasPage, setSizePill } from './engineOverlays'
import { contentBounds, raster } from './view'
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
  const { inspect, canvas, invoke, revision, tools, query, doc } = designKit
  const view = useView()
  usePageBackdrop(inspect.background)
  // The engine's overlays take the page's green and keep guide labels clear of the size pill.
  setCanvasPage(inspect.background)
  const viewMemory = useRef<RasterView | null>(null)

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

  // The chrome, asked again after every change, zoom, and tool answer (hover moves the labels' colour).
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
  }, [ask, doc, revision, zoom, tools.decorate])

  // The auto layout band under the pointer, or being dragged.
  const [hot, setHot] = useState<ZoneKey>(null)
  const [redraws, setRedraws] = useState(0)
  const redraw = useCallback(() => setRedraws((n) => n + 1), [])
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const theme = useMemo(readTheme, [revision, document.documentElement.dataset.colorScheme])
  setSizePill(chrome?.sizeLabel ?? null)
  // The chrome under the engine's overlays (grids, labels) and over them (bands, size pill, rulers).
  const decorateUnder = useMemo(() => (chrome ? chromeDecoration(chrome, theme, hot, redraw, 'under') : null), [chrome, theme, hot, redraw, redraws])
  const decorate = useMemo(() => (chrome ? chromeDecoration(chrome, theme, hot, redraw, 'over') : null), [chrome, theme, hot, redraw, redraws])

  return (
    <div
      style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}
      onContextMenu={(event) => {
        event.preventDefault()
        dispatchUiRequests([{ kind: 'contextMenu', data: { clientX: event.clientX, clientY: event.clientY, deep: event.metaKey || event.ctrlKey } }])
      }}
    >
      {fit ? (
        <KitDocumentCanvas canvas={canvas} viewMemory={viewMemory} initialArea={fit.area} framed={false} decorate={decorate} decorateUnder={decorateUnder} animated={tools.wantsText} onView={onView} testId="design-canvas">
          {chrome?.autoLayout && view && tools.active === 'move' ? <AutoLayoutHandles chrome={chrome} view={view} onHot={setHot} /> : null}
          <CanvasOverlays />
          {view && designKit.ui.rightTab === 'prototype' ? <ProtoCanvasLayer view={view} /> : null}
          {view && designKit.ui.rightTab === 'inspect' ? <DevCanvasLayer view={view} /> : null}
        </KitDocumentCanvas>
      ) : (
        <div className="design-canvas-message">{designKit.editor.session.failure ?? designKit.t('kit.opening')}</div>
      )}
    </div>
  )
}

/** The padding and gap bands (2.5 §4): invisible until hovered; dragging one sets the value. */
function AutoLayoutHandles({ chrome, view, onHot }: { chrome: DesignChrome; view: KitCanvasView; onHot: (key: ZoneKey) => void }) {
  const { interaction, t } = useDesignKit()
  const al = chrome.autoLayout!
  const dragging = useRef<string | null>(null)
  const zones = [
    ...al.padding.map((zone) => ({ key: `padding:${zone.side}`, prop: zone.key, value: zone.value, rect: zone.rect, axis: zone.side === 0 || zone.side === 2 ? ('y' as const) : ('x' as const), sign: zone.side === 0 || zone.side === 3 ? 1 : -1, gap: false, side: zone.side })),
    ...al.gaps.map((zone, index) => ({ key: `gap:${index}`, prop: zone.key, value: zone.value, rect: zone.rect, axis: zone.vertical ? ('y' as const) : ('x' as const), sign: 1, gap: true, side: -1 })),
  ]
  return (
    <>
      {zones.map((zone) => {
        const left = (zone.rect.x - view.x) * view.zoom
        const top = (zone.rect.y - view.y) * view.zoom
        return (
          <div
            key={zone.key}
            className="design-al-zone"
            data-testid={`design-al-${zone.key.replace(':', '-')}`}
            style={{ left, top, width: zone.rect.width * view.zoom, height: zone.rect.height * view.zoom, cursor: zone.axis === 'y' ? 'ns-resize' : 'ew-resize' }}
            onPointerEnter={() => onHot(zone.key)}
            onPointerLeave={() => {
              if (dragging.current !== zone.key) onHot(null)
            }}
            onPointerDown={(event) => {
              if (event.button !== 0) return
              event.preventDefault()
              event.stopPropagation()
              const target = event.currentTarget
              target.setPointerCapture(event.pointerId)
              dragging.current = zone.key
              const start = { x: event.clientX, y: event.clientY }
              const drag = interaction(t(zone.gap ? 'interaction.gap' : 'interaction.padding'))
              let last = zone.value
              const move = (e: PointerEvent) => {
                const delta = ((zone.axis === 'x' ? e.clientX - start.x : e.clientY - start.y) * zone.sign) / view.zoom
                const next = Math.max(0, Math.round(zone.value + delta))
                if (next === last) return
                last = next
                // Shift sets both sides of the axis.
                const key = !zone.gap && e.shiftKey ? (zone.axis === 'y' ? 'paddingVertical' : 'paddingHorizontal') : zone.prop
                drag.update('node.set', { ids: [al.id], props: { [key]: next } })
              }
              // A release commits; a press the browser cancels puts the padding back. Losing the
              // capture without either (the zone went away) keeps what the drag did.
              const end = (how: 'commit' | 'cancel') => () => {
                target.removeEventListener('pointermove', move)
                target.removeEventListener('pointerup', up)
                target.removeEventListener('pointercancel', cancel)
                target.removeEventListener('lostpointercapture', up)
                dragging.current = null
                onHot(null)
                void (how === 'commit' ? drag.commit() : drag.cancel())
              }
              const up = end('commit')
              const cancel = end('cancel')
              target.addEventListener('pointermove', move)
              target.addEventListener('pointerup', up)
              target.addEventListener('pointercancel', cancel)
              target.addEventListener('lostpointercapture', up)
            }}
          />
        )
      })}
    </>
  )
}
