// The scrub of a mixed stroke weight, repeated: node tests/scrub-repeat.mjs [chromium|webkit]
// paint.mjs's "a scrub moves each stroke weight of a mixed selection" failed twice in CI on
// Chromium (weights 3 and 7 became 1363 and 10; the screenshot was at 1660 %) and passed on rerun.
// This performs the same scrub (a pointer drag on the Stroke weight field's lead over two layers
// with different weights) 30 times at the default zoom and 30 times at 1660 %, each from a fresh
// state (the weights set again, the selection made again so the field mounts anew), and asserts
// the exact weights each time: both layers move by the same delta, the one the field shows, which
// is more than 0 and no more than the drag's pixels. A failure prints every pointer event of that
// scrub and every command the scrub ran.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { evidence } from './evidence.mjs'

const which = process.argv[2] ?? 'chromium'
const RUNS = 30
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
const page = await context.newPage()
const shots = evidence(page, 'scrub-repeat', which)
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
const until = async (check, tries = 80) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(50)
  }
  return false
}

const SAVE = 199 // the "Save" frame, with a stroke (as in paint.mjs)
const OTHER = 68 // a frame given the same stroke at another weight
const STEPS = 8 // pointer moves of the drag
const PX = 3 // pixels per move
const started = Date.now()

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  const designKit = (body, arg) => frame().evaluate(body, arg)
  const node = (id) => designKit((n) => window.designKit.query('node', { id: n }), id)
  const weights = async () => [(await node(SAVE)).strokeWeight, (await node(OTHER)).strokeWeight]
  const idle = () => until(async () => (await designKit(() => window.designKit.query('toolbar'))).interaction === null)
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true, 400)

  // A record of the scrub: the pointer events the editor's document sees (position, movement,
  // Pointer Lock) and every command the session runs, each with its parameters.
  await designKit(() => {
    window.__scrub = []
    for (const type of ['pointerdown', 'pointermove', 'pointerup']) {
      document.addEventListener(type, (e) => window.__scrub.push(`${type} x${Math.round(e.clientX)} mx${e.movementX} lock${document.pointerLockElement ? 1 : 0}`), true)
    }
    document.addEventListener('pointerlockchange', () => window.__scrub.push(`lock ${document.pointerLockElement ? 'on' : 'off'}`))
    const session = window.designKit.editor.session
    const run = session.run.bind(session)
    session.run = (command, params) => {
      window.__scrub.push(`run ${command} ${JSON.stringify(params ?? {}).slice(0, 400)}`)
      return run(command, params)
    }
  })

  // The paint test's state of the Save frame before its scrub: weight 3, outside, dashed, a top side.
  await designKit(([a, b]) => window.designKit.run('node.set', { ids: [a], props: { strokes: [{ type: 'solid', color: '#FFFFFF', opacity: 0.35 }], strokeAlign: 'outside', dashes: [12, 12], strokeSides: [3, 0, 0, 0] } }), [SAVE, OTHER])
  await designKit((id) => window.designKit.run('node.set', { ids: [id], props: { strokes: [{ type: 'solid', color: '#FFFFFF', opacity: 0.35 }] } }), OTHER)

  for (const zoom of [null, 16.6]) {
    if (zoom !== null) {
      await designKit((z) => window.designKit.canvas.current.zoomTo(z), zoom)
      await until(async () => Math.abs((await designKit(() => window.designKit.canvas.current.view().zoom)) - zoom) < 0.01)
    }
    const at = zoom === null ? 'default zoom' : `zoom ${Math.round(zoom * 100)} %`
    let clean = 0
    for (let run = 1; run <= RUNS; run += 1) {
      // A fresh state: the weights set again, nothing selected, then the two layers.
      await designKit(([a, b]) => window.designKit.run('node.set', { ids: [a], props: { strokeWeight: 3 } }).then(() => window.designKit.run('node.set', { ids: [b], props: { strokeWeight: 7 } })), [SAVE, OTHER])
      await designKit(() => window.designKit.run('select.set', { ids: [] }))
      const field = kit.getByTestId('design-stroke-weight')
      await until(async () => (await field.count()) === 0)
      await designKit((ids) => window.designKit.run('select.set', { ids }), [SAVE, OTHER])
      const mixed = await until(async () => (await field.count()) === 1 && (await field.getAttribute('data-mixed')) === 'true')
      const [wa, wb] = await weights()
      await designKit(() => {
        window.__scrub = []
      })
      const lead = await field.locator('.design-field-lead').boundingBox()
      const x = lead.x + lead.width / 2
      const y = lead.y + lead.height / 2
      await page.mouse.move(x, y)
      await page.mouse.down()
      for (let i = 1; i <= STEPS; i += 1) {
        await page.mouse.move(x + i * PX, y)
        await page.waitForTimeout(30)
      }
      // The field shows the drag's delta (its value from 0, as the selection is mixed).
      await until(async () => Number(await field.locator('input').inputValue()) > 0, 40)
      const delta = Number(await field.locator('input').inputValue())
      await page.mouse.up()
      await idle()
      // The scrub's commit and its refresh land.
      const settled = await until(async () => {
        const [va, vb] = await weights()
        return va === wa + delta && vb === wb + delta
      }, 40)
      const [va, vb] = await weights()
      const ok = mixed && wa === 3 && wb === 7 && delta > 0 && delta <= STEPS * PX && settled
      if (ok) clean += 1
      else {
        failures += 1
        shots.failed(`${at} run ${run}`)
        console.log(`FAIL ${which} ${at} run ${run}: ${wa},${wb} → ${va},${vb} (field ${delta}, mixed ${mixed})`)
        console.log(`  scrub: ${(await designKit(() => window.__scrub.join('\n    '))) ?? ''}`)
      }
    }
    console.log(`${clean === RUNS ? 'PASS' : 'FAIL'} ${which} ${at}: ${clean} of ${RUNS} scrubs of a mixed stroke weight moved each weight by the drag's delta`)
  }
  console.log(`${which}: ${((Date.now() - started) / 1000).toFixed(0)} s`)
} catch (error) {
  failures += 1
  shots.failed('error')
  console.log(`FAIL ${error instanceof Error ? error.message.split('\n').slice(0, 12).join(' / ') : String(error)}`)
} finally {
  if (errors.length) console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
  await shots.settled()
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
