/**
 * Always mounted: the prototype preview pane (upstream `present/chrome.rs` `show_pane`) and the
 * full-window presenter (`present.rs`, `present/chrome.rs`, `present/player.rs`). Present opens
 * its own document tab upstream; in the kit iframe it covers the whole editor. Both play with
 * `Player` (the engine's player). They open on the engine's `present` and `preview` UI requests
 * (`proto.present`, `proto.preview`), close on `preview.close` (`proto.closePreview`), and the
 * presenter's options come from `present.options` (`proto.presentOptions`).
 */
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useDesignKit, useSelection } from '../kit/hooks'
import { useUiRequest } from '../kit/requests'
import { Icon } from '../icons/Icon'
import { IconButton, Menu, type MenuEntry } from '../ui'
import { useThumbnail } from '../dev/widgets'
import { topLevelFrame, usePrototype, useTree } from './data'
import { CUSTOM_DEVICE, DEVICES } from './model'
import { PresentComments, PresentInspectPanel, usePresentInspect } from './PresentExtras'
import { openThread, useOpenComment } from '../comments/CommentsLayer'
import { Player, type PlayerApi, type PlayerFlow, type PlayerState, type Scale } from './Player'
import { closePlayer, openPlayer, useProtoState } from './store'
import './player.css'

type Options = { scale: Scale; deviceFrame: boolean; hotspotHints: boolean; inspect: boolean }

/** The presenter's options, as the last `present.options` request left them (kept for later presentations). */
let options: Options = { scale: 'fit', deviceFrame: true, hotspotHints: true, inspect: false }

/** Whether the presenter shows the comments (kept for later presentations, as upstream's `present_view`). */
let commentsShown = false

const SCALES: Scale[] = ['fit', 'fitWidth', 'fill', 'actual']

export function PrototypeHost() {
  const player = useProtoState((s) => s.player)
  const pane = useProtoState((s) => s.pane)
  const [, redraw] = useState(0)

  const startOf = (data: unknown) => {
    const start = (data as { start?: unknown } | null)?.start
    return typeof start === 'number' ? start : null
  }
  const proto = usePrototype()
  const tree = useTree()
  const selection = useSelection()
  useUiRequest('present', (data) => {
    // `proto.present` answers its start; a request without one (older callers) starts as it would.
    const selected = selection.ids[0] != null ? topLevelFrame(tree, selection.ids[0]) : null
    const start = startOf(data) ?? selected ?? proto?.flows[0]?.start ?? proto?.frames[0]?.id ?? null
    if (start != null) openPlayer('present', start)
  })
  useUiRequest('preview', (data) => openPlayer('preview', startOf(data)))
  useUiRequest('preview.close', () => closePlayer('preview'))
  useUiRequest('present.options', (data) => {
    const d = (data ?? {}) as Partial<Options>
    const next: Partial<Options> = {}
    if (typeof d.scale === 'string' && SCALES.includes(d.scale)) next.scale = d.scale
    if (typeof d.deviceFrame === 'boolean') next.deviceFrame = d.deviceFrame
    if (typeof d.hotspotHints === 'boolean') next.hotspotHints = d.hotspotHints
    if (typeof d.inspect === 'boolean') next.inspect = d.inspect
    options = { ...options, ...next }
    redraw((n) => n + 1)
  })
  return (
    <>
      {pane && !player ? <PreviewPane key={pane.key} start={pane.start} /> : null}
      {player ? <Presenter key={player.key} start={player.start} options={options} /> : null}
    </>
  )
}

/** The flow picker (flow name or "Flows", a menu of flows with their descriptions). */
function FlowPicker({ flows, current, onPick, size }: { flows: PlayerFlow[]; current: PlayerFlow | null; onPick: (flow: PlayerFlow) => void; size: 'pane' | 'full' }) {
  const { t } = useDesignKit()
  const button = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  if (!flows.length) return null
  const items: MenuEntry[] = flows.flatMap((flow) => [
    { label: flow.name, checked: current?.start === flow.start, onSelect: () => onPick(flow), testId: `design-proto-player-flow-${flow.start}` },
    ...(flow.description?.trim() ? [{ type: 'custom' as const, render: () => <div className="design-proto-flow-menu-description">{flow.description}</div> }] : []),
  ])
  return (
    <>
      <button ref={button} type="button" className="design-proto-flow-picker" data-size={size} aria-expanded={open} data-tip={current?.description?.trim() || t('Flows')} onClick={() => setOpen((now) => !now)} data-testid="design-proto-player-flows">
        <span>{current?.name ?? t('Flows')}</span>
        <Icon name="Chevron" size={17} />
      </button>
      {open ? <Menu tone="light" anchor={button.current} placement="below-start" minWidth={240} items={items} onClose={() => setOpen(false)} /> : null}
      <span className="design-proto-crumb-sep">/</span>
    </>
  )
}

/** Measures an element's content box. */
function useSize<T extends HTMLElement>() {
  const ref = useRef<T>(null)
  const [size, setSize] = useState({ width: 0, height: 0 })
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return undefined
    const observer = new ResizeObserver(() => setSize({ width: el.clientWidth, height: el.clientHeight }))
    observer.observe(el)
    setSize({ width: el.clientWidth, height: el.clientHeight })
    return () => observer.disconnect()
  }, [])
  return [ref, size] as const
}

/** The player's state and its controls, as the player reports them. */
function usePlaying() {
  const [playing, setPlaying] = useState<{ s: PlayerState | null; api: PlayerApi | null }>({ s: null, api: null })
  return [playing, (s: PlayerState | null, api: PlayerApi) => setPlaying({ s, api })] as const
}

/** The floating preview pane: 320 × 600 at the canvas's top right, resizable; it follows the selected top-level frame. */
function PreviewPane({ start: first }: { start: number | null }) {
  const { t, run } = useDesignKit()
  const tree = useTree()
  const selection = useSelection()
  const [start, setStart] = useState(first)
  const [playing, onState] = usePlaying()
  const [hot, setHot] = useState(false)
  const [area, size] = useSize<HTMLDivElement>()
  const [at, setAt] = useState<{ left: number; top: number } | null>(null)
  useLayoutEffect(() => {
    const canvas = document.querySelector('.design-canvas-cell')?.getBoundingClientRect()
    if (canvas) setAt({ left: Math.max(canvas.left, canvas.right - 336), top: canvas.top + 16 })
  }, [])
  // Selecting another top-level frame restarts the pane there.
  const selectedTop = selection.ids[0] != null ? topLevelFrame(tree, selection.ids[0]) : null
  useEffect(() => {
    if (selectedTop != null && selectedTop !== start) setStart(selectedTop)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedTop])
  const drag = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || (event.target instanceof Element && event.target.closest('button'))) return
    const from = { x: event.clientX, y: event.clientY, left: at?.left ?? 0, top: at?.top ?? 0 }
    const el = event.currentTarget
    el.setPointerCapture(event.pointerId)
    const move = (e: PointerEvent) => setAt({ left: Math.max(0, Math.min(window.innerWidth - 120, from.left + e.clientX - from.x)), top: Math.max(0, Math.min(window.innerHeight - 40, from.top + e.clientY - from.y)) })
    const up = () => {
      el.removeEventListener('pointermove', move)
      el.removeEventListener('pointerup', up)
    }
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerup', up)
  }
  const s = playing.s
  return createPortal(
    <div className="design-proto-pane design-layer" style={at ?? { left: -9999, top: -9999 }} role="dialog" aria-label={t('Open prototype preview')} data-testid="design-proto-pane" onPointerEnter={() => setHot(true)} onPointerLeave={() => setHot(false)}>
      <div className="design-proto-pane-bar" onPointerDown={drag}>
        <div className="design-proto-crumbs">
          <FlowPicker flows={s?.flows ?? []} current={s?.flow ?? null} size="pane" onPick={(f) => playing.api?.restart(f.start)} />
          <span className="design-proto-crumb-name" data-testid="design-proto-pane-frame">{s?.name ?? ''}</span>
        </div>
        <IconButton icon="Reset" label={t('Restart')} onClick={() => playing.api?.restart()} testId="design-proto-pane-restart" />
        <IconButton icon="Play" label={t('Present')} onClick={() => void run('proto.present', s?.current != null ? { start: s.current } : start != null ? { start } : {})} testId="design-proto-pane-present" />
        <IconButton icon="Close" label={t('Close preview')} onClick={() => void run('proto.closePreview', {})} testId="design-proto-pane-close" />
      </div>
      <div ref={area} className="design-proto-pane-area" style={{ background: s?.background }}>
        {start != null && size.width > 0 ? (
          <Player start={start} width={size.width} height={size.height} hotspotHints deviceFrame scaleMode="fit" pane keys={hot} onState={onState} testId="design-proto-pane-player" />
        ) : (
          <div className="design-proto-player-empty">{t('Select a frame to preview it')}</div>
        )}
      </div>
      <svg className="design-proto-pane-grip" viewBox="0 0 12 12" aria-hidden="true">
        <path d="M9 11L11 9M5 11L11 5M1 11L11 1" stroke="currentColor" strokeWidth="1" fill="none" />
      </svg>
    </div>,
    document.body,
  )
}

/** The scale options' labels, without and with a device (upstream `options_menu`). */
const SCALE_LABELS: Record<Scale, [string, string]> = {
  fit: ['Fit to window', 'Fit device to window'],
  fitWidth: ['Fit width', 'Fit width'],
  fill: ['Fill window', 'Zoom device to fill window'],
  actual: ['Actual size (100%)', 'Device at 100%'],
}

/** The full-window presenter: top bar, the player, the bottom navigation bar. */
function Presenter({ start, options: shown }: { start: number | null; options: Options }) {
  const { t, run } = useDesignKit()
  const proto = usePrototype()
  const [playing, onState] = usePlaying()
  const [menu, setMenu] = useState<'options' | 'device' | 'scale' | null>(null)
  const optionsButton = useRef<HTMLButtonElement>(null)
  const deviceButton = useRef<HTMLButtonElement>(null)
  const scaleButton = useRef<HTMLButtonElement>(null)
  const [area, size] = useSize<HTMLDivElement>()
  const s = playing.s
  const settings = proto?.settings ?? null
  const device = s?.device ?? null
  const responsive = s?.responsive ?? settings?.responsive === true
  const frames = proto?.frames ?? []
  const index = frames.findIndex((f) => f.id === s?.current)
  const thumb = useThumbnail(s?.current ?? null, 26)
  const close = () => closePlayer('present')
  const [comments, setCommentsState] = useState(commentsShown)
  const openComment = useOpenComment()
  const toggleComments = () => {
    commentsShown = !comments
    setCommentsState(commentsShown)
    if (!commentsShown) openThread(null)
  }
  const toggleRef = useRef(toggleComments)
  toggleRef.current = toggleComments
  const inspect = usePresentInspect(shown.inspect, s)
  const setOptions = (next: Partial<Options>) => void run('proto.presentOptions', next)
  // Restart goes to the start (the flow picked since, if any).
  const restart = () => playing.api?.restart()
  // Previous and Next frame step the top-level frames instantly: the engine's own arrow keys.
  const step = (by: number) => void playing.api?.key({ key: by > 0 ? 'ArrowRight' : 'ArrowLeft', shiftKey: false, ctrlKey: false, altKey: false, metaKey: false } as KeyboardEvent)
  // Z skips Fit width with a device, and does nothing while a responsive frame fills the window.
  const scales = device ? SCALES.filter((id) => id !== 'fitWidth') : SCALES
  const nextScale = () => {
    if (responsive && !device) return
    const i = scales.indexOf(shown.scale)
    setOptions({ scale: scales[(i + 1) % scales.length]! })
  }
  const openCommentRef = useRef(openComment)
  openCommentRef.current = openComment
  // Escape closes the open thread, else the presenter, even with a menu open (but not over the link notice).
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || document.querySelector('.design-proto-notice')) return
      event.preventDefault()
      event.stopPropagation()
      if (openCommentRef.current != null) openThread(null)
      else close()
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [])
  // Leaving the presenter closes a thread opened in it.
  useEffect(() => () => openThread(null), [])
  // The keys the prototype did not use (the engine's player already takes R, the arrows, Space and N): Z next size.
  const onKey = (event: KeyboardEvent) => {
    if (menu || event.metaKey || event.ctrlKey || event.altKey) return
    if (event.key === 'z' || event.key === 'Z') nextScale()
    // C shows the comments when the prototype does not use it.
    if (event.key === 'c' && !event.shiftKey) toggleRef.current()
  }
  // Shift+C shows the comments before the prototype sees the key.
  const onKeyFirst = (event: KeyboardEvent) => {
    if (menu || !event.shiftKey || event.metaKey || event.ctrlKey || event.altKey || event.key.toLowerCase() !== 'c') return false
    toggleRef.current()
    return true
  }
  const scaleLabel = (id: Scale) => t(SCALE_LABELS[id][device ? 1 : 0])
  const scaleItems = (testIds: boolean): MenuEntry[] => scales.map((id) => ({ label: scaleLabel(id), checked: shown.scale === id, onSelect: () => setOptions({ scale: id }), testId: testIds ? `design-proto-present-scale-${id}` : undefined }))
  const hasColors = device?.color != null
  const optionItems: MenuEntry[] = device
    ? [
        ...scaleItems(true),
        { label: t('Next size option'), checked: false, shortcut: 'Z', onSelect: nextScale },
        { type: 'separator' },
        { label: t('Responsive'), checked: responsive, onSelect: () => void run('proto.settings', { responsive: true }) },
        { label: t('Fixed size'), checked: !responsive, onSelect: () => void run('proto.settings', { responsive: false }) },
        { type: 'separator' },
        { label: t('Show device frame'), checked: shown.deviceFrame, onSelect: () => setOptions({ deviceFrame: !shown.deviceFrame }), testId: 'design-proto-present-device-frame' },
        { label: t('Show hotspot hints'), checked: shown.hotspotHints, onSelect: () => setOptions({ hotspotHints: !shown.hotspotHints }), testId: 'design-proto-present-hints' },
        { label: t('Restart'), checked: false, shortcut: 'R', onSelect: restart },
      ]
    : [
        // Without a device, Responsive is one of the sizes (the frame fills the window at 100%).
        { label: t('Responsive'), checked: responsive, onSelect: () => void run('proto.settings', { responsive: true }) },
        ...scales.map((id) => ({
          label: scaleLabel(id),
          checked: shown.scale === id && !responsive,
          onSelect: () => {
            setOptions({ scale: id })
            if (responsive) void run('proto.settings', { responsive: false })
          },
          testId: `design-proto-present-scale-${id}`,
        })),
        ...(responsive ? [] : [{ label: t('Next size option'), checked: false, shortcut: 'Z', onSelect: nextScale }]),
        { type: 'separator' },
        ...(hasColors ? [{ label: t('Show device frame'), checked: shown.deviceFrame, onSelect: () => setOptions({ deviceFrame: !shown.deviceFrame }), testId: 'design-proto-present-device-frame' }] : []),
        { label: t('Show hotspot hints'), checked: shown.hotspotHints, onSelect: () => setOptions({ hotspotHints: !shown.hotspotHints }), testId: 'design-proto-present-hints' },
        { label: t('Restart'), checked: false, shortcut: 'R', onSelect: restart },
      ]
  const deviceItems: MenuEntry[] = [
    { label: t('No device'), checked: !device, onSelect: () => void run('proto.settings', { device: 'None', responsive: false }) },
    { type: 'separator' },
    ...DEVICES.map((d) => ({ label: `${d.name}  ${d.width}×${d.height}`, checked: settings?.device === d.name, onSelect: () => void run('proto.settings', { device: d.name, responsive: true }), testId: `design-proto-present-device-${d.name}` })),
    { type: 'separator' },
    { label: t(CUSTOM_DEVICE), checked: settings?.device === CUSTOM_DEVICE, onSelect: () => void run('proto.settings', { device: CUSTOM_DEVICE, responsive: true }) },
  ]
  return createPortal(
    <div className="design-proto-present" role="dialog" aria-label={t('Present')} data-testid="design-proto-present">
      <div className="design-proto-present-main">
        <div className="design-proto-present-bar">
          <span className="design-proto-present-thumb">{thumb ? <img src={thumb} alt="" /> : null}</span>
          <div className="design-proto-crumbs">
            <FlowPicker
              flows={s?.flows ?? []}
              current={s?.flow ?? null}
              size="full"
              onPick={(f) => playing.api?.restart(f.start)}
            />
            <span className="design-proto-crumb-name" data-testid="design-proto-present-frame">{s?.name ?? ''}</span>
          </div>
          <div className="design-proto-present-actions">
            <button type="button" className="design-proto-text-button" onClick={close} data-testid="design-proto-present-close">
              {t('Close')}
            </button>
            <button type="button" className="design-proto-text-button" onClick={restart} data-testid="design-proto-present-restart">
              {t('Restart')}
            </button>
            <button ref={optionsButton} type="button" className="design-proto-text-button" aria-expanded={menu === 'options'} onClick={() => setMenu(menu === 'options' ? null : 'options')} data-testid="design-proto-present-options">
              {t('Options')}
            </button>
            <button type="button" className="design-proto-text-button" aria-pressed={comments} data-tip={t('Show comments (Shift+C)')} onClick={toggleComments} data-testid="design-proto-present-comments">
              {t('Comments')}
            </button>
            <button type="button" className="design-proto-text-button" aria-pressed={shown.inspect} data-tip={t('Inspect the layer under the pointer: size, colours, typography')} onClick={() => setOptions({ inspect: !shown.inspect })} data-testid="design-proto-present-inspect-button">
              {t('Inspect')}
            </button>
          </div>
        </div>
        <div ref={area} className="design-proto-present-area" style={{ background: s?.background }}>
          {start != null && size.width > 0 ? (
            <Player
              start={start}
              width={size.width}
              height={size.height}
              hotspotHints={shown.hotspotHints}
              deviceFrame={shown.deviceFrame}
              scaleMode={shown.scale}
              keys
              onKey={onKey}
              onKeyFirst={onKeyFirst}
              onState={onState}
              onPointer={inspect.onPointer}
              layer={({ scale, state }) => (
                <>
                  {inspect.outline(scale)}
                  {comments ? <PresentComments s={state} scale={scale} /> : null}
                </>
              )}
              testId="design-proto-present-player"
            />
          ) : null}
        </div>
      </div>
      {/* Centred on the window, as upstream's navigation area (the Inspect panel included). */}
      <div className="design-proto-present-nav">
        <button type="button" className="design-proto-nav-step" disabled={index <= 0} data-tip={t('Previous frame')} aria-label={t('Previous frame')} onClick={() => step(-1)} data-testid="design-proto-present-prev">
          <Icon name="ChevronLeft" size={21} />
        </button>
        <span className="design-proto-nav-count" data-testid="design-proto-present-count">{index >= 0 ? `${index + 1} / ${frames.length}` : `– / ${frames.length}`}</span>
        <button type="button" className="design-proto-nav-step" disabled={index < 0 || index >= frames.length - 1} data-tip={t('Next frame')} aria-label={t('Next frame')} onClick={() => step(1)} data-testid="design-proto-present-next">
          <Icon name="ChevronRight" size={21} />
        </button>
        <span className="design-proto-nav-separator" />
        <button ref={deviceButton} type="button" className="design-proto-nav-menu" aria-expanded={menu === 'device'} onClick={() => setMenu(menu === 'device' ? null : 'device')} data-testid="design-proto-present-device">
          <span>{`${device ? device.name : t('No device')}    `}</span>
          <Icon name="Chevron" size={17} />
        </button>
        {responsive && !device ? null : (
          <button ref={scaleButton} type="button" className="design-proto-nav-menu" aria-expanded={menu === 'scale'} onClick={() => setMenu(menu === 'scale' ? null : 'scale')} data-testid="design-proto-present-scale">
            <span>{`${scaleLabel(shown.scale)}    `}</span>
            <Icon name="Chevron" size={17} />
          </button>
        )}
      </div>
      {shown.inspect ? <PresentInspectPanel inspected={inspect.inspected} /> : null}
      {menu === 'options' ? <Menu tone="light" anchor={optionsButton.current} placement="below-end" gap={1} minWidth={220} items={optionItems} onClose={() => setMenu(null)} testId="design-proto-present-options-menu" /> : null}
      {menu === 'device' ? <Menu tone="light" anchor={deviceButton.current} placement="above-start" minWidth={212} items={deviceItems} onClose={() => setMenu(null)} testId="design-proto-present-device-menu" /> : null}
      {menu === 'scale' ? <Menu tone="light" anchor={scaleButton.current} placement="above-start" minWidth={200} items={scaleItems(false)} onClose={() => setMenu(null)} testId="design-proto-present-scale-menu" /> : null}
    </div>,
    document.body,
  )
}
