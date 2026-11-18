/**
 * The editor's layout (2.1 §1): the left rail, the left panel, the canvas with the floating
 * toolbar, help button and toast, and the right panel with its header; resizable panel edges
 * (`fitPanels`), keyboard regions (F6), and the UI requests the shell answers (panels, Dev Mode).
 */
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react'
import { useFrameMode, type KitCanvasView } from '@kition/kit-editor'
import { DesignCanvas } from '../canvas/DesignCanvas'
import { useDesignKit } from '../kit/hooks'
import { regionKeyDown, regionProps } from '../kit/regions'
import { useUiRequest } from '../kit/requests'
import { LeftPanel } from '../left/LeftPanel'
import { LeftHost } from '../left/LeftHost'
import { DesignPanel } from '../design/DesignPanel'
import { PrototypePanel } from '../prototype/PrototypePanel'
import { DevPanel } from '../dev/DevPanel'
import { MenusHost } from '../menus/MenusHost'
import { CommentsPanel } from '../comments/CommentsPanel'
import { DevHost } from '../dev/DevHost'
import { VersionHistory, useVersionHistoryOpen } from '../dev/VersionHistory'
import { PrototypeHost } from '../prototype/PrototypeHost'
import { ToastHost, TooltipLayer } from '../ui'
import { ScrollBar } from '../ui/ScrollBar'
import { HelpButton } from './HelpButton'
import { LeftRail } from './LeftRail'
import { PanelEdge } from './PanelEdge'
import { RightHeader } from './RightHeader'
import { Toolbar } from './Toolbar'
import { PANEL_DEFAULT, fitPanels } from './layout'

const WIDTHS_KEY = 'kit.design.panelWidths'

function storedWidths() {
  try {
    const value = JSON.parse(localStorage.getItem(WIDTHS_KEY) ?? 'null') as { left?: unknown; right?: unknown } | null
    const pick = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? v : PANEL_DEFAULT)
    return { left: pick(value?.left), right: pick(value?.right) }
  } catch {
    return { left: PANEL_DEFAULT, right: PANEL_DEFAULT }
  }
}

export function Shell({ onView }: { onView: (view: KitCanvasView) => void }) {
  const designKit = useDesignKit()
  const panelScroll = useRef<HTMLDivElement>(null)
  const { ui, t } = designKit
  const root = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(1440)
  const [asked, setAsked] = useState(storedWidths)
  const focus = useFrameMode() === 'focus'
  const versions = useVersionHistoryOpen()

  useLayoutEffect(() => {
    const el = root.current
    if (!el) return undefined
    const observer = new ResizeObserver(() => setWidth(el.clientWidth))
    observer.observe(el)
    setWidth(el.clientWidth)
    return () => observer.disconnect()
  }, [])

  const widths = fitPanels(width, asked, ui.leftOpen)
  const ask = (next: { left: number; right: number }, done: boolean) => {
    const fitted = fitPanels(width, next, ui.leftOpen)
    const kept = { left: ui.leftOpen ? fitted.left : next.left, right: fitted.right }
    setAsked(kept)
    if (done) {
      try {
        localStorage.setItem(WIDTHS_KEY, JSON.stringify(kept))
      } catch {
        // No storage in this frame: the widths last for the session.
      }
    }
  }
  const resize = (side: 'left' | 'right') => (value: number, done: boolean) => ask({ ...asked, [side]: value }, done)
  // View ▸ Panels ▸ Reset panel widths (`view.panelWidths`): the widths it names, else both back to the default.
  useUiRequest('panel.widths', (data) => {
    const given = (data ?? {}) as { left?: unknown; right?: unknown }
    const pick = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? v : null)
    const left = pick(given.left)
    const right = pick(given.right)
    const neither = left === null && right === null
    ask({ left: left ?? (neither ? PANEL_DEFAULT : asked.left), right: right ?? (neither ? PANEL_DEFAULT : asked.right) }, true)
  })

  // The engine asks for panels (its View commands and shortcuts only request UI).
  useUiRequest('panel.devMode', () => ui.setRightTab(ui.rightTab === 'inspect' ? 'design' : 'inspect'))
  useUiRequest('panel.designPrototype', () => ui.setRightTab(ui.rightTab === 'design' ? 'prototype' : 'design'))
  useUiRequest('panel.design', () => ui.setRightTab('design'))
  useUiRequest('panel.prototype', () => ui.setRightTab('prototype'))
  useUiRequest('panel.leftSidebar', () => ui.setLeftOpen(!ui.leftOpen))
  useUiRequest('panel.layers', () => {
    ui.setLeftTab('file')
    ui.setLeftOpen(true)
  })
  useUiRequest('panel.assets', () => {
    ui.setLeftTab('assets')
    ui.setLeftOpen(true)
  })
  useUiRequest('palette.actions', () => ui.setActions(true))

  // Escape and a press move the keyboard region; the region system handles F6.
  useEffect(() => {
    const el = root.current
    if (!el) return undefined
    const down = (event: PointerEvent) => {
      const region = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-design-region]')?.dataset.designRegion : undefined
      el.dataset.region = region ?? 'canvas'
    }
    el.addEventListener('pointerdown', down, true)
    return () => el.removeEventListener('pointerdown', down, true)
  }, [])

  const keyDownCapture = (event: KeyboardEvent<HTMLDivElement>) => {
    if (regionKeyDown(root.current, event)) {
      event.preventDefault()
      event.stopPropagation()
    }
  }

  const hidden = focus || ui.uiHidden
  const leftShown = ui.leftOpen && !hidden
  // The Comment tool shows the Comments panel in place of the Design panel (2.5 §17).
  const body = ui.rightTab === 'prototype' ? <PrototypePanel /> : ui.rightTab === 'inspect' ? <DevPanel /> : designKit.inspect.tool === 'comment' ? <CommentsPanel /> : <DesignPanel />
  return (
    <div
      ref={root}
      className="design-shell"
      data-left={leftShown ? 'open' : 'closed'}
      data-ui={hidden ? 'hidden' : undefined}
      data-tab={ui.rightTab}
      style={{ ['--design-left' as string]: `${widths.left}px`, ['--design-right' as string]: `${widths.right}px` }}
      onKeyDownCapture={keyDownCapture}
      data-testid="design-shell"
    >
      <nav className="design-rail" {...regionProps('layers', t('Sidebar'))} data-testid="design-rail">
        <LeftRail />
      </nav>
      <aside className="design-left" {...regionProps('layers', t('Layers'))} aria-label={t(ui.leftTab === 'assets' ? 'Assets' : 'Layers')}>
        {leftShown ? versions ? <VersionHistory /> : <LeftPanel /> : null}
      </aside>
      <main className="design-canvas-cell" {...regionProps('canvas', t('region.canvas'))}>
        <DesignCanvas onView={onView} />
        {hidden ? null : <Toolbar />}
        {hidden ? null : <HelpButton />}
        <DevHost />
        <ToastHost />
        <MenusHost />
        <LeftHost />
      </main>
      <aside className="design-right" {...regionProps('properties', t(ui.rightTab === 'prototype' ? 'Prototype' : ui.rightTab === 'inspect' ? 'Dev Mode' : 'Design'))} data-testid="design-right">
        <RightHeader />
        <div ref={panelScroll} className="design-panel-scroll design-scroll-hidden">{body}</div>
        <ScrollBar target={panelScroll} testId="design-right-scrollbar" />
      </aside>
      {leftShown ? <PanelEdge x={56 + widths.left} side="left" width={widths.left} onWidth={resize('left')} label={t('panel.resize')} testId="design-edge-left" /> : null}
      {hidden ? null : <PanelEdge x={width - widths.right} side="right" width={widths.right} onWidth={resize('right')} label={t('panel.resize')} testId="design-edge-right" />}
      <PrototypeHost />
      <TooltipLayer />
    </div>
  )
}
