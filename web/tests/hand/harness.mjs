// The hand tests' shared start: opens dev.html (its own Vite server unless DESIGN_ORIGIN names a
// running one) in `which` browser, and gives the steps' helpers. Real pointer and keyboard input
// only in the steps; the helpers read the engine back (`inspect`, `query`) and set up the view.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { evidence } from '../evidence.mjs'

/** `init`: a script for every frame before the editor's own (a test's counters). */
export async function openHand(script, which, { init = null } = {}) {
  let server = null
  let origin = process.env.DESIGN_ORIGIN
  if (!origin) {
    server = await createServer({ configFile: new URL('../../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
    await server.listen()
    origin = `http://127.0.0.1:${server.httpServer.address().port}`
  }
  const browser = await (which === 'webkit' ? webkit : chromium).launch()
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  if (init) await context.addInitScript(init)
  const page = await context.newPage()
  const shots = evidence(page, script, which)
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
  const until = async (check, tries = 80) => {
    for (let i = 0; i < tries; i += 1) {
      if (await check().catch(() => false)) return true
      await page.waitForTimeout(100)
    }
    return false
  }
  /** Runs one part; a part that throws fails and the next one still runs. */
  const part = async (name, body) => {
    try {
      await body()
    } catch (error) {
      step(`${name}: ran to the end`, false, error instanceof Error ? error.message.split('\n').slice(0, 4).join(' / ') : String(error))
    }
  }
  const finish = async (error) => {
    if (error) {
      failures += 1
      shots.failed('error')
      console.log(`FAIL ${error instanceof Error ? error.message.split('\n').slice(0, 12).join(' / ') : String(error)}`)
    }
    if (errors.length) console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
    await shots.settled()
    await browser.close()
    await server?.close()
    process.exit(failures ? 1 : 0)
  }

  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  const designKit = (body, arg) => frame().evaluate(body, arg)
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true, 300)
  await until(async () => (await designKit(() => window.designKit.tools.tools.length)) > 0, 100)
  await page.waitForTimeout(800)
  const mod = (await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control'
  const surface = await kit.getByTestId('kit-canvas-surface').boundingBox()
  const inspect = () => designKit(() => window.designKit.inspect)
  const run = (command, params = {}) => designKit(([command, params]) => window.designKit.run(command, params), [command, params])
  const query = (name, params = {}) => designKit(([name, params]) => window.designKit.query(name, params), [name, params])
  const node = (id) => query('node', { id })
  const view = () => designKit(() => window.designKit.canvas.current.view())
  /** The undo stack's labels, newest first. */
  const undoStack = () => designKit(() => window.designKit.invoke('query', 'history', '{}').then((text) => JSON.parse(text).undo ?? []))
  /** A document point (page coordinates) on the browser page. */
  const toPage = async (x, y) => {
    const [v, area] = await designKit(() => [window.designKit.canvas.current.view(), window.designKit.inspect.area])
    return { x: surface.x + (x - area.x - v.x) * v.zoom, y: surface.y + (y - area.y - v.y) * v.zoom }
  }
  /** A browser page point as a document point. */
  const toDoc = async (px, py) => {
    const [v, area] = await designKit(() => [window.designKit.canvas.current.view(), window.designKit.inspect.area])
    return { x: (px - surface.x) / v.zoom + v.x + area.x, y: (py - surface.y) / v.zoom + v.y + area.y }
  }
  /** Setup, not a gesture: the view at `zoom` with document point (`x`, `y`) at the canvas's centre. */
  const viewAt = async (zoom, x, y) => {
    await designKit(([zoom, x, y]) => {
      const handle = window.designKit.canvas.current
      const area = window.designKit.inspect.area
      handle.zoomTo(zoom)
      const v = handle.view()
      handle.panTo(x - area.x - v.width / zoom / 2, y - area.y - v.height / zoom / 2)
    }, [zoom, x, y])
    await page.waitForTimeout(500)
  }
  /** Waits until the editor's state stops changing (answers lag on a loaded machine). */
  const settle = async (min = 200) => {
    await page.waitForTimeout(min)
    let last = ''
    for (let i = 0; i < 30; i += 1) {
      const now = await designKit(() => {
        const i = window.designKit.inspect
        return JSON.stringify([i.selection, i.tool, i.undo, i.layers, i.editing, i.textEdit, i.page, i.area])
      }).catch(() => '')
      if (now === last) return
      last = now
      await page.waitForTimeout(150)
    }
  }
  return { page, kit, frame, designKit, step, until, part, finish, mod, surface, inspect, run, query, node, view, undoStack, toPage, toDoc, viewAt, settle }
}
