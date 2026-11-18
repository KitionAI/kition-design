/**
 * The prototype player the preview pane and the presenter show (2.6, upstream `present/*`), on
 * the engine's runtime: `player(command, params)` runs the prototype (navigation, overlays,
 * variables, Change to, Scroll to, scrolling, timers, hover and press states) on its own copy of
 * the document, and `playerRender(params)` draws its screen, a transition at any point included
 * (smart animate, dissolve, move, push, slide). This component only places the screen (the scale
 * options, the device body), forwards pointer, wheel and key input, plays transitions frame by
 * frame, ticks the engine's timers at `nextDue`, flashes hotspot hints, and asks before leaving
 * the prototype for a link (bindings/wasm/src/player.rs).
 *
 * The engine has one player: the preview pane and the presenter never play at once (the pane
 * hides while the presenter shows).
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { useDesignKit } from '../kit/hooks'
import { ease, placement as transitionPlacement, type EasingJson } from './easing'

export type PlayerFlow = { start: number; name: string; description?: string | null }
export type PlayerDevice = { name: string; width: number; height: number; landscape: boolean; rotates: boolean; color: { name: string; rgb: string } | null }
type Box = { x: number; y: number; width: number; height: number }
export type PlayerHotspot = Box & { id: number; name: string | null; triggers: string[] }
export type PlayerOverlay = Box & { id: number; name: string | null; background: string | null; closeOnOutside: boolean; position: string }

/** `player(…).state` (screen units). */
export type PlayerState = {
  start: number
  current: number
  name: string | null
  flow: PlayerFlow | null
  flows: PlayerFlow[]
  screen: { width: number; height: number } | null
  frame: Box | null
  device: PlayerDevice | null
  background: string
  responsive: boolean
  history: number[]
  canBack: boolean
  overlays: PlayerOverlay[]
  scroll: [number, number]
  scrollRange: [number, number]
  hotspots: PlayerHotspot[]
  hovered: number[]
  pressed: boolean
  waiting: number
  nextDue: number | null
  clock: number
  revision: string | number
}

export type PlayerTransition = {
  seq: number
  kind: string
  from: number
  to: number
  overlay: number | null
  transition: { type: string; direction?: string }
  smartAnimate: boolean
  duration: number
  durationMs: number
  easing: unknown
  follow: { x: number; y: number; length: number } | null
}

type Answer = { handled: boolean; changed: boolean; missed: boolean; cursor: string; links: string[]; transition: PlayerTransition | null; state: PlayerState }

export type PlayerApi = {
  state: PlayerState | null
  /** Back to `start` (default the frame the player started at) with the document as it is. */
  restart: (start?: number) => void
  /** Closes the top overlay, else goes back one screen. */
  back: () => void
  /** Whether the last key reached the prototype and it used it (the presenter's own keys wait for it). */
  key: (event: KeyboardEvent) => Promise<boolean>
}

export type Scale = 'fit' | 'fitWidth' | 'fill' | 'actual'

export type PlayerProps = {
  start: number | null
  /** The area the player fills, in CSS pixels. */
  width: number
  height: number
  hotspotHints: boolean
  deviceFrame: boolean
  scaleMode: Scale
  /** The preview pane: always Fit, 12 px padding, always the device body. */
  pane?: boolean
  /** Keys go to the prototype (the presenter; the pane only while the pointer is over it). */
  keys: boolean
  /** The presenter's own keys, for the keys the prototype did not use. */
  onKey?: (event: KeyboardEvent) => void
  /** The player tells its host what shows. */
  onState?: (state: PlayerState | null, api: PlayerApi) => void
  /** Keys the host takes before the prototype (true: used). */
  onKeyFirst?: (event: KeyboardEvent) => boolean
  /** The pointer over the screen (screen units), or null when it leaves. */
  onPointer?: (point: { x: number; y: number } | null) => void
  /** Drawn over the screen, unclipped, in CSS pixels from the screen's top left (`scale` CSS pixels per screen unit). */
  layer?: (view: { scale: number; state: PlayerState }) => ReactNode
  testId?: string
}

const now = () => performance.now() / 1000

/** A transition's two pictures, composited in the page (`px`: pixels per screen unit of each). */
type Composite = { from: HTMLCanvasElement; fromPx: number; to: HTMLCanvasElement; toPx: number; transition: PlayerTransition['transition']; easing: EasingJson; background: string }

/** A transition playing: its clock, the On drag pointer, and its composited pictures once rendered. */
type Anim = { seq: number; begin: number; ms: number; from: number; follow: PlayerTransition['follow']; down: { x: number; y: number } | null; progress: number; comp: Composite | null; dirty: boolean }

/** Transitions the engine draws frame by frame (the screen's parts move, not whole screens). */
const NO_COMPOSITE = new Set(['overlayOpen', 'overlayClose', 'scroll'])

/** The player that last started: an unmounting player stops the engine's only if it is still its own. */
let owner = 0

/** Width of a device's screen held upright, its bezel and its screen's corner radius (upstream `layout::bezel`). */
function bezel(device: string, w: number): [number, number] {
  if (device === 'MacBook Air') return [Math.max(6, w * 0.025), 0]
  if (device === 'Desktop') return [Math.max(6, w * 0.02), 0]
  if (device.startsWith('iPad')) return [Math.max(8, w * 0.05), w * 0.04]
  return [Math.max(6, w * 0.06), w * 0.13]
}

/** How far the body reaches past the screen: [each side, top, bottom] (upstream `body_outset`). */
function bodyOutset(device: string, w: number, h: number, landscape: boolean): [number, number, number] {
  const up = landscape ? h : w
  const [b] = bezel(device, up)
  if (device === 'MacBook Air') return [b + up * 0.08, b, b + Math.max(6, up * 0.022)]
  if (device === 'Desktop') return [b, b, b + up * 0.08 + 6]
  return landscape ? [b, b + 3, b + 3] : [b + 3, b, b]
}

/** A colour lighter (k > 0) or darker (k < 0), as upstream's `device::shade`. */
function shade(hex: string, k: number) {
  const n = parseInt(hex.replace('#', ''), 16)
  const f = (c: number) => Math.round(Math.min(255, Math.max(0, k >= 0 ? c + (255 - c) * k : c * (1 + k))))
  return `rgb(${f((n >> 16) & 255)}, ${f((n >> 8) & 255)}, ${f(n & 255)})`
}

type Placement = { scale: number; screen: { width: number; height: number }; whole: { width: number; height: number }; offset: { x: number; y: number }; body: PlayerDevice | null; pad: number }

/** Where the screen goes in the area (upstream `present/layout.rs` `layout`). */
export function place(state: PlayerState, area: { width: number; height: number }, scaleMode: Scale, deviceFrame: boolean, pane: boolean): Placement | null {
  if (!state.screen) return null
  const device = state.device
  const fills = state.responsive && !device
  const mode: Scale = pane ? 'fit' : scaleMode
  const body = device && (pane || deviceFrame) && device.color ? device : null
  const pad = !fills && (mode === 'fit' || mode === 'actual') ? (pane ? 12 : 32) : 0
  const avail = { width: Math.max(1, area.width - 2 * pad), height: Math.max(1, area.height - 2 * pad) }
  const { width: sw, height: sh } = state.screen
  const outer = (scale: number) => {
    const w = sw * scale
    const h = sh * scale
    const [x, top, bottom] = body ? bodyOutset(body.name, w, h, body.landscape) : [0, 0, 0]
    return { w, h, x, top, bottom, whole: { width: w + 2 * x, height: h + top + bottom } }
  }
  const factor = (o: { width: number; height: number }) => {
    const kx = avail.width / Math.max(1, o.width)
    const ky = avail.height / Math.max(1, o.height)
    return mode === 'fit' ? Math.min(kx, ky) : mode === 'fitWidth' ? kx : mode === 'fill' ? Math.max(kx, ky) : 1
  }
  let scale = fills ? 1 : factor({ width: sw, height: sh })
  if (body) scale *= factor(outer(scale).whole)
  const max = mode === 'fit' ? 1 : 16
  scale = Number.isFinite(scale) ? Math.min(max, Math.max(0.01, scale)) : 0.01
  const o = outer(scale)
  return { scale, screen: { width: o.w, height: o.h }, whole: o.whole, offset: { x: o.x, y: o.top }, body, pad }
}

/** The device body behind the screen and the glass over it (upstream `device::device_body`, `device_glass`). */
function DeviceBody({ device, w, h }: { device: PlayerDevice; w: number; h: number }) {
  const color = device.color?.rgb ?? '#38393d'
  const body = shade(color, 0)
  const edge = shade(color, -0.3)
  const metal = shade(color, 0.12)
  const up = device.landscape ? h : w
  const [b, screenR] = bezel(device.name, up)
  const band = Math.min(Math.max(1.5, up * 0.016), Math.max(1.5, b * 0.5))
  const at = (x: number, y: number, width: number, height: number, style: CSSProperties): CSSProperties => ({ position: 'absolute', left: x, top: y, width, height, ...style })
  const parts: CSSProperties[] = []
  if (device.name === 'MacBook Air') {
    parts.push(at(-b, -b, w + 2 * b, h + 2 * b, { background: body, borderRadius: b * 1.2 }))
    parts.push(at(-b + band, -b + band, w + 2 * b - 2 * band, h + 2 * b - 2 * band, { background: '#161618', borderRadius: Math.max(0, b * 1.2 - band) }))
    const baseH = Math.max(6, up * 0.022)
    parts.push(at(-b - up * 0.08, h + b, w + 2 * b + up * 0.16, baseH, { background: metal, borderRadius: '2px 2px 10px 10px' }))
    parts.push(at(w / 2 - up * 0.06, h + b, up * 0.12, 3, { background: edge, borderRadius: 2 }))
  } else if (device.name === 'Desktop') {
    parts.push(at(-b, -b, w + 2 * b, h + 2 * b, { background: body, borderRadius: 8 }))
    parts.push(at(-b + band, -b + band, w + 2 * b - 2 * band, h + 2 * b - 2 * band, { background: '#161618', borderRadius: Math.max(0, 8 - band) }))
    parts.push(at(w / 2 - up * 0.06, h + b, up * 0.12, up * 0.08, { background: metal }))
    parts.push(at(w / 2 - up * 0.12, h + b + up * 0.08, up * 0.24, 6, { background: edge, borderRadius: 3 }))
  } else {
    const slab = { x: -b, y: -b, w: w + 2 * b, h: h + 2 * b }
    if (!device.name.startsWith('iPad')) {
      for (const [left, f0, len] of [[true, 0.18, 0.06], [true, 0.27, 0.1], [false, 0.24, 0.13]] as const) {
        if (device.landscape) parts.push(at(slab.x + slab.w * f0, left ? slab.y + slab.h - 0.5 : slab.y - 2.5, slab.w * len, 2.5, { background: edge, borderRadius: 1 }))
        else parts.push(at(left ? slab.x - 2 : slab.x + slab.w, slab.y + slab.h * f0, 2.5, slab.h * len, { background: edge, borderRadius: 1 }))
      }
    }
    parts.push(at(slab.x, slab.y, slab.w, slab.h, { background: body, borderRadius: screenR + b, boxShadow: `inset 0 0 0 1px ${edge}` }))
    const glass = b - band
    parts.push(at(slab.x + band, slab.y + band, slab.w - 2 * band, slab.h - 2 * band, { background: '#161618', borderRadius: Math.min(screenR + glass, glass * 3.4) }))
  }
  return (
    <div className="design-proto-device-body" aria-hidden="true" data-testid="design-proto-device-body">
      {parts.map((style, i) => (
        <span key={i} style={style} />
      ))}
    </div>
  )
}

/** The camera pill over an iPhone's screen. */
function Camera({ device, w, h }: { device: PlayerDevice; w: number; h: number }) {
  if (!device.name.startsWith('iPhone')) return null
  const up = device.landscape ? h : w
  const style: CSSProperties = device.landscape
    ? { left: up * 0.045 - up * 0.0425, top: h / 2 - up * 0.15, width: up * 0.085, height: up * 0.3 }
    : { left: w / 2 - up * 0.15, top: up * 0.045 - up * 0.0425, width: up * 0.3, height: up * 0.085 }
  return <span className="design-proto-camera" style={{ ...style, borderRadius: up * 0.0425 }} aria-hidden="true" />
}

/** The engine's screen corner radius for a device with a body. */
export const screenRadius = (device: PlayerDevice | null, w: number, h: number) => (device ? Math.min(bezel(device.name, device.landscape ? h : w)[1], w / 2, h / 2) : 0)

/** Keys that never go to the prototype (modifiers alone). */
const MODIFIERS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'CapsLock', 'Fn', 'OS', 'Hyper', 'Super'])

/** "Don't show this again" for this session too (the engine keeps it as a preference it does not read back). */
let linkNoticeSkipped = false

export function Player({ start, width, height, hotspotHints, deviceFrame, scaleMode, pane = false, keys, onKey, onState, onKeyFirst, onPointer, layer, testId }: PlayerProps) {
  const { invoke, revision, t, editor } = useDesignKit()
  const [state, setState] = useState<PlayerState | null>(null)
  const [failed, setFailed] = useState<string | null>(null)
  const [cursor, setCursor] = useState('default')
  const [flash, setFlash] = useState(0)
  const [notice, setNotice] = useState<string | null>(null)
  const canvas = useRef<HTMLCanvasElement>(null)
  const me = useRef(0)
  const latest = useRef<PlayerState | null>(null)
  const timer = useRef<number | null>(null)
  const anim = useRef<Anim | null>(null)
  const ratio = typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1
  const downAt = useRefLess()
  const hintsRef = useRef(hotspotHints)
  hintsRef.current = hotspotHints

  const placement = useMemo(() => (state ? place(state, { width, height }, scaleMode, deviceFrame, pane) : null), [deviceFrame, height, pane, scaleMode, state, width])
  const placeRef = useRef(placement)
  placeRef.current = placement

  // --- Rendering: one picture at a time, the newest wish wins.
  const drawing = useRef(false)
  const wish = useRef<{ progress?: number; k: number } | null>(null)
  /** The player revision the canvas shows at full resolution (null while it shows anything else). */
  const drawnRevision = useRef<string | number | null>(null)
  /** What the engine's last picture cost per pixel (ms), to size in-between pictures to the frame budget. */
  const cost = useRef(0)

  /** Pixels per screen unit at placement `p`, inside the engine's picture caps (8192 a side, 32 M pixels). */
  const pixelsPerUnit = useCallback(
    (s: PlayerState, p: Placement, k = 1) => {
      const { width: sw, height: sh } = s.screen ?? { width: 1, height: 1 }
      let px = Math.min(8, Math.max(0.01, p.scale * ratio * k))
      px = Math.min(px, 8192 / Math.max(1, sw), 8192 / Math.max(1, sh), Math.sqrt((32 * 1024 * 1024) / Math.max(1, sw * sh)))
      return px
    },
    [ratio],
  )

  /** One engine picture as image data (`params` as `playerRender`'s, at `px` pixels per unit). */
  const picture = useCallback(
    async (s: PlayerState, px: number, params: Record<string, unknown>): Promise<ImageData | null> => {
      const began = performance.now()
      const bytes = await invoke<Uint8Array>('playerRender', JSON.stringify({ ...params, scale: px, format: 'rgba' })).catch(() => null)
      if (!bytes || me.current !== owner || bytes.length < 8) return null
      // The engine's picture says its size (two little-endian u32) before its pixels.
      const head = new DataView(bytes.buffer, bytes.byteOffset, 8)
      const w = head.getUint32(0, true)
      const h = head.getUint32(4, true)
      if (w <= 0 || h <= 0 || bytes.length < 8 + w * h * 4) return null
      cost.current = (performance.now() - began) / Math.max(1, w * h)
      return new ImageData(new Uint8ClampedArray(bytes.buffer as ArrayBuffer, bytes.byteOffset + 8, w * h * 4), w, h)
    },
    [invoke],
  )

  /** Shows `image` on the canvas (frame `current`). */
  const show = useCallback((image: ImageData | HTMLCanvasElement, current: number) => {
    const el = canvas.current
    if (!el) return
    if (el.width !== image.width) el.width = image.width
    if (el.height !== image.height) el.height = image.height
    const ctx = el.getContext('2d')
    if (image instanceof ImageData) ctx?.putImageData(image, 0, 0)
    else {
      ctx?.clearRect(0, 0, el.width, el.height)
      ctx?.drawImage(image, 0, 0)
    }
    el.dataset.frame = String(current)
    el.dataset.drawn = String(Number(el.dataset.drawn ?? 0) + 1)
  }, [])

  /** Draws the screen (`progress` < 1: the transition at that point; `k` < 1: at a lower resolution). */
  const draw = useCallback(
    async (progress?: number, k = 1) => {
      wish.current = { progress, k }
      if (drawing.current) return
      drawing.current = true
      try {
        while (wish.current) {
          const next = wish.current
          wish.current = null
          const s = latest.current
          const p = placeRef.current
          if (!s?.screen || !p || !canvas.current || me.current !== owner) continue
          const partial = next.progress != null && next.progress < 1
          const image = await picture(s, pixelsPerUnit(s, p, next.k), partial ? { progress: next.progress } : {})
          if (!image || me.current !== owner) continue
          show(image, s.current)
          drawnRevision.current = !partial && next.k === 1 && latest.current === s ? s.revision : null
        }
      } finally {
        drawing.current = false
      }
    },
    [picture, pixelsPerUnit, show],
  )

  /** A transition composited in the page from two pictures (every kind but overlays, scrolling and smart animate). */
  const composite = useCallback((c: Composite, progress: number) => {
    const el = canvas.current
    if (!el) return
    const { to, from } = c
    const w = to.width
    const h = to.height
    if (el.width !== w) el.width = w
    if (el.height !== h) el.height = h
    const ctx = el.getContext('2d')
    if (!ctx) return
    const pl = transitionPlacement(c.transition, ease(c.easing, progress))
    ctx.globalAlpha = 1
    ctx.fillStyle = c.background
    ctx.fillRect(0, 0, w, h)
    const k = c.toPx / Math.max(1e-6, c.fromPx)
    const put = (image: HTMLCanvasElement, off: [number, number], alpha: number, scale: number) => {
      if (alpha <= 0) return
      ctx.globalAlpha = Math.min(1, alpha)
      ctx.drawImage(image, Math.round(off[0] * w), Math.round(off[1] * h), image.width * scale, image.height * scale)
    }
    if (pl.fromOnTop) {
      put(to, pl.toOff, pl.toAlpha, 1)
      put(from, pl.fromOff, pl.fromAlpha, k)
    } else {
      put(from, pl.fromOff, pl.fromAlpha, k)
      put(to, pl.toOff, pl.toAlpha, 1)
    }
    ctx.globalAlpha = 1
    el.dataset.drawn = String(Number(el.dataset.drawn ?? 0) + 1)
    el.dataset.composited = String(Number(el.dataset.composited ?? 0) + 1)
  }, [])

  /**
   * Plays a transition. Most kinds render the screens before and after once, at full resolution,
   * and composite them every animation frame as the engine's `transition` describes (offsets and
   * opacities over the eased progress). Overlays, scrolling and smart animate ask the engine for
   * every frame, at a resolution that fits the frame budget, then draw the end at full resolution.
   */
  const animate = useCallback(
    (tr: PlayerTransition, before: PlayerState | null) => {
      const ms = tr.durationMs
      const previous = anim.current
      if (!(ms > 0) || (tr.transition.type === 'instant' && tr.kind !== 'scroll')) {
        anim.current = null
        void draw()
        return
      }
      const a: Anim = { seq: tr.seq, begin: performance.now(), ms, from: 0, follow: tr.follow, down: null, progress: 0, comp: null, dirty: false }
      anim.current = a
      // In-between engine pictures: as many pixels as about 24 ms of rendering allows.
      const s0 = latest.current
      const p0 = placeRef.current
      const full = s0?.screen && p0 ? (s0.screen.width * pixelsPerUnit(s0, p0)) * (s0.screen.height * pixelsPerUnit(s0, p0)) : 0
      const k = cost.current > 0 && full > 0 ? Math.min(1, Math.max(0.3, Math.sqrt(24 / (cost.current * full)))) : 0.5
      const frame = async (p: number) => {
        if (a.comp) composite(a.comp, p)
        else await draw(p, k)
      }
      const finish = () => {
        if (anim.current !== a) return
        anim.current = null
        const s = latest.current
        if (a.comp && !a.dirty && s && canvas.current) {
          show(a.comp.to, s.current)
          drawnRevision.current = s.revision
        } else void draw()
      }
      const step = async () => {
        if (anim.current !== a) return
        // An On drag transition follows the pointer while it is down.
        if (a.follow && latest.current?.pressed && a.down) {
          await frame(a.progress)
          requestAnimationFrame(() => void step())
          return
        }
        const p = Math.min(1, a.from + ((performance.now() - a.begin) / ms) * (1 - a.from))
        a.progress = p
        if (p >= 1) return finish()
        await frame(p)
        requestAnimationFrame(() => void step())
      }
      const composited = !NO_COMPOSITE.has(tr.kind) && !tr.smartAnimate && tr.transition.type !== 'smartAnimate'
      const s = latest.current
      const p = placeRef.current
      if (!composited || !s?.screen || !p) {
        requestAnimationFrame(() => void step())
        return
      }
      // The screen before: the canvas when it shows it at full resolution, else the engine's.
      const px = pixelsPerUnit(s, p)
      const el = canvas.current
      let from: { image: HTMLCanvasElement; px: number } | null = null
      if (el && el.width > 0 && !previous && !drawing.current && before && drawnRevision.current === before.revision) {
        const copy = document.createElement('canvas')
        copy.width = el.width
        copy.height = el.height
        copy.getContext('2d')?.drawImage(el, 0, 0)
        from = { image: copy, px: before.screen ? el.width / Math.max(1, before.screen.width) : px }
      }
      void (async () => {
        const toImage = await picture(s, px, {})
        const fromImage = from ? null : await picture(s, px, { state: 'from' })
        if (anim.current !== a) return
        const asCanvas = (image: ImageData) => {
          const c = document.createElement('canvas')
          c.width = image.width
          c.height = image.height
          c.getContext('2d')?.putImageData(image, 0, 0)
          return c
        }
        if (toImage && (from || fromImage)) {
          a.comp = { from: from?.image ?? asCanvas(fromImage!), fromPx: from?.px ?? px, to: asCanvas(toImage), toPx: px, transition: tr.transition, easing: tr.easing as EasingJson, background: s.background }
        }
        // The clock starts with the first composited frame.
        a.begin = performance.now()
        requestAnimationFrame(() => void step())
      })()
    },
    [composite, draw, picture, pixelsPerUnit, show],
  )

  // --- Talking to the engine: in order; pointer moves coalesce.
  const chain = useRef<Promise<unknown>>(Promise.resolve())
  const pendingMove = useRef<Record<string, unknown> | null>(null)
  const applyRef = useRef<(answer: Answer) => void>(() => undefined)
  const send = useCallback(
    (command: string, params: Record<string, unknown> = {}): Promise<Answer | null> => {
      const job = chain.current.then(async () => {
        if (me.current !== owner) return null
        try {
          const text = await invoke<string>('player', command, JSON.stringify({ ...params, time: now() }))
          const answer = JSON.parse(text) as Answer
          if (me.current === owner && answer.state) applyRef.current(answer)
          return answer
        } catch (error) {
          if (command === 'start') setFailed(error instanceof Error ? error.message : String(error))
          return null
        }
      })
      chain.current = job
      return job
    },
    [invoke],
  )
  const move = useCallback(
    (params: Record<string, unknown>) => {
      const queued = pendingMove.current
      pendingMove.current = params
      if (queued) return
      void chain.current.then(() => {
        const next = pendingMove.current
        pendingMove.current = null
        if (next) void send('event', next)
      })
    },
    [send],
  )

  const schedule = useCallback(
    (s: PlayerState) => {
      if (timer.current != null) window.clearTimeout(timer.current)
      timer.current = null
      if (s.nextDue == null) return
      const wait = Math.max(0, (s.nextDue - s.clock) * 1000) + 4
      timer.current = window.setTimeout(() => {
        timer.current = null
        void send('tick')
      }, Math.min(wait, 2 ** 30))
    },
    [send],
  )

  const openLink = useCallback(
    (url: string) => {
      const host = editor.host as unknown as { init?: { capabilities?: string[] }; peer?: { request: (method: string, params: unknown) => Promise<unknown> } }
      if (host.init?.capabilities?.includes('open-url') && host.peer) void host.peer.request('host/openUrl', { url }).catch(() => undefined)
      else window.open(url, '_blank', 'noopener')
    },
    [editor],
  )

  applyRef.current = (answer: Answer) => {
    const before = latest.current
    latest.current = answer.state
    setState(answer.state)
    setFailed(null)
    setCursor(answer.cursor)
    schedule(answer.state)
    if (answer.missed && hintsRef.current) setFlash((n) => n + 1)
    for (const url of answer.links) {
      if (/^mailto:/i.test(url) || linkNoticeSkipped) openLink(url)
      else if (/^https?:/i.test(url)) setNotice(url)
    }
    const resized = !before?.screen || !answer.state.screen || before.screen.width !== answer.state.screen.width || before.screen.height !== answer.state.screen.height
    if (answer.transition) {
      if (resized) placeRef.current = latest.current ? place(latest.current, { width, height }, scaleMode, deviceFrame, pane) : null
      animate(answer.transition, before)
    } else if (answer.changed || resized) {
      if (resized) placeRef.current = latest.current ? place(latest.current, { width, height }, scaleMode, deviceFrame, pane) : null
      if (!anim.current) void draw()
      else anim.current.dirty = true
    }
  }

  // Start (again when `start` changes), stop when unmounted.
  const area = useRef({ width, height })
  area.current = { width, height }
  useEffect(() => {
    if (start == null) return undefined
    owner += 1
    me.current = owner
    const mine = owner
    anim.current = null
    void send('start', { start, window: { width: Math.round(area.current.width), height: Math.round(area.current.height) } })
    return () => {
      if (timer.current != null) window.clearTimeout(timer.current)
      timer.current = null
      if (owner === mine) {
        void invoke('player', 'stop', '{}').catch(() => undefined)
        owner += 1
      }
      me.current = 0
    }
  }, [invoke, send, start])

  // The area changed: responsive prototypes without a device take its size.
  useEffect(() => {
    if (!latest.current) return
    void send('resize', { width: Math.round(width), height: Math.round(height) })
  }, [height, send, width])

  // Follows edits: the engine copies the document again on the next call.
  const firstRevision = useRef(revision)
  useEffect(() => {
    if (firstRevision.current === revision || !latest.current) return
    void send('state').then(() => {
      if (!anim.current) void draw()
    })
  }, [draw, revision, send])

  // A new scale or option draws again at the new size.
  useLayoutEffect(() => {
    if (latest.current && !anim.current) void draw()
  }, [draw, placement?.scale, placement?.screen.width, placement?.screen.height])

  const api = useMemo<PlayerApi>(
    () => ({
      state,
      restart: (to?: number) => void send('restart', to != null ? { start: to } : {}),
      back: () => void send('back'),
      key: async (event) => {
        if (MODIFIERS.has(event.key)) return false
        const answer = await send('event', { type: 'key', key: event.key, shift: event.shiftKey, ctrl: event.ctrlKey, alt: event.altKey, meta: event.metaKey })
        return answer?.handled === true
      },
    }),
    [send, state],
  )
  const latestOnState = useRef(onState)
  latestOnState.current = onState
  useEffect(() => {
    latestOnState.current?.(state, api)
  }, [api, state])

  // Keys: to the prototype first; the presenter's own keys get the ones it did not use.
  const onKeyRef = useRef(onKey)
  onKeyRef.current = onKey
  const onKeyFirstRef = useRef(onKeyFirst)
  onKeyFirstRef.current = onKeyFirst
  useEffect(() => {
    if (!keys) return undefined
    const key = (event: KeyboardEvent) => {
      if (MODIFIERS.has(event.key) || event.repeat) return
      const target = event.target
      if (target instanceof HTMLElement && target.closest('input, textarea, [contenteditable="true"]')) return
      if (document.querySelector('.design-proto-notice, [role="menu"]')) return
      // The presenter closes on Escape whatever the prototype does (it listens itself).
      if (event.key === 'Escape' && onKeyRef.current) return
      if (pane && (event.metaKey || event.ctrlKey)) return
      if (onKeyFirstRef.current?.(event)) {
        event.preventDefault()
        event.stopPropagation()
        return
      }
      event.preventDefault()
      event.stopPropagation()
      const copy = { key: event.key, shiftKey: event.shiftKey, ctrlKey: event.ctrlKey, altKey: event.altKey, metaKey: event.metaKey, repeat: false } as KeyboardEvent
      void api.key(copy).then((used) => {
        if (!used) onKeyRef.current?.(copy)
      })
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [api, keys, pane])

  useEffect(
    () => () => {
      if (timer.current != null) window.clearTimeout(timer.current)
    },
    [],
  )

  if (start == null) return <div className="design-proto-player-empty" data-testid={testId}>{t('Select a frame to preview it')}</div>
  if (failed && !state) return <div className="design-proto-player-empty" data-testid={testId}>{t('Select a frame to preview it')}</div>
  if (!state || !placement) return <div className="design-proto-player" style={{ width, height }} data-testid={testId} />

  const scale = placement.scale
  const toPoint = (event: { clientX: number; clientY: number }) => {
    const box = canvas.current?.getBoundingClientRect()
    if (!box) return { x: 0, y: 0 }
    return { x: (event.clientX - box.left) / scale, y: (event.clientY - box.top) / scale }
  }
  const radius = placement.body ? screenRadius(placement.body, placement.screen.width, placement.screen.height) : 0
  const followDrag = (q: { x: number; y: number }) => {
    const a = anim.current
    if (!a?.follow || !a.down) return
    const along = (q.x - a.down.x) * a.follow.x + (q.y - a.down.y) * a.follow.y
    a.progress = Math.min(1, Math.max(0, along / Math.max(1, a.follow.length)))
  }
  /** The press ends at `q`, or is cancelled (`null`): the engine's `pointerCancel` fires nothing. */
  const release = (q: { x: number; y: number } | null) => {
    const a = anim.current
    void send('event', q ? { type: 'pointerUp', ...q } : { type: 'pointerCancel' }).then(() => {
      // A followed drag let go: the rest plays out.
      if (a && anim.current === a && a.follow && a.down) {
        a.down = null
        a.from = a.progress
        a.begin = performance.now()
      }
    })
    downAt.set(null)
  }
  return (
    <div className="design-proto-player" style={{ width, height }} data-testid={testId}>
      <div className="design-proto-stage" style={{ padding: placement.pad }}>
        <div className="design-proto-device" style={{ width: placement.whole.width, height: placement.whole.height }}>
          <div className="design-proto-screen-at" style={{ left: placement.offset.x, top: placement.offset.y, width: placement.screen.width, height: placement.screen.height }}>
            {placement.body ? <DeviceBody device={placement.body} w={placement.screen.width} h={placement.screen.height} /> : null}
            <div
              className="design-proto-screen"
              style={{ width: placement.screen.width, height: placement.screen.height, borderRadius: radius, background: state.background, cursor }}
              data-testid={testId ? `${testId}-screen` : undefined}
              data-frame={state.current}
              data-overlays={state.overlays.map((o) => o.id).join(',')}
              onPointerDown={(event) => {
                if (event.button !== 0) return
                event.currentTarget.setPointerCapture(event.pointerId)
                const q = toPoint(event)
                downAt.set(q)
                void send('event', { type: 'pointerDown', ...q }).then(() => {
                  const a = anim.current
                  if (a?.follow) a.down = q
                })
              }}
              onPointerMove={(event) => {
                const q = toPoint(event)
                followDrag(q)
                onPointer?.(q)
                move({ type: 'pointerMove', ...q })
              }}
              onPointerUp={(event) => {
                if (event.button !== 0 || !downAt.get()) return
                release(toPoint(event))
              }}
              // The browser took the press away (a touch turned into a scroll, a system gesture):
              // the press ends without a click.
              onPointerCancel={() => {
                if (downAt.get()) release(null)
              }}
              onLostPointerCapture={() => {
                if (downAt.get()) release(null)
              }}
              onPointerLeave={() => {
                onPointer?.(null)
                if (!downAt.get()) move({ type: 'pointerLeave' })
              }}
              onWheel={(event) => {
                const q = toPoint(event)
                const k = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? placement.screen.height : 1
                void send('event', { type: 'wheel', ...q, dx: (event.deltaX * k) / scale, dy: (event.deltaY * k) / scale })
              }}
            >
              <canvas ref={canvas} className="design-proto-canvas" style={{ width: placement.screen.width, height: placement.screen.height }} data-testid={testId ? `${testId}-canvas` : undefined} />
              {flash ? (
                <div key={flash} className="design-proto-hints" aria-hidden="true" data-testid={testId ? `${testId}-hints` : undefined}>
                  {state.hotspots.map((h) => (
                    <span key={h.id} style={{ left: h.x * scale, top: h.y * scale, width: h.width * scale, height: h.height * scale }} />
                  ))}
                </div>
              ) : null}
              {placement.body ? <Camera device={placement.body} w={placement.screen.width} h={placement.screen.height} /> : null}
            </div>
            {layer ? <div className="design-proto-screen-layer">{layer({ scale, state })}</div> : null}
          </div>
        </div>
      </div>
      {notice ? (
        <LinkNotice
          url={notice}
          onClose={() => setNotice(null)}
          onOpen={(skip) => {
            if (skip) linkNoticeSkipped = true
            openLink(notice)
            setNotice(null)
          }}
        />
      ) : null}
    </div>
  )
}

/** A mutable cell that is not React state (the press point). */
function useRefLess() {
  const cell = useRef<{ x: number; y: number } | null>(null)
  return { get: () => cell.current, set: (v: { x: number; y: number } | null) => void (cell.current = v) }
}

/** "Leaving the prototype" (upstream `link_notice`). */
function LinkNotice({ url, onClose, onOpen }: { url: string; onClose: () => void; onOpen: (skip: boolean) => void }) {
  const { t, run } = useDesignKit()
  const [skip, setSkip] = useState(false)
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      onClose()
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [onClose])
  return (
    <div className="design-proto-notice-scrim" onClick={onClose}>
      <div className="design-proto-notice" role="dialog" aria-label={t('Leaving the prototype')} onClick={(event) => event.stopPropagation()} data-testid="design-proto-link-notice">
        <div className="design-s13">{t('Leaving the prototype')}</div>
        <div className="design-r11">{t('This link opens a website outside the prototype:')}</div>
        <div className="design-r11 design-secondary" style={{ wordBreak: 'break-all' }} data-testid="design-proto-link-notice-url">
          {url}
        </div>
        <label className="design-r11 design-proto-notice-check">
          <input type="checkbox" checked={skip} onChange={(event) => setSkip(event.target.checked)} /> {t("Don't show this again")}
        </label>
        <div className="design-proto-notice-buttons">
          <button type="button" className="design-button" data-variant="secondary" onClick={onClose} data-testid="design-proto-link-notice-cancel">
            {t('Cancel')}
          </button>
          <button
            type="button"
            className="design-button"
            data-variant="primary"
            onClick={() => {
              if (skip) void run('proto.linkNotice', { show: false })
              onOpen(skip)
            }}
            data-testid="design-proto-link-notice-open"
          >
            {t('Open link')}
          </button>
        </div>
      </div>
    </div>
  )
}
