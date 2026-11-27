// The view stays where a person put it (plan 2026-11-26 stage 2): node tests/hand/view.mjs [chromium|webkit]
// Real pointer, wheel and keys: the first frame drawn on a new page stays under the pointer while
// the page's document area grows round it (walkthrough B15: the view jumped about 750 px and the
// next clicks missed), a mouse wheel's notch with Cmd zooms one step around the pointer (B3: 2.7
// times), a trackpad's small deltas keep their feel, the wheel and Space with a drag pan, and
// Shift+1 fits the content. At 100 % and 200 % the view sits on whole device pixels, also when it
// was put half a pixel off (M5: the canvas copies its frame pixel for pixel, and half a pixel off
// the GPU dropped rows: steps on round edges, broken thin lines), and a slanted line drawn there
// has ink in every column on the screen. Each page keeps its own view when another is shown and it
// comes back (B14). Stage 8: with Z held a click zooms in a step about the pointer, Alt+click out,
// a drag zooms to its area, and letting go of Z gives the canvas back (B11); a drag held at the
// canvas's edge pans the view, the dragged frame staying under the pointer, one undo step. Prints
// PASS or FAIL per step.
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
  // The frame the 'view on screen' part draws (the stage 8 parts use it).
  let shown = null
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

  await part('M5', async () => {
    const dpr = await page.evaluate(() => window.devicePixelRatio || 1)
    const onPixels = (v, scale) => near(v.x * scale, Math.round(v.x * scale), 1e-6) && near(v.y * scale, Math.round(v.y * scale), 1e-6)
    for (const zoom of [1, 2]) {
      // Half a device pixel off on purpose, as a centred zoom on a canvas of odd size puts it.
      await hand.designKit(([zoom, dpr]) => {
        const handle = window.designKit.canvas.current
        handle.zoomTo(zoom)
        const v = handle.view()
        handle.panTo(Math.round(v.x) + 0.5 / (zoom * dpr), Math.round(v.y) + 0.5 / (zoom * dpr))
      }, [zoom, dpr])
      await page.waitForTimeout(300)
      const v = await view()
      step(`M5 at ${zoom * 100} % the view is on whole device pixels`, v.zoom === zoom && onPixels(v, zoom * dpr), JSON.stringify(v))
    }
    await viewAt(1, 190, 120)
    await hand.designKit(() => {
      const handle = window.designKit.canvas.current
      const v = handle.view()
      handle.panTo(v.x + 0.5, v.y + 0.5)
    })
    await page.waitForTimeout(300)
    step('M5 at 100 % after a half-pixel pan the view is on whole pixels', onPixels(await view(), dpr), JSON.stringify(await view()))
    // A slanted line inside the frame (white), drawn with the Line tool and a real drag.
    const empty = await toPage(300, 700)
    await page.mouse.click(empty.x, empty.y)
    await page.keyboard.press('l')
    await until(async () => (await inspect()).tool === 'line', 40)
    const a = await toPage(60, 60)
    const b = await toPage(300, 130)
    await drag(a, b)
    await until(async () => (await inspect()).tool === 'move', 60)
    await page.keyboard.press('Escape')
    await page.mouse.move(surface.x + surface.width - 20, surface.y + surface.height - 20)
    await settle(800)
    const shot = await page.screenshot({ clip: { x: Math.round(a.x) + 6, y: Math.round(a.y) - 6, width: Math.round(b.x - a.x) - 12, height: Math.round(b.y - a.y) + 12 } })
    const columns = await page.evaluate(async (png) => {
      const image = new Image()
      image.src = `data:image/png;base64,${png}`
      await image.decode()
      const c = document.createElement('canvas')
      c.width = image.width
      c.height = image.height
      const g = c.getContext('2d')
      g.drawImage(image, 0, 0)
      const d = g.getImageData(0, 0, c.width, c.height).data
      let blank = 0
      for (let x = 0; x < c.width; x += 1) {
        let ink = false
        for (let y = 0; y < c.height && !ink; y += 1) ink = d[(y * c.width + x) * 4] < 160
        if (!ink) blank += 1
      }
      return { width: c.width, blank }
    }, shot.toString('base64'))
    step('M5 a slanted 1 px line has ink in every column on screen', columns.width > 100 && columns.blank === 0, JSON.stringify(columns))
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

  // B14: each page keeps its view: 200 % on one page, another page added and shown, and back.
  await part('B14', async () => {
    const here = (await inspect()).page
    await viewAt(2, 150, 300)
    const centre = { x: surface.x + surface.width / 2, y: surface.y + surface.height / 2 }
    const before = await toDoc(centre.x, centre.y)
    const pages = (await inspect()).pages.length
    await kit.getByTestId('design-pages-add').click()
    await until(async () => (await inspect()).pages.length === pages + 1, 60)
    await page.keyboard.press('Escape')
    await settle()
    const other = (await inspect()).page
    await viewAt(0.5, 0, 0)
    await kit.getByTestId(`design-page-${here}`).click()
    await until(async () => (await inspect()).page === here, 60)
    await page.waitForTimeout(600)
    await settle()
    const back = await view()
    const at = await toDoc(centre.x, centre.y)
    step('B14 going back to a page shows its view as it was left', back.zoom === 2 && near(at.x, before.x, 1) && near(at.y, before.y, 1), `zoom ${back.zoom}, centre ${Math.round(at.x)},${Math.round(at.y)} (was ${Math.round(before.x)},${Math.round(before.y)})`)
    await kit.getByTestId(`design-page-${other}`).click()
    await until(async () => (await inspect()).page === other, 60)
    await page.waitForTimeout(600)
    step('B14 the other page keeps its own view', (await view()).zoom === 0.5, `zoom ${(await view()).zoom}`)
  })

  // The view the editor reports is the one on the screen: at 75 % the canvas draws a view put half
  // a device pixel off on the nearest whole one and maps the pointer through that, so a point
  // worked out from the reported view must land on the document point it names. It did not (the
  // canvas reported the view as asked): a frame drawn from (600, 100) to (700, 180) came out at
  // y 101, and in the login task a press on a frame's label landed in a selected text's rotate
  // zone and a double-click on an instance's text on a small frame's edge (regressions/
  // issue-hand-login-label-and-instance-text).
  await part('view on screen', async () => {
    const dpr = await page.evaluate(() => window.devicePixelRatio || 1)
    // A page of its own, so nothing on it takes the frame in.
    const pages = (await inspect()).pages.length
    await kit.getByTestId('design-pages-add').click()
    await until(async () => (await inspect()).pages.length === pages + 1 && (await inspect()).layers === 0, 60)
    await page.keyboard.press('Escape')
    await settle()
    await hand.designKit(([dpr]) => {
      const handle = window.designKit.canvas.current
      const area = window.designKit.inspect.area
      handle.zoomTo(0.75)
      const device = 0.75 * dpr
      const half = (value) => (Math.round(value * device) + 0.5) / device
      handle.panTo(half(500 - area.x), half(20 - area.y))
    }, [dpr])
    await page.waitForTimeout(400)
    const empty = await toPage(900, 600)
    await page.mouse.click(empty.x, empty.y)
    await page.keyboard.press('Escape')
    await page.keyboard.press('f')
    await until(async () => (await inspect()).tool === 'frame', 40)
    // Measured now: the host page above the editor may have moved it since the start.
    const box = await kit.getByTestId('kit-canvas-surface').boundingBox()
    const at = async (x, y) => {
      const [v, area] = await hand.designKit(() => [window.designKit.canvas.current.view(), window.designKit.inspect.area])
      return { x: box.x + (x - area.x - v.x) * v.zoom, y: box.y + (y - area.y - v.y) * v.zoom }
    }
    await drag(await at(600, 100), await at(700, 180))
    await until(async () => (await inspect()).tool === 'move', 60)
    await settle(300)
    const id = (await inspect()).selection[0]
    const b = id != null ? (await node(id)).absoluteBounds : null
    step('a frame drawn between two points of the reported view is exactly there at 75 %', !!b && b.x === 600 && b.y === 100 && b.width === 100 && b.height === 80, JSON.stringify(b))
    // A click on its name label (page point from the reported view) picks it after a click away.
    await page.mouse.click(empty.x, empty.y)
    await settle()
    const label = await at(604, 100 - 12 / 0.75)
    await page.mouse.click(label.x, label.y)
    await settle()
    step('and a click on its label picks it', JSON.stringify((await inspect()).selection) === JSON.stringify([id]), JSON.stringify((await inspect()).selection))
    await page.keyboard.press('Escape')
    shown = id
  })

  // B11 (stage 8): with Z held a click zooms in a step about the pointer, Alt+click out a step, a
  // drag zooms to the area; nothing is selected or drawn; letting go of Z gives the canvas back.
  await part('B11', async () => {
    await viewAt(1, 650, 140)
    const away = await toPage(850, 300)
    await page.mouse.click(away.x, away.y)
    await page.keyboard.press('Escape')
    await settle()
    const cursor = () => hand.designKit(() => document.querySelector('.design-canvas-root')?.getAttribute('data-cursor') ?? null)
    const at = await toPage(640, 130)
    const under = await toDoc(at.x, at.y)
    await page.mouse.move(at.x, at.y)
    await page.keyboard.down('z')
    await until(async () => (await cursor()) === 'zoom-in', 20)
    step('B11 holding Z shows the zoom-in cursor', (await cursor()) === 'zoom-in', String(await cursor()))
    await page.mouse.click(at.x, at.y)
    await until(async () => (await view()).zoom === 2, 30)
    const still = await toDoc(at.x, at.y)
    step('B11 a click with Z held zooms in one step about the pointer', (await view()).zoom === 2 && near(still.x, under.x, 1) && near(still.y, under.y, 1), `zoom ${(await view()).zoom}, ${JSON.stringify(under)} -> ${JSON.stringify(still)}`)
    step('B11 and selects nothing', (await inspect()).selection.length === 0 && (await inspect()).tool === 'move', JSON.stringify((await inspect()).selection))
    await page.keyboard.down('Alt')
    await until(async () => (await cursor()) === 'zoom-out', 20)
    step('B11 with Alt the cursor zooms out', (await cursor()) === 'zoom-out', String(await cursor()))
    await page.mouse.click(at.x, at.y)
    await page.keyboard.up('Alt')
    await until(async () => (await view()).zoom === 1, 30)
    step('B11 Alt+click zooms out one step', (await view()).zoom === 1, `zoom ${(await view()).zoom}`)
    const layers = (await inspect()).layers
    await drag(await toPage(600, 100), await toPage(700, 180))
    await until(async () => (await view()).zoom > 2, 30)
    await page.waitForTimeout(200)
    const v = await view()
    const middle = await toDoc(surface.x + surface.width / 2, surface.y + surface.height / 2)
    step('B11 a drag with Z held zooms to the area dragged', v.zoom > 2 && near(middle.x, 650, 2) && near(middle.y, 140, 2), `zoom ${v.zoom.toFixed(2)}, centre ${Math.round(middle.x)},${Math.round(middle.y)}`)
    step('B11 and draws nothing', (await inspect()).layers === layers && (await inspect()).selection.length === 0, `${layers} -> ${(await inspect()).layers} layers`)
    await page.keyboard.up('z')
    await until(async () => (await cursor()) === null, 20)
    const frame = await toPage(650, 140)
    await page.mouse.click(frame.x, frame.y)
    await settle()
    step('B11 letting go of Z gives the canvas back to the Move tool', JSON.stringify((await inspect()).selection) === JSON.stringify([shown]) && (await view()).zoom === v.zoom, `${JSON.stringify((await inspect()).selection)} (the frame ${shown}), zoom ${(await view()).zoom}`)
  })

  // Stage 8: a drag held at the canvas's edge pans the view toward it, the dragged layer staying
  // under the pointer; one undo step.
  await part('edge pan', async () => {
    await viewAt(1, 650, 300)
    const start = await toPage(650, 140)
    await page.mouse.click(start.x, start.y)
    await settle()
    const before = await view()
    const b0 = (await node(shown)).absoluteBounds
    const undo0 = (await hand.undoStack()).length
    const edge = { x: surface.x + surface.width - 4, y: start.y }
    await page.mouse.move(start.x, start.y)
    await page.mouse.down()
    for (let i = 1; i <= 12; i += 1) await page.mouse.move(start.x + ((edge.x - start.x) * i) / 12, start.y)
    await page.waitForTimeout(900)
    const during = await view()
    // Out of the edge zone, so the view stops, then where the pointer and the frame are.
    const still = { x: edge.x - 40, y: edge.y }
    await page.mouse.move(still.x, still.y)
    await page.waitForTimeout(400)
    const pointerDoc = await toDoc(still.x, still.y)
    const b1 = (await node(shown)).absoluteBounds
    await page.mouse.up()
    await settle(400)
    const panned = (during.x - before.x) * during.zoom
    step('edge pan holding a drag at the right edge pans the view right', panned > 100, `${Math.round(panned)} screen px in 0.9 s`)
    // The press was at the frame's centre: the pointer is still over the same point of it.
    step('edge pan the dragged frame follows the pointer', near(pointerDoc.x - b1.x, 650 - b0.x, 3) && near(b1.y, b0.y, 1), `pointer at ${Math.round(pointerDoc.x)}, frame at ${Math.round(b1.x)} (was ${b0.x})`)
    const undo1 = await hand.undoStack()
    step('edge pan the drag is one undo step', undo1.length === undo0 + 1, JSON.stringify(undo1.slice(0, 2)))
    const stopped = await view()
    await page.waitForTimeout(300)
    step('edge pan stops with the release', (await view()).x === stopped.x, `${stopped.x} -> ${(await view()).x}`)
    await page.keyboard.press(`${mod}+z`)
    await settle(300)
    step('edge pan Cmd+Z puts the frame back', (await node(shown)).absoluteBounds.x === b0.x, JSON.stringify((await node(shown)).absoluteBounds))
  })
  await finish()
} catch (error) {
  await finish(error)
}
