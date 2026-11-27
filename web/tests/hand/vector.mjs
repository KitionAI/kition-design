// Vector editing by hand (plan 2026-11-26 stage 10; R-C15, R-C16, R-C17; walkthrough C7–C10):
// node tests/hand/vector.mjs [chromium|webkit]
// Real pointer and keys on a new page at 100 %. The pen: three clicks and a click on the first
// point close a filled triangle; a press dragged out makes a curve; Enter ends an open path and
// edits its points with the Move tool, and Enter again leaves (C9); a double-click ends a path
// without a doubled last point. Vector edit mode: a double-click on a rectangle edits its points;
// a point dragged moves; a selected point is deleted with Delete; P, B, Q, X, Shift+E, Shift+W and
// V pick the edit tools without leaving edit mode, and the toolbar's buttons show those keys and
// pick the same tools; P then a click on a segment adds a point there; Cmd+A selects every point
// and Cmd+J joins an open path's ends; Cmd held over a segment bends it. The Design panel shows
// the selected point's X and Y, its corner radius and mirroring, and a typed X moves the point
// (one undo step). The pencil keeps every drag sample even while the engine is slow to answer.
// Read back from the engine. Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

// Before the editor's scripts, in the kit's frame: the pointer messages sent to the engine's
// worker, and (while `slow` is on) every drag sample held back 60 ms, as a busy engine would be.
const slowDrags = () => {
  if (window === window.top) return
  const m = (window.__drags = { slow: false, sent: 0 })
  const post = Worker.prototype.postMessage
  Worker.prototype.postMessage = function (data, transfer) {
    const r = data?.request
    let drag = false
    if (r?.method === 'tool' && r.name === 'pointer') {
      try {
        drag = JSON.parse(r.args[0]).action === 'drag'
      } catch {
        drag = false
      }
    }
    if (drag && m.slow) {
      m.sent += 1
      setTimeout(() => post.call(this, data, transfer), 60)
      return undefined
    }
    return post.call(this, data, transfer)
  }
}

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-vector', which, { init: slowDrags })
const { page, kit, designKit, step, until, part, finish, mod, inspect, query, node, toPage, viewAt, settle, undoStack } = hand

const near = (a, b, within = 1) => Math.abs(a - b) <= within
const selection = async () => (await inspect()).selection
/** The vector edit state (`vector.edit.state`), or null outside edit mode. */
const edited = async () => (await query('vectorEdit'))?.state ?? null
const editTool = async () => (await inspect()).vectorEdit?.tool ?? null
/** A real drag between document points, in `steps` moves, `held` keys down round it. */
const drag = async (from, to, { steps = 10, held = [] } = {}) => {
  const a = await toPage(from.x, from.y)
  const b = await toPage(to.x, to.y)
  for (const key of held) await page.keyboard.down(key)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(a.x + ((b.x - a.x) * i) / steps, a.y + ((b.y - a.y) * i) / steps)
    await page.waitForTimeout(16)
  }
  await page.waitForTimeout(80)
  await page.mouse.up()
  for (const key of held) await page.keyboard.up(key)
  await settle(300)
}
const click = async (x, y) => {
  const at = await toPage(x, y)
  await page.mouse.click(at.x, at.y)
  await settle(250)
}
const press = async (key) => {
  await page.keyboard.press(key)
  await settle(200)
}
const curved = (s) => Math.hypot(...s.ta) + Math.hypot(...s.tb) > 1
/** The page point of vertex `i` of the edited layer. */
const vertexAt = async (i) => {
  const [x, y] = (await edited()).overlay.vertices[i]
  return { x, y }
}

try {
  let open = null
  await part('setup', async () => {
    const { pages, page: first } = await inspect()
    await kit.getByTestId('design-pages-add').click()
    await until(async () => (await inspect()).pages.length === pages.length + 1 && (await inspect()).page !== first, 60)
    await settle()
    await viewAt(1, 500, 350)
    await click(900, 620)
    const now = await inspect()
    step('setup a new page', now.pages.length === pages.length + 1 && now.page !== first && now.selection.length === 0, JSON.stringify([now.page, now.selection]))
  })

  await part('pen-close', async () => {
    const layers = (await inspect()).layers
    await press('p')
    await until(async () => (await inspect()).tool === 'pen', 30)
    for (const [x, y] of [[100, 100], [220, 100], [160, 200]]) await click(x, y)
    await click(100, 100)
    await until(async () => (await inspect()).layers === layers + 1, 30)
    const id = (await selection())[0]
    const n = id ? await node(id) : null
    step('pen-close three clicks and a click on the first point make one closed vector', (await inspect()).layers === layers + 1 && n?.type === 'VECTOR', JSON.stringify([n?.type, (await inspect()).layers - layers]))
    step('pen-close the closed path is filled', (n?.fills?.length ?? 0) > 0, JSON.stringify(n?.fills))
    step('pen-close the pen stays chosen', (await inspect()).tool === 'pen' && (await inspect()).editing === 'none', JSON.stringify([(await inspect()).tool, (await inspect()).editing]))
    await press('Escape')
    await until(async () => (await inspect()).tool === 'move', 20)
    await press('Enter')
    await until(async () => (await inspect()).editing === 'vector', 30)
    const net = (await edited())?.network
    step('pen-close it has three points and three segments', net?.vertices.length === 3 && net?.segments.length === 3, JSON.stringify([net?.vertices.length, net?.segments.length]))
    await press('Escape')
    await press('Escape')
    await until(async () => (await inspect()).editing === 'none', 20)
  })

  await part('pen-curve', async () => {
    await press('Escape')
    await press('p')
    await until(async () => (await inspect()).tool === 'pen', 30)
    await click(300, 100)
    await drag({ x: 420, y: 100 }, { x: 460, y: 140 }, { steps: 8 })
    const id = (await selection())[0]
    await press('Enter')
    await until(async () => (await inspect()).editing === 'vector', 30)
    const state = await edited()
    const segs = state?.network?.segments ?? []
    step('pen-curve a press dragged out makes a curve', segs.length === 1 && curved(segs[0]), JSON.stringify(segs))
    step('pen-curve Enter ends the open path and edits its points with the Move tool', state?.id === id && (await editTool()) === 'move', JSON.stringify([state?.id, id, await editTool()]))
    await press('Enter')
    await until(async () => (await inspect()).editing === 'none', 30)
    const now = await inspect()
    step('pen-curve Enter again leaves edit mode with the path selected (C9)', now.editing === 'none' && JSON.stringify(now.selection) === JSON.stringify([id]) && now.tool === 'move', JSON.stringify([now.editing, now.selection, now.tool]))
  })

  await part('pen-double-click', async () => {
    await press('Escape')
    await press('p')
    await until(async () => (await inspect()).tool === 'pen', 30)
    await click(300, 250)
    await click(400, 250)
    const at = await toPage(450, 300)
    await page.mouse.dblclick(at.x, at.y)
    await settle(400)
    await until(async () => (await inspect()).editing === 'vector', 30)
    const state = await edited()
    open = state?.id ?? null
    const net = state?.network
    step('pen-double-click a double-click ends the path without doubling its last point', net?.vertices.length === 3 && net?.segments.length === 2, JSON.stringify([net?.vertices.length, net?.segments.length]))
    step('pen-double-click and edits its points with the Move tool', (await editTool()) === 'move', String(await editTool()))
    await press('Escape')
    await press('Escape')
    await until(async () => (await inspect()).editing === 'none', 20)
  })

  await part('edit', async () => {
    await press('Escape')
    await press('r')
    await until(async () => (await inspect()).tool === 'rectangle', 30)
    await drag({ x: 100, y: 300 }, { x: 220, y: 380 }, { steps: 6 })
    await until(async () => (await inspect()).tool === 'move' && (await selection()).length === 1, 30)
    const id = (await selection())[0]
    const at = await toPage(160, 340)
    await page.mouse.dblclick(at.x, at.y)
    await until(async () => (await inspect()).editing === 'vector', 30)
    let state = await edited()
    step('edit a double-click on a rectangle edits its points', state?.id === id && state?.network?.vertices.length === 4 && (await editTool()) === 'move', JSON.stringify([state?.id, id, state?.network?.vertices.length]))
    // Drag the top-right corner.
    const corner = state.overlay.vertices.findIndex(([x, y]) => near(x, 220) && near(y, 300))
    const undo = (await undoStack()).length
    await drag({ x: 220, y: 300 }, { x: 250, y: 280 }, { steps: 8 })
    state = await edited()
    const moved = state?.overlay?.vertices?.[corner]
    // Point snapping may take it a few pixels to a neighbour's line.
    step('edit a point dragged moves', moved && near(moved[0], 250, 6) && near(moved[1], 280, 6), JSON.stringify(moved))
    step('edit one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    // Select the bottom-left corner and delete it.
    await click(100, 380)
    state = await edited()
    step('edit a click selects the point', state?.overlay?.selectedVertices?.length === 1, JSON.stringify(state?.overlay?.selectedVertices))
    await press('Delete')
    state = await edited()
    step('edit Delete removes the selected point', state?.network?.vertices.length === 3 && (await inspect()).editing === 'vector', JSON.stringify([state?.network?.vertices.length, (await inspect()).editing]))
    // P stays in edit mode; a click on a segment adds a point there.
    await press('p')
    await until(async () => (await editTool()) === 'pen', 20)
    step('edit P picks the pen and stays in edit mode', (await inspect()).editing === 'vector' && (await editTool()) === 'pen' && (await inspect()).tool === 'move', JSON.stringify([(await inspect()).editing, await editTool(), (await inspect()).tool]))
    const segs = (await edited()).overlay.segments
    const [p0, , , p3] = segs[0]
    await click((p0[0] + p3[0]) / 2, (p0[1] + p3[1]) / 2)
    state = await edited()
    step('edit the pen on a segment adds a point there', state?.network?.vertices.length === 4 && (await inspect()).editing === 'vector', JSON.stringify([state?.network?.vertices.length, (await inspect()).editing]))
    await press('v')
    await until(async () => (await editTool()) === 'move', 20)
    step('edit V picks the Move tool again', (await editTool()) === 'move' && (await inspect()).editing === 'vector', String(await editTool()))
  })

  await part('keys', async () => {
    const id = (await inspect()).vectorEdit?.id
    for (const [key, tool] of [['b', 'paint'], ['q', 'lasso'], ['x', 'cut'], ['Shift+E', 'eraser'], ['Shift+W', 'width'], ['p', 'pen'], ['v', 'move']]) {
      await press(key)
      await until(async () => (await editTool()) === tool, 20)
      const now = await inspect()
      step(`keys ${key} picks the ${tool} tool in edit mode`, (await editTool()) === tool && now.editing === 'vector' && now.vectorEdit?.id === id, JSON.stringify([await editTool(), now.editing, now.tool]))
    }
    step('keys Shift+E leaves the Design tab shown', (await kit.getByTestId('design-design-panel').count()) === 1, '')
    const layers = await selection()
    await press(`${mod}+a`)
    const state = await edited()
    step('keys Cmd+A selects every point, not the layers', state?.overlay?.selectedVertices?.length === state?.network?.vertices.length && JSON.stringify(await selection()) === JSON.stringify(layers), JSON.stringify([state?.overlay?.selectedVertices, await selection()]))
    // The toolbar's buttons show the keys that work, and pick the same tools.
    const tips = {}
    for (const tool of ['move', 'pen', 'paint', 'lasso', 'cut', 'eraser', 'width']) tips[tool] = await kit.getByTestId(`design-vector-tool-${tool}`).getAttribute('data-tip').catch(() => null)
    // The tip is the label, two spaces, then the key ("Lasso  Q", "Eraser  ⇧E" on a Mac).
    const shown = (tool, key) => typeof tips[tool] === 'string' && (tips[tool].split('  ')[1] ?? '').toUpperCase().includes(key)
    step('keys the toolbar shows the tools\' keys', shown('move', 'V') && shown('pen', 'P') && shown('paint', 'B') && shown('lasso', 'Q') && shown('cut', 'X') && shown('eraser', 'E') && shown('width', 'W'), JSON.stringify(tips))
    await kit.getByTestId('design-vector-tool-paint').click()
    await until(async () => (await editTool()) === 'paint', 20)
    step('keys the paint bucket button picks the paint bucket', (await editTool()) === 'paint', String(await editTool()))
    await kit.getByTestId('design-vector-tool-width').click()
    await until(async () => (await editTool()) === 'width', 20)
    step('keys the variable width button picks variable width', (await editTool()) === 'width', String(await editTool()))
    await press('v')
    await until(async () => (await editTool()) === 'move', 20)
    await press('Enter')
    await until(async () => (await inspect()).editing === 'none', 20)
    step('keys Enter leaves edit mode (C9)', (await inspect()).editing === 'none', (await inspect()).editing)
  })

  await part('join', async () => {
    if (!open) throw new Error('no open path from pen-double-click')
    await press('Escape')
    // Select the open path by its segment, then Enter edits its points.
    const b = (await node(open)).absoluteBounds
    await click(350, 250)
    await until(async () => JSON.stringify(await selection()) === JSON.stringify([open]), 20)
    await press('Enter')
    await until(async () => (await inspect()).editing === 'vector', 30)
    const undo = (await undoStack()).length
    await press(`${mod}+a`)
    await press(`${mod}+j`)
    const net = (await edited())?.network
    step('join Cmd+A, Cmd+J join the open path\'s ends', net?.segments.length === 3 && (await inspect()).editing === 'vector', JSON.stringify([net?.segments.length, b]))
    step('join one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
  })

  await part('bend', async () => {
    // Cmd held over a straight segment of the edited path bends it.
    const state = await edited()
    const straight = state.network.segments.findIndex((s) => !curved(s))
    const [p0, , , p3] = state.overlay.segments[straight]
    const mid = { x: (p0[0] + p3[0]) / 2, y: (p0[1] + p3[1]) / 2 }
    const undo = (await undoStack()).length
    await drag(mid, { x: mid.x + 10, y: mid.y + 30 }, { steps: 8, held: [mod] })
    const after = (await edited())?.network?.segments?.[straight]
    step('bend Cmd held over a segment bends it', after && curved(after), JSON.stringify(after))
    step('bend one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
  })

  await part('panel', async () => {
    // One point selected: its X and Y, corner radius and mirroring in the Design panel.
    // A click on empty canvas clears the points (and stays in edit mode), then one point is picked.
    await click(800, 600)
    const v0 = await vertexAt(0)
    await click(v0.x, v0.y)
    await until(async () => JSON.stringify((await edited())?.overlay?.selectedVertices) === '[0]', 20)
    step('panel one point picked', JSON.stringify((await edited())?.overlay?.selectedVertices) === '[0]' && (await inspect()).editing === 'vector', JSON.stringify((await edited())?.overlay?.selectedVertices))
    step('panel no size for a single point', (await kit.getByTestId('design-vector-w').count()) === 0, '')
    const x = kit.getByTestId('design-vector-x').locator('input')
    await until(async () => (await x.count()) === 1, 30)
    const box = await query('vectorBounds')
    const shown = Number(await x.inputValue().catch(() => 'NaN'))
    step('panel the selected point\'s X shows', near(shown, box?.x ?? NaN, 0.01), JSON.stringify([shown, box]))
    step('panel corner radius and mirroring show', (await kit.getByTestId('design-vector-radius').count()) === 1 && (await kit.getByTestId('design-vector-mirroring').count()) === 1, '')
    const undo = (await undoStack()).length
    const before = (await edited()).overlay.vertices
    await x.click()
    await page.keyboard.press(`${mod}+a`)
    await page.keyboard.type('600')
    await page.keyboard.press('Enter')
    await until(async () => near((await query('vectorBounds'))?.x ?? NaN, 600, 0.01), 30)
    const moved = await vertexAt(0)
    step('panel a typed X moves the point', near(moved.x - v0.x, 600 - box.x, 0.01) && near(moved.y, v0.y, 0.01), JSON.stringify([v0, moved, box]))
    step('panel the other points stay', JSON.stringify((await edited()).overlay.vertices.slice(1)) === JSON.stringify(before.slice(1)), '')
    step('panel one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    await settle(200)
    await press('p')
    await until(async () => (await editTool()) === 'pen', 20)
    step('panel the keys are the canvas\'s again after Enter', (await editTool()) === 'pen' && (await inspect()).editing === 'vector', String(await editTool()))
    await press('v')
    await press('Escape')
    await press('Escape')
    await until(async () => (await inspect()).editing === 'none', 20)
  })

  await part('pencil', async () => {
    // The pencil keeps every sample: each drag sample is held back 60 ms on its way to the engine,
    // as a busy engine would be, and still all of them reach it.
    await press('Escape')
    await press('Shift+P')
    await until(async () => (await inspect()).tool === 'pencil', 30)
    const layers = (await inspect()).layers
    const a = await toPage(100, 500)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await designKit(() => {
      window.__drags.sent = 0
      window.__drags.slow = true
    })
    const moves = 30
    for (let i = 1; i <= moves; i += 1) {
      await page.mouse.move(a.x + i * 10, a.y + (i % 2 === 0 ? 0 : 40))
      await page.waitForTimeout(20)
    }
    await page.waitForTimeout(150)
    await page.mouse.up()
    await settle(800)
    const sent = await designKit(() => {
      window.__drags.slow = false
      return window.__drags.sent
    })
    step('pencil every drag sample reaches the engine while it is slow', sent >= moves - 2, `${sent} of ${moves} moves`)
    await until(async () => (await inspect()).layers === layers + 1, 30)
    const id = (await selection())[0]
    const b = id ? (await node(id)).absoluteBounds : null
    step('pencil the stroke follows the zigzag', (await inspect()).layers === layers + 1 && b && b.height >= 35 && b.width >= 280, JSON.stringify(b))
  })
} catch (error) {
  await finish(error)
}
await finish()
