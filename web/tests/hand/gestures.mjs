// The canvas's everyday gestures (plan 2026-11-26 stage 3): node tests/hand/gestures.mjs [chromium|webkit]
// Real pointer and keys on a new page with two rectangles, A (100, 100, 100 x 80) and B (300, 100,
// 100 x 80), at 100 %: a click on a layer selects it, Shift+click adds one, a click on the empty
// canvas clears; a drag moves the layer as one undo step; each of the eight handles resizes from
// its side; dragging just outside a corner rotates; a drag that comes near another layer's edge
// snaps to it and shows the red guide; the arrows nudge by 1 and Shift by 10; Alt+drag leaves the
// original and drags a copy, Cmd+D duplicates again; Delete removes and Cmd+Z brings it back. The
// results are read back from the engine (`inspect`, `query`). Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-gestures', which)
const { page, kit, designKit, step, until, part, finish, mod, inspect, node, toPage, viewAt, settle } = hand

const near = (a, b, within = 1) => Math.abs(a - b) <= within
const box = async (id) => {
  const n = await node(id)
  const b = n.absoluteBounds
  return { x: b.x, y: b.y, w: b.width, h: b.height, w0: n.width, h0: n.height, rotation: n.rotation ?? 0 }
}
const show = (b) => `${Math.round(b.x)},${Math.round(b.y)} ${Math.round(b.w)}x${Math.round(b.h)}${b.rotation ? ` ${Math.round(b.rotation)}deg` : ''}`
/** A real drag between document points, in `steps` moves; `before` runs before the release. */
const drag = async (from, to, { steps = 12, before = null } = {}) => {
  const a = await toPage(from.x, from.y)
  const b = await toPage(to.x, to.y)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(a.x + ((b.x - a.x) * i) / steps, a.y + ((b.y - a.y) * i) / steps)
    await page.waitForTimeout(16)
  }
  if (before) await before()
  await page.mouse.up()
  await settle(300)
}
/** A real click on a document point, with `held` keys down round it (`Shift`). */
const click = async (x, y, held = []) => {
  const at = await toPage(x, y)
  for (const key of held) await page.keyboard.down(key)
  await page.mouse.click(at.x, at.y)
  for (const key of held) await page.keyboard.up(key)
  await settle(250)
}
const selection = async () => (await inspect()).selection
const undo = async () => {
  await page.keyboard.press(`${mod}+z`)
  await settle(300)
}

try {
  let A = null
  let B = null
  await part('setup', async () => {
    const pages = (await inspect()).pages.length
    await kit.getByTestId('design-pages-add').click()
    await until(async () => (await inspect()).pages.length === pages + 1 && (await inspect()).layers === 0, 60)
    await settle()
    await viewAt(1, 300, 200)
    await click(600, 400)
    for (const [x, y] of [[100, 100], [300, 100]]) {
      await page.keyboard.press('r')
      await until(async () => (await inspect()).tool === 'rectangle', 40)
      await drag({ x, y }, { x: x + 100, y: y + 80 }, { steps: 8 })
      await until(async () => (await inspect()).tool === 'move' && (await selection()).length === 1, 40)
      if (x === 100) A = (await selection())[0]
      else B = (await selection())[0]
    }
    const a = await box(A)
    const b = await box(B)
    step('setup two rectangles drawn', near(a.x, 100) && near(a.y, 100) && near(a.w, 100) && near(a.h, 80) && near(b.x, 300) && near(b.w, 100), `${show(a)} / ${show(b)}`)
  })

  await part('select', async () => {
    await click(600, 400)
    await click(150, 140)
    step('select a click on a layer selects it', JSON.stringify(await selection()) === JSON.stringify([A]), JSON.stringify(await selection()))
    await click(350, 140, ['Shift'])
    const both = await selection()
    step('select Shift+click adds a layer', both.length === 2 && both.includes(A) && both.includes(B), JSON.stringify(both))
    await click(600, 400)
    step('select a click on the empty canvas clears', (await selection()).length === 0)
  })

  await part('move', async () => {
    await click(150, 140)
    await drag({ x: 150, y: 140 }, { x: 170, y: 160 })
    const moved = await box(A)
    step('move a drag moves the layer', near(moved.x, 120) && near(moved.y, 120) && near(moved.w, 100), show(moved))
    await undo()
    const back = await box(A)
    step('move Cmd+Z takes the whole drag back', near(back.x, 100) && near(back.y, 100), show(back))
  })

  await part('resize', async () => {
    await click(150, 140)
    // Each handle dragged 10 px outward: [name, where, by, the box it gives].
    const handles = [
      ['top-left', [100, 100], [-10, -10], [90, 90, 110, 90]],
      ['top', [150, 100], [0, -10], [100, 90, 100, 90]],
      ['top-right', [200, 100], [10, -10], [100, 90, 110, 90]],
      ['right', [200, 140], [10, 0], [100, 100, 110, 80]],
      ['bottom-right', [200, 180], [10, 10], [100, 100, 110, 90]],
      ['bottom', [150, 180], [0, 10], [100, 100, 100, 90]],
      ['bottom-left', [100, 180], [-10, 10], [90, 100, 110, 90]],
      ['left', [100, 140], [-10, 0], [90, 100, 110, 80]],
    ]
    for (const [name, [x, y], [dx, dy], [ex, ey, ew, eh]] of handles) {
      await drag({ x, y }, { x: x + dx, y: y + dy }, { steps: 8 })
      const b = await box(A)
      step(`resize the ${name} handle`, near(b.x, ex) && near(b.y, ey) && near(b.w, ew) && near(b.h, eh), `${show(b)}, wanted ${ex},${ey} ${ew}x${eh}`)
      await undo()
      const back = await box(A)
      if (!(near(back.x, 100) && near(back.y, 100) && near(back.w, 100) && near(back.h, 80))) step(`resize Cmd+Z after the ${name} handle`, false, show(back))
    }
  })

  await part('rotate', async () => {
    await click(150, 140)
    // Just outside the top-right corner, then round the centre.
    await drag({ x: 210, y: 92 }, { x: 240, y: 150 }, { steps: 12 })
    const turned = await box(A)
    step('rotate dragging outside a corner rotates', Math.abs(turned.rotation) > 5, show(turned))
    await undo()
    step('rotate Cmd+Z takes it back', Math.abs((await box(A)).rotation) < 0.01)
  })

  await part('snap', async () => {
    await click(350, 140)
    let guides = 0
    // B dragged 40 left and 3 down: its top comes 3 px from A's top and snaps to it.
    await drag({ x: 350, y: 140 }, { x: 310, y: 143 }, {
      before: async () => {
        const zoom = (await designKit(() => window.designKit.canvas.current.view())).zoom
        const state = await designKit((zoom) => window.designKit.editor.session.tool('toolState', [0, 0, '{}', zoom]), zoom)
        guides = (state?.overlays ?? []).filter((o) => o.kind === 'line' && o.color[0] === 0xf2 && o.color[1] === 0x48 && o.color[2] === 0x22).length
      },
    })
    const b = await box(B)
    step('snap the layer snaps to the other one\'s top', near(b.y, 100, 0.01) && near(b.x, 260, 1), show(b))
    step('snap the red guide shows during the drag', guides > 0, `${guides} guide lines`)
    await undo()
  })

  await part('nudge', async () => {
    await click(150, 140)
    const before = await box(A)
    await page.keyboard.press('ArrowRight')
    await settle(250)
    const right = await box(A)
    step('nudge an arrow moves 1 px', near(right.x, before.x + 1, 0.01) && near(right.y, before.y, 0.01), show(right))
    await page.keyboard.press('Shift+ArrowDown')
    await settle(250)
    const down = await box(A)
    step('nudge Shift+arrow moves 10 px', near(down.y, before.y + 10, 0.01) && near(down.x, right.x, 0.01), show(down))
  })

  await part('duplicate', async () => {
    await click(150, 150)
    const original = await box(A)
    const layers = (await inspect()).layers
    await page.keyboard.down('Alt')
    await drag({ x: 150, y: 150 }, { x: 150, y: 300 })
    await page.keyboard.up('Alt')
    await settle(300)
    const copy = (await selection())[0]
    const left = await box(A)
    const dragged = copy !== undefined && copy !== A ? await box(copy) : null
    step('duplicate Alt+drag makes a copy', (await inspect()).layers === layers + 1 && dragged !== null && near(dragged.y, original.y + 150, 2) && near(dragged.x, original.x, 2), dragged ? show(dragged) : JSON.stringify(await selection()))
    step('duplicate Alt+drag leaves the original', near(left.x, original.x, 0.01) && near(left.y, original.y, 0.01), show(left))
    await page.keyboard.press(`${mod}+d`)
    await settle(300)
    step('duplicate Cmd+D duplicates again', (await inspect()).layers === layers + 2, `${(await inspect()).layers} layers`)
  })

  await part('delete', async () => {
    const layers = (await inspect()).layers
    const gone = (await selection())[0]
    await page.keyboard.press('Backspace')
    await settle(300)
    step('delete Delete removes the selected layer', (await inspect()).layers === layers - 1 && !(await designKit((id) => window.designKit.query('node', { id }).then(() => true, () => false), gone)), `${(await inspect()).layers} layers`)
    await undo()
    step('delete Cmd+Z brings it back', (await inspect()).layers === layers, `${(await inspect()).layers} layers`)
  })
} catch (error) {
  await finish(error)
}
await finish()
