// The canvas overlays in a real browser (2.5 §1, §6, §9, §12, §13): node tests/canvas.mjs [chromium|webkit]
// The engine's overlays go through kit-editor's `overlayStyle` and `drawOverlay` hooks
// (src/canvas/engineOverlays.ts) and draw in the upstream editor's style: read back as canvas
// pixels (white handles with the page's green ring, the green marquee, the red guide label,
// the theme's caret) and as the drawn rotation cursor. Starts its own Vite server unless
// DESIGN_ORIGIN names a running one. Prints PASS or FAIL per step.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { evidence } from './evidence.mjs'

const which = process.argv[2] ?? 'chromium'
let server = null
let origin = process.env.DESIGN_ORIGIN
if (!origin) {
  server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
  await server.listen()
  origin = `http://127.0.0.1:${server.httpServer.address().port}`
}
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 })
const page = await context.newPage()
const shots = evidence(page, 'canvas', which)
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
const step = (name, ok, detail = '') => {
  if (!ok) {
    failures += 1
    shots.failed(name)
  }
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` - ${detail}` : ''}`)
}
const until = async (check, tries = 60) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(100)
  }
  return false
}

// The sample's page is dark (#1E1E1E): the overlay green is the bright one.
const BRAND = [0x02, 0xda, 0x96]
const GUIDE = [0xf2, 0x48, 0x22]
const near = (a, b, tolerance = 40) => Boolean(a) && a.every((v, i) => i > 2 || Math.abs(v - b[i]) <= tolerance)
const hex = (rgb) => (rgb ? `#${rgb.slice(0, 3).map((v) => v.toString(16).padStart(2, '0')).join('')}` : 'none')

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  const designKit = (body, arg) => frame().evaluate(body, arg)
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true)
  await page.waitForTimeout(800)
  const surface = await page.frameLocator('#kit').getByTestId('kit-canvas-surface').boundingBox()

  /** The view: zoom and the page point at the canvas's centre. */
  const viewAt = async (zoom, cx, cy) => {
    await designKit(([zoom, cx, cy]) => {
      const h = window.designKit.canvas.current
      const area = window.designKit.inspect.area
      h.zoomTo(zoom)
      const v = h.view()
      h.panTo(cx - area.x - v.width / zoom / 2, cy - area.y - v.height / zoom / 2)
    }, [zoom, cx, cy])
    await page.waitForTimeout(700)
  }
  /** A page point on the canvas, in canvas CSS pixels. */
  const onCanvas = (px, py) =>
    designKit(([px, py]) => {
      const v = window.designKit.canvas.current.view()
      const area = window.designKit.inspect.area
      return { x: (px - area.x - v.x) * v.zoom, y: (py - area.y - v.y) * v.zoom }
    }, [px, py])
  /** The drawn canvas pixel at canvas CSS point (x, y). */
  const pixel = (x, y) =>
    designKit(([x, y]) => {
      const canvas = document.querySelector('[data-testid=kit-canvas-surface]')
      const ratio = canvas.width / canvas.clientWidth
      return Array.from(canvas.getContext('2d').getImageData(Math.floor(x * ratio), Math.floor(y * ratio), 1, 1).data)
    }, [x, y])
  /** The engine's overlays now (document pixels), asked with the pointer where it last went. */
  let pointer = { x: 0, y: 0 }
  const overlays = () =>
    designKit(async ([px, py]) => {
      const area = window.designKit.inspect.area
      const mods = JSON.stringify({ alt: false, shift: false, command: false, control: false, space: false })
      return (await window.designKit.editor.session.tool('toolState', [px - area.x, py - area.y, mods, window.designKit.canvas.current.view().zoom])).overlays
    }, [pointer.x, pointer.y])
  const docToCanvas = (x, y) =>
    designKit(([x, y]) => {
      const v = window.designKit.canvas.current.view()
      return { x: (x - v.x) * v.zoom, y: (y - v.y) * v.zoom }
    }, [x, y])

  // The selection: Search (an auto layout frame with a corner radius) at 160%.
  await designKit(() => window.designKit.run('select.set', { ids: [86] }))
  await viewAt(1.6, 196.5, 300)
  await until(async () => (await overlays()).some((o) => o.kind === 'handle'))
  await page.waitForTimeout(400)
  const list = await overlays()
  const corner = list.find((o) => o.kind === 'anchor' && o.size === 8)
  const radius = list.find((o) => o.kind === 'handle')
  const c = await docToCanvas(corner.x, corner.y)
  const cornerFill = await pixel(Math.round(c.x - 0.5) + 0.5, Math.round(c.y - 0.5) + 0.5)
  step('a selection corner is a white square', near(cornerFill, [255, 255, 255], 10), hex(cornerFill))
  /** The pixels from (x, y) leftwards over `n` CSS pixels. */
  const leftwards = async (x, y, n) => {
    const out = []
    for (let i = 0; i <= n; i += 1) out.push(await pixel(x - i, y))
    return out
  }
  const cornerRow = await leftwards(c.x, c.y, 6)
  step('with a ring in the page green', cornerRow.some((p) => near(p, BRAND, 12)), cornerRow.map(hex).join(' '))
  const r = await docToCanvas(radius.x, radius.y)
  const radiusFill = await pixel(r.x, r.y)
  step('a corner radius handle is a white disc (kit-editor would fill it green)', near(radiusFill, [255, 255, 255], 10), hex(radiusFill))
  const radiusRow = await leftwards(r.x, r.y, 6)
  // White out to r 3.5, then the ring (anti-aliased at 1x: green over the light field).
  step('with its ring outside r 3.5', near(radiusRow[3], [255, 255, 255], 10) && radiusRow.slice(4).some((p) => p[1] - p[0] > 80 && p[1] > 200), radiusRow.map(hex).join(' '))

  // Alt over another layer: the red measured box and the distance label on a red pill.
  const filters = await onCanvas(102, 221)
  pointer = { x: 102, y: 221 }
  await page.keyboard.down('Alt')
  await page.mouse.move(surface.x + filters.x + 3, surface.y + filters.y, { steps: 3 })
  await page.mouse.move(surface.x + filters.x, surface.y + filters.y, { steps: 2 })
  const measured = await until(async () => (await overlays()).some((o) => o.kind === 'measure'))
  await page.waitForTimeout(400)
  const measure = (await overlays()).find((o) => o.kind === 'measure')
  const m = measure ? await docToCanvas(measure.x, measure.y) : null
  // The label is centred on its line (kit-editor's default would set an ink pill 12 px off).
  const label = m ? await pixel(m.x - 6, m.y) : null
  step('Alt measures with a red distance label on its line', measured && near(label, GUIDE), hex(label))
  await page.keyboard.up('Alt')
  await page.mouse.move(surface.x + 5, surface.y + surface.height - 5)

  // A marquee on empty canvas: the page green at 10% and a solid 1 px inside stroke.
  await designKit(() => window.designKit.run('select.set', { ids: [] }))
  await viewAt(1, 600, 300)
  const a = await onCanvas(420, 100)
  const b = await onCanvas(520, 200)
  await page.mouse.move(surface.x + a.x, surface.y + a.y)
  await page.mouse.down()
  await page.mouse.move(surface.x + (a.x + b.x) / 2, surface.y + (a.y + b.y) / 2, { steps: 4 })
  await page.mouse.move(surface.x + b.x, surface.y + b.y, { steps: 4 })
  // The drag's moves reach the engine one after another: wait for the marquee to reach the
  // pointer (on GitHub's Linux runners WebKit's marquee was still 3/4 of the way when read).
  const reached = async () => {
    const marquee = (await overlays()).find((o) => o.kind === 'marquee')
    if (!marquee) return false
    const end = await docToCanvas(marquee.x1, marquee.y1)
    return Math.abs(end.x - b.x) < 1 && Math.abs(end.y - b.y) < 1
  }
  await until(reached)
  await page.waitForTimeout(300)
  const edge = await pixel(Math.round(a.x) + 0.5, (a.y + b.y) / 2)
  // Halfway down its left edge: a dashed stroke (kit-editor's default) would leave gaps.
  const edges = []
  for (let y = Math.round(a.y) + 4; y < Math.round(a.y) + 12; y += 1) edges.push(await pixel(Math.round(a.x) + 0.5, y))
  step('the marquee is the page green, solid', near(edge, BRAND) && edges.every((p) => near(p, BRAND)), `${hex(edge)} at ${JSON.stringify(a)}, ${edges.map(hex).join(' ')}, overlays ${JSON.stringify((await overlays()).filter((o) => o.kind === 'marquee'))}`)
  await page.mouse.up()
  await page.waitForTimeout(300)

  // Text editing: the caret in the theme's text colour, not the page's.
  await designKit(() => window.designKit.run('select.set', { ids: [70] }))
  await designKit(() => window.designKit.run('text.edit.begin', { id: 70, caret: 2 }))
  await viewAt(4, 46, 27)
  const line = await until(async () => (await overlays()).some((o) => o.kind === 'line'))
  const caret = (await overlays()).find((o) => o.kind === 'line')
  const k = caret ? await docToCanvas(caret.ax, (caret.ay + caret.by) / 2) : null
  let seen = null
  // It blinks: look for it over a second.
  for (let i = 0; i < 12 && k; i += 1) {
    const p = await pixel(Math.round(k.x - 0.5) + 0.5, k.y)
    if (near(p, [0x1e, 0x1e, 0x1e], 50)) seen = p
    await page.waitForTimeout(100)
  }
  step('the caret is drawn in the theme text colour and blinks', line && Boolean(seen), hex(seen))
  await designKit(() => window.designKit.run('text.edit.end', {}))

  // The rotation zone outside a corner shows upstream's drawn rotation cursor.
  await designKit(() => window.designKit.run('select.set', { ids: [86] }))
  await viewAt(1.6, 196.5, 300)
  const outside = await onCanvas(373 + 6, 131.8 - 6)
  await page.mouse.move(surface.x + outside.x - 4, surface.y + outside.y + 4)
  await page.mouse.move(surface.x + outside.x, surface.y + outside.y, { steps: 2 })
  const rotate = await until(async () => (await designKit(() => getComputedStyle(document.querySelector('[data-testid=kit-canvas-surface]')).cursor)).startsWith('url('))
  step('the rotation zone shows the drawn rotation cursor', rotate)
} catch (error) {
  failures += 1
  shots.failed('error')
  console.log(`FAIL ${error instanceof Error ? error.message.split('\n').slice(0, 12).join(' / ') : String(error)}`)
} finally {
  if (errors.length) console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
  await shots.settled()
  await browser.close()
  if (server) await server.close()
}
process.exit(failures ? 1 : 0)
