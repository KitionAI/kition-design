// The canvas's everyday gestures (plan 2026-11-26 stage 3): node tests/hand/gestures.mjs [chromium|webkit]
// Real pointer and keys on a new page with two rectangles, A (100, 100, 100 x 80) and B (300, 100,
// 100 x 80), at 100 %: a click on a layer selects it, Shift+click adds one, a click on the empty
// canvas clears; a drag moves the layer as one undo step; each of the eight handles resizes from
// its side; dragging just outside a corner rotates; a drag that comes near another layer's edge
// snaps to it and shows the red guide; the arrows nudge by 1 and Shift by 10; Alt+drag leaves the
// original and drags a copy, Cmd+D duplicates again; Delete removes and Cmd+Z brings it back. S3:
// with Alt held over the other layer the distance shows, and once the pointer leaves the canvas
// the hover outline and the distance go (A16; kit-editor's `pointerLeft`). The results are read
// back from the engine (`inspect`, `query`). Stage 5: Alt pressed or let go during a drag switches
// between copy and move; with a frame selected a press on a layer in it drags the frame and a
// click selects the layer; a double-click on an auto layout edge hugs (Alt: fills); Alt+drag of a
// variant out of its set gives an instance; Assets names the page of same-named components.
// Stage 8: Alt pressed or let go with the pointer still shows or hides the distance, and turns a
// drag that has stopped into a copy; Paste here away from a selected component set pastes on the
// page, where a click selects it (G5). Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-gestures', which)
const { page, kit, designKit, step, until, part, finish, mod, inspect, run, node, toPage, viewAt, settle } = hand

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

  await part('leave', async () => {
    // The engine's overlays now (the hover outline is a path; Alt's distances have number labels).
    const overlays = async () => {
      const zoom = (await designKit(() => window.designKit.canvas.current.view())).zoom
      const state = await designKit((zoom) => window.designKit.editor.session.tool('toolState', [0, 0, '{}', zoom]), zoom)
      return state?.overlays ?? []
    }
    const count = async (kind) => (await overlays()).filter((o) => o.kind === kind).length
    // Off the canvas in one move (no canvas move on the way), over the editor's top bar.
    const { surface } = hand
    const away = { x: surface.x + surface.width / 2, y: surface.y - 12 }
    const over = await toPage(350, 140)
    // Nothing selected: the hovered layer's outline is the only path.
    await click(600, 400)
    await page.mouse.move(over.x - 20, over.y)
    await page.mouse.move(over.x, over.y, { steps: 4 })
    await settle(250)
    const outlined = await count('path')
    await page.mouse.move(away.x, away.y)
    await settle(300)
    const left = await count('path')
    step('leave the hover outline goes when the pointer leaves the canvas', outlined > 0 && left === 0, `${outlined} paths over the layer, ${left} off the canvas`)
    // A selected, Alt held over the other layer: the distance to it.
    await click(150, 140)
    await page.mouse.move(over.x - 20, over.y)
    await page.keyboard.down('Alt')
    await page.mouse.move(over.x, over.y, { steps: 4 })
    await settle(250)
    const measured = await count('measure')
    await page.mouse.move(away.x, away.y)
    await settle(300)
    const after = await count('measure')
    await page.keyboard.up('Alt')
    step('leave the Alt distance goes when the pointer leaves the canvas', measured > 0 && after === 0, `${measured} measurements over the layer, ${after} off the canvas`)
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

  // Stage 5. A24: Alt pressed during a drag makes it a copy (the original stays); let go, the
  // original moves and no copy is left. The release decides.
  await part('alt during a drag', async () => {
    const layers = (await inspect()).layers
    await click(600, 400)
    await click(150, 150)
    const original = await box(A)
    const steps = async (from, to, altAt) => {
      const a = await toPage(from.x, from.y)
      const b = await toPage(to.x, to.y)
      await page.mouse.move(a.x, a.y)
      if (altAt === 0) await page.keyboard.down('Alt')
      await page.mouse.down()
      for (let i = 1; i <= 12; i += 1) {
        if (i === 6) await page.keyboard[altAt === 0 ? 'up' : 'down']('Alt')
        await page.mouse.move(a.x + ((b.x - a.x) * i) / 12, a.y + ((b.y - a.y) * i) / 12)
        await page.waitForTimeout(16)
      }
      await page.mouse.up()
      if (altAt !== 0) await page.keyboard.up('Alt')
      await settle(300)
    }
    await steps({ x: 150, y: 150 }, { x: 150, y: 330 }, 6)
    const copy = (await selection())[0]
    const dragged = copy !== undefined && copy !== A ? await box(copy) : null
    step('alt during a drag Alt pressed midway drags a copy', (await inspect()).layers === layers + 1 && dragged !== null && near(dragged.y, original.y + 180, 2), dragged ? show(dragged) : JSON.stringify(await selection()))
    step('alt during a drag the original stays', near((await box(A)).y, original.y, 0.01), show(await box(A)))
    await undo()
    await click(150, 150)
    await steps({ x: 150, y: 150 }, { x: 330, y: 150 }, 0)
    step('alt during a drag Alt let go midway moves the original, no copy', (await inspect()).layers === layers && near((await box(A)).x, original.x + 180, 2), `${(await inspect()).layers} layers, ${show(await box(A))}`)
    await undo()
  })

  // Stage 8: a modifier pressed or let go with the pointer still reaches the tool: Alt over another
  // layer shows the distance at once, and Alt pressed during a drag that has stopped moving makes
  // it a copy.
  await part('modifier still', async () => {
    const overlays = async () => {
      const zoom = (await designKit(() => window.designKit.canvas.current.view())).zoom
      const state = await designKit((zoom) => window.designKit.editor.session.tool('toolState', [0, 0, '{}', zoom]), zoom)
      return state?.overlays ?? []
    }
    const measured = async () => (await overlays()).filter((o) => o.kind === 'measure').length
    const layers = (await inspect()).layers
    await click(600, 400)
    const a = await box(A)
    const b = await box(B)
    await click(a.x + a.w / 2, a.y + a.h / 2)
    const over = await toPage(b.x + b.w / 2, b.y + b.h / 2)
    await page.mouse.move(over.x - 10, over.y)
    await page.mouse.move(over.x, over.y, { steps: 3 })
    await settle(250)
    const plain = await measured()
    await page.keyboard.down('Alt')
    await until(async () => (await measured()) > 0, 20)
    const held = await measured()
    await page.keyboard.up('Alt')
    await until(async () => (await measured()) === 0, 20)
    const let_go = await measured()
    step('modifier still Alt pressed with the pointer still shows the distance', plain === 0 && held > 0, `${plain} measurements, ${held} with Alt`)
    step('modifier still and letting go of Alt hides it', let_go === 0, `${let_go} measurements`)
    // A drag that has stopped, then Alt.
    const from = await toPage(a.x + a.w / 2, a.y + a.h / 2)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    for (let i = 1; i <= 8; i += 1) {
      await page.mouse.move(from.x, from.y + (150 * i) / 8)
      await page.waitForTimeout(16)
    }
    await page.waitForTimeout(250)
    await page.keyboard.down('Alt')
    await page.waitForTimeout(400)
    await page.mouse.up()
    await page.keyboard.up('Alt')
    await settle(300)
    const copy = (await selection())[0]
    const dragged = copy !== undefined && copy !== A ? await box(copy) : null
    step('modifier still Alt pressed during a still drag makes it a copy', (await inspect()).layers === layers + 1 && dragged !== null && near(dragged.y, a.y + 150, 2), dragged ? show(dragged) : `${(await inspect()).layers} layers`)
    step('modifier still the original stays', near((await box(A)).y, a.y, 0.01), show(await box(A)))
    await undo()
  })

  // R-A22: with a frame selected, a press on a layer in it drags the frame; a click selects that layer.
  let row = null
  let child = null
  await part('press in the selection', async () => {
    row = await run('frame.add', { x: 600, y: 100, width: 300, height: 100, name: 'Row' })
    row = row?.id ?? (await selection())[0]
    await run('node.set', { id: row, props: { fill: '#ffffff' } })
    const inside = await run('shape.rectangle', { x: 20, y: 20, width: 40, height: 30, parent: row })
    child = inside?.id ?? (await selection())[0]
    await run('shape.rectangle', { x: 80, y: 20, width: 40, height: 30, parent: row })
    await run('select.set', { ids: [row] })
    await settle()
    await drag({ x: 640, y: 135 }, { x: 680, y: 135 })
    const moved = await box(row)
    step('press in the selection a drag on a layer in the selected frame moves the frame', near(moved.x, 640, 1) && JSON.stringify(await selection()) === JSON.stringify([row]), `${show(moved)}, ${JSON.stringify(await selection())}`)
    await undo()
    await run('select.set', { ids: [row] })
    await settle()
    await click(640, 135)
    step('press in the selection a click there selects that layer', JSON.stringify(await selection()) === JSON.stringify([child]), JSON.stringify(await selection()))
  })

  // Double-click on an auto layout object's edge: the right edge hugs the width; Alt on a child's
  // edge fills it.
  await part('double-click an edge', async () => {
    await run('layout.add', { ids: [row], direction: 'horizontal' })
    await run('node.set', { id: row, props: { padding: 10, gap: 10, sizingH: 'fixed', sizingV: 'fixed', width: 300, height: 100 } })
    await run('select.set', { ids: [row] })
    await viewAt(1, 750, 300)
    await settle()
    const r = await box(row)
    const edge = await toPage(r.x + r.w, r.y + r.h / 2)
    await page.mouse.move(edge.x - 20, edge.y)
    await page.mouse.move(edge.x, edge.y, { steps: 4 })
    await settle(200)
    await page.mouse.dblclick(edge.x, edge.y)
    await settle(300)
    const hugged = await box(row)
    step('double-click an edge the right edge of an auto layout frame hugs its width', Math.round(hugged.w) === 110 && Math.round(hugged.h) === 100, `${show(r)} -> ${show(hugged)} (10 + 40 + 10 + 40 + 10 wide), undo ${JSON.stringify((await hand.undoStack()).slice(0, 2))}`)
    await run('node.set', { id: row, props: { sizingH: 'fixed', width: 300 } })
    const kidId = child
    await run('select.set', { ids: [kidId] })
    await settle()
    const k = await box(kidId)
    const kidEdge = await toPage(k.x + k.w, k.y + k.h / 2)
    await page.mouse.move(kidEdge.x - 10, kidEdge.y)
    await page.mouse.move(kidEdge.x, kidEdge.y, { steps: 3 })
    await settle(200)
    await page.keyboard.down('Alt')
    await page.mouse.dblclick(kidEdge.x, kidEdge.y)
    await page.keyboard.up('Alt')
    await settle(300)
    const filled = await box(kidId)
    step('double-click an edge Alt on a child\'s edge fills the frame', filled.w > k.w + 50, `${show(k)} -> ${show(filled)}`)
  })

  // G9: Alt+drag of a variant out of its set gives an instance where it is dropped; the set stays.
  // G8: Assets tells same-named components on different pages apart by the page's name.
  await part('variant out of its set', async () => {
    const button = await run('frame.add', { x: 100, y: 500, width: 160, height: 40, name: 'Button' })
    let main = button?.id ?? (await selection())[0]
    await run('node.set', { id: main, props: { fill: '#2563eb' } })
    const made = await run('component.add', { ids: [main] })
    main = made?.id ?? (await selection())[0]
    await run('select.set', { ids: [main] })
    await run('component.addVariant', {})
    const set = (await node(main)).parent
    const setBox = await box(set)
    const screen = await run('frame.add', { x: 500, y: 500, width: 300, height: 300, name: 'Screen' })
    const screenId = screen?.id ?? (await selection())[0]
    await run('node.set', { id: screenId, props: { fill: '#ffffff' } })
    await run('select.set', { ids: [main] })
    await viewAt(1, 450, 620)
    await settle()
    const b = await box(main)
    await page.keyboard.down('Alt')
    await drag({ x: b.x + 20, y: b.y + 20 }, { x: 600, y: 600 }, { steps: 16 })
    await page.keyboard.up('Alt')
    await settle(300)
    const copy = (await selection())[0]
    const n = copy !== undefined ? await node(copy) : null
    const parent = n?.parent && typeof n.parent === 'object' ? n.parent.id : n?.parent
    step('variant out of its set Alt+drag gives an instance in the frame it is dropped in', n?.type === 'INSTANCE' && String(parent) === String(screenId), JSON.stringify(n && { type: n.type, parent }))
    const after = await box(set)
    step('variant out of its set the set keeps its size and its two variants', near(after.w, setBox.w, 0.5) && near(after.h, setBox.h, 0.5) && (await node(set)).childCount === 2, `${show(setBox)} -> ${show(after)}`)
    // Another Button on another page, then the Assets panel.
    const here = (await inspect()).page
    const other = await run('page.add', { name: 'Other' })
    const second = await run('frame.add', { x: 0, y: 0, width: 100, height: 40, name: 'Button' })
    await run('component.add', { ids: [second?.id ?? (await selection())[0]] })
    await run('page.select', { id: here })
    await settle()
    await kit.getByTestId('design-rail-assets').click()
    await until(async () => (await kit.locator('.design-asset-page').count()) >= 2, 60)
    const labels = await kit.locator('.design-asset-tile .design-asset-label').allInnerTexts()
    const pagesNamed = labels.filter((l) => /Button/.test(l))
    step('variant out of its set Assets shows the page of each of two same-named components', pagesNamed.some((l) => /Other/.test(l)) && pagesNamed.length >= 2 && pagesNamed.every((l) => / · /.test(l)), JSON.stringify(labels.slice(0, 12)))
    await kit.getByTestId('design-rail-file').click().catch(() => undefined)
    void other
  })

  // G5 (stage 8, regressions/issue-hand-g5-pasted-instance-pick): a variant copied, Escape (which
  // selects its set), Paste here from the context menu on the empty canvas. The instance went into
  // the set, outside the set's clip: nothing showed and a click on it selected nothing.
  await part('pasted instance', async () => {
    const button = await run('frame.add', { x: 100, y: 900, width: 160, height: 40, name: 'Chip' })
    let main = button?.id ?? (await selection())[0]
    await run('node.set', { id: main, props: { fill: '#16a34a' } })
    const made = await run('component.add', { ids: [main] })
    main = made?.id ?? (await selection())[0]
    await run('select.set', { ids: [main] })
    await run('component.addVariant', {})
    const set = (await node(main)).parent
    await run('select.set', { ids: [main] })
    await viewAt(1, 500, 950)
    await settle()
    await page.keyboard.press(`${mod}+c`)
    await settle(200)
    await page.keyboard.press('Escape')
    await settle(200)
    const escaped = await selection()
    const spot = await toPage(600, 1000)
    await page.mouse.click(spot.x, spot.y, { button: 'right' })
    await until(async () => (await kit.getByTestId('design-context-menu').count()) === 1, 40)
    await kit.getByTestId('design-context-edit.paste').click()
    await settle(400)
    const pasted = (await selection())[0]
    const n = pasted !== undefined ? await node(pasted) : null
    const parent = n?.parent && typeof n.parent === 'object' ? n.parent.id : n?.parent
    step('pasted instance Paste here away from the selected set pastes on the page, at the point', n?.type === 'INSTANCE' && String(parent) !== String(set) && near(n.absoluteBounds.x, 600, 1) && near(n.absoluteBounds.y, 1000, 1), JSON.stringify(n && { type: n.type, parent, bounds: n.absoluteBounds, escaped }))
    await click(900, 1200)
    await click(680, 1020)
    step('pasted instance a click at its centre selects it', pasted !== undefined && JSON.stringify(await selection()) === JSON.stringify([pasted]), JSON.stringify(await selection()))
  })
} catch (error) {
  await finish(error)
}
await finish()
