// The view stays where a person put it (plan 2026-11-26 stage 2): node tests/hand/view.mjs [chromium|webkit]
// Real pointer, wheel and keys: the first frame drawn on a new page stays under the pointer while
// the page's document area grows round it (walkthrough B15: the view jumped about 750 px and the
// next clicks missed), a mouse wheel's notch with Cmd zooms one step around the pointer (B3: 2.7
// times), a trackpad's small deltas keep their feel, the wheel and Space with a drag pan, and
// Shift+1 fits the content. Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-view', which)
const { page, kit, step, until, part, finish, mod, surface, inspect, node, view, toPage, toDoc, viewAt, settle } = hand

const near = (a, b, within) => Math.abs(a - b) <= within
/** A real drag from `from` to `to` in `steps` moves. */
const drag = async (from, to, steps = 16) => {
  await page.mouse.move(from.x, from.y)
  await page.mouse.down()
  for (let i = 1; i <= steps; i += 1) await page.mouse.move(from.x + ((to.x - from.x) * i) / steps, from.y + ((to.y - from.y) * i) / steps)
  await page.mouse.up()
}

try {
  let frameId = null
  await part('B15', async () => {
    const pages = (await inspect()).pages.length
    await kit.getByTestId('design-pages-add').click()
    await until(async () => (await inspect()).pages.length === pages + 1 && (await inspect()).layers === 0, 60)
    await settle()
    const before = (await inspect()).area
    await viewAt(0.75, 420, 420)
    // Give the keyboard to the canvas the way a person does, with a click on it.
    const empty = await toPage(900, 900)
    await page.mouse.click(empty.x, empty.y)
    await page.keyboard.press('f')
    await until(async () => (await inspect()).tool === 'frame', 40)
    const from = await toPage(0, 0)
    const to = await toPage(375, 812)
    await drag(from, to)
    await until(async () => (await inspect()).layers === 1 && (await inspect()).tool === 'move', 60)
    await settle(400)
    frameId = (await inspect()).selection[0]
    const after = (await inspect()).area
    const b = (await node(frameId)).absoluteBounds
    const shown = await toPage(b.x, b.y)
    console.log(`  area ${JSON.stringify(before)} -> ${JSON.stringify(after)}, frame ${JSON.stringify(b)}`)
    step('B15 the new frame is where it was drawn', near(b.x, 0, 1) && near(b.y, 0, 1) && near(b.width, 375, 1) && near(b.height, 812, 1), JSON.stringify(b))
    step('B15 the frame stays under the pointer as the area grows', near(shown.x, from.x, 2) && near(shown.y, from.y, 2), `drawn from ${Math.round(from.x)},${Math.round(from.y)}, shown at ${Math.round(shown.x)},${Math.round(shown.y)}`)
    // A click with the Text tool inside it lands in it.
    await page.keyboard.press('t')
    await until(async () => (await inspect()).tool === 'text', 40)
    const inside = { x: from.x + (to.x - from.x) / 2, y: from.y + (to.y - from.y) / 2 }
    await page.mouse.click(inside.x, inside.y)
    const editing = await until(async () => (await inspect()).editing === 'text', 60)
    await page.keyboard.type('ab')
    const textId = (await inspect()).textEdit?.id
    await until(async () => textId != null && (await node(textId)).characters === 'ab', 40)
    const parent = textId != null ? (await node(textId)).parent : null
    step('B15 a Text tool click inside the new frame makes a text in it', editing && String(parent?.id ?? parent) === String(frameId), `editing ${editing}, parent ${JSON.stringify(parent)}, frame ${frameId}`)
    await page.keyboard.press('Escape')
    await page.keyboard.press('Escape')
    await until(async () => (await inspect()).editing === 'none', 40)
  })

  await part('B3', async () => {
    await viewAt(1, 200, 400)
    const at = { x: surface.x + surface.width * 0.4, y: surface.y + surface.height * 0.45 }
    await page.mouse.move(at.x, at.y)
    const under = await toDoc(at.x, at.y)
    const zoomOf = async () => (await view()).zoom
    const z0 = await zoomOf()
    await page.keyboard.down(mod)
    await page.mouse.wheel(0, -100)
    await page.keyboard.up(mod)
    await until(async () => (await zoomOf()) !== z0, 30)
    await page.waitForTimeout(200)
    const z1 = await zoomOf()
    step('B3 one mouse wheel notch with Cmd zooms in 1.2 times', near(z1 / z0, 1.2, 0.005), `${z0} -> ${z1} (${(z1 / z0).toFixed(3)} times)`)
    const still = await toDoc(at.x, at.y)
    step('B3 around the pointer', near(still.x, under.x, 1) && near(still.y, under.y, 1), `${JSON.stringify(under)} -> ${JSON.stringify(still)}`)
    await page.keyboard.down(mod)
    await page.mouse.wheel(0, 200)
    await page.keyboard.up(mod)
    await until(async () => (await zoomOf()) !== z1, 30)
    await page.waitForTimeout(200)
    const z2 = await zoomOf()
    step('B3 two notches back zoom out 1.44 times', near(z1 / z2, 1.44, 0.01), `${z1} -> ${z2}`)
    // A trackpad's small steps keep their feel (exp(0.01 per pixel)).
    await page.keyboard.down(mod)
    await page.mouse.wheel(0, -10)
    await page.keyboard.up(mod)
    await until(async () => (await zoomOf()) !== z2, 30)
    await page.waitForTimeout(200)
    const z3 = await zoomOf()
    step('B3 a trackpad step with Cmd still zooms smoothly', near(z3 / z2, Math.exp(0.1), 0.01), `${z2} -> ${z3} (${(z3 / z2).toFixed(3)} times)`)
  })

  await part('wheel pan', async () => {
    const before = await view()
    await page.mouse.wheel(0, 200)
    await until(async () => (await view()).y !== before.y, 30)
    await page.waitForTimeout(200)
    const after = await view()
    step('the wheel without Cmd pans 200 screen pixels down', near((after.y - before.y) * after.zoom, 200, 1) && after.zoom === before.zoom, `${before.y} -> ${after.y} at ${after.zoom}`)
    await page.mouse.wheel(150, 0)
    await until(async () => (await view()).x !== after.x, 30)
    await page.waitForTimeout(200)
    const sideways = await view()
    step('and sideways', near((sideways.x - after.x) * sideways.zoom, 150, 1), `${after.x} -> ${sideways.x}`)
  })

  await part('Space drag', async () => {
    const empty = { x: surface.x + surface.width - 80, y: surface.y + surface.height - 80 }
    await page.mouse.click(empty.x, empty.y)
    await settle()
    const before = await view()
    const selection = JSON.stringify((await inspect()).selection)
    await page.keyboard.down('Space')
    await drag(empty, { x: empty.x - 120, y: empty.y - 80 })
    await page.keyboard.up('Space')
    await until(async () => (await view()).x !== before.x, 30)
    await page.waitForTimeout(200)
    const after = await view()
    step('Space and a drag pan the view with the pointer', near((after.x - before.x) * after.zoom, 120, 2) && near((after.y - before.y) * after.zoom, 80, 2), `${JSON.stringify(before)} -> ${JSON.stringify(after)}`)
    step('and leave the tool and the selection as they were', (await inspect()).tool === 'move' && JSON.stringify((await inspect()).selection) === selection, `${(await inspect()).tool} ${JSON.stringify((await inspect()).selection)}`)
  })

  await part('Shift+1', async () => {
    await page.keyboard.press('Shift+Digit1')
    await page.waitForTimeout(600)
    await settle()
    const b = (await node(frameId)).absoluteBounds
    const a = await toPage(b.x, b.y)
    const z = await toPage(b.x + b.width, b.y + b.height)
    const inView = a.x >= surface.x - 1 && a.y >= surface.y - 1 && z.x <= surface.x + surface.width + 1 && z.y <= surface.y + surface.height + 1
    step('Shift+1 fits the page’s content in view', inView && z.y - a.y > surface.height * 0.5, `${JSON.stringify(a)} ${JSON.stringify(z)} in ${JSON.stringify(surface)}`)
  })
  await finish()
} catch (error) {
  await finish(error)
}
