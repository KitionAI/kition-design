// Auto layout on the canvas by hand (plan 2026-11-26 stage 9; R-F5, R-F6, R-F19, R-F29–R-F34):
// node tests/hand/handles.mjs [chromium|webkit]
// Real pointer and keys on a new page at 100 %: three rectangles drawn in a row and Shift+A make a
// horizontal auto layout with their gap, three in a column a vertical one; `10, 30` in a padding
// field sets 10 above and below and 30 at the sides. On the canvas: dragging the left padding band
// sets the left padding; with Alt or Shift the right one too, with Alt+Shift all four, and the
// sides it sets light up; a click on a band types its value there (Enter sets it as one undo
// step, Escape keeps it, the keys are the canvas's again after both); dragging a gap band sets the
// gap. An Auto gap (space between) dragged on the canvas, or typed in the panel, becomes a fixed
// gap at the new value. Dragging a child past its neighbour reorders the auto layout. Each edit
// is one undo step. Read back from the engine. Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-handles', which)
const { page, kit, step, until, part, finish, mod, inspect, query, node, toPage, viewAt, settle, undoStack } = hand

const near = (a, b, within = 0.6) => Math.abs(a - b) <= within
const selection = async () => (await inspect()).selection
const layout = async (id) => (await node(id)).layout ?? {}
const show = (l) => `${l.mode} gap ${l.gap} padding [${l.padding}] ${l.mainAlign}`
const order = async (id) => ((await query('node', { id, depth: 1 })).children ?? []).map((c) => c.id)
/** A real drag between browser page points, in `steps` moves, `held` keys down round it. */
const dragPx = async (a, b, { steps = 10, held = [] } = {}) => {
  for (const key of held) await page.keyboard.down(key)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(a.x + ((b.x - a.x) * i) / steps, a.y + ((b.y - a.y) * i) / steps)
    await page.waitForTimeout(16)
  }
  await page.waitForTimeout(100)
  await page.mouse.up()
  for (const key of held) await page.keyboard.up(key)
  await settle(300)
}
/** The same between document points. */
const drag = async (from, to, options) => dragPx(await toPage(from.x, from.y), await toPage(to.x, to.y), options)
const click = async (x, y) => {
  const at = await toPage(x, y)
  await page.mouse.click(at.x, at.y)
  await settle(250)
}
/** A band's centre on the page, once the band is there. */
const band = async (name) => {
  const zone = kit.getByTestId(`design-al-${name}`)
  await until(async () => (await zone.count()) === 1, 40)
  const box = await zone.boundingBox()
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 }
}
/** Which bands are lit now. */
const lit = () => kit.locator('.design-al-zone[data-lit]').evaluateAll((els) => els.map((e) => e.getAttribute('data-testid').replace('design-al-', '')).sort())
/** Draws rectangles at document boxes [x, y, w, h], each with the Rectangle tool; answers their ids. */
const draw = async (boxes) => {
  const ids = []
  for (const [x, y, w, h] of boxes) {
    await page.keyboard.press('r')
    await until(async () => (await inspect()).tool === 'rectangle', 40)
    await drag({ x, y }, { x: x + w, y: y + h }, { steps: 6 })
    await until(async () => (await inspect()).tool === 'move' && (await selection()).length === 1, 40)
    ids.push((await selection())[0])
  }
  return ids
}
/** A marquee from empty canvas round `x0, y0 – x1, y1`, then Shift+A; answers the new frame. */
const wrap = async (x0, y0, x1, y1) => {
  await click(900, 620)
  await drag({ x: x0, y: y0 }, { x: x1, y: y1 })
  await page.keyboard.press('Shift+a')
  await until(async () => {
    const id = (await selection())[0]
    return id !== undefined && (await node(id)).layout?.mode !== undefined
  }, 40)
  return (await selection())[0]
}

try {
  let AL = null
  let kids = []
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

  await part('shift-a', async () => {
    kids = await draw([[100, 100, 100, 60], [250, 100, 80, 80], [380, 100, 120, 40]])
    const undo = (await undoStack()).length
    AL = await wrap(80, 80, 520, 200)
    const row = await layout(AL)
    step('shift-a three rectangles in a row make a horizontal auto layout with their gap', row.mode === 'horizontal' && row.gap === 50 && JSON.stringify(await order(AL)) === JSON.stringify(kids), show(row))
    step('shift-a one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    const column = await draw([[700, 100, 80, 40], [700, 180, 60, 60], [700, 280, 100, 30]])
    const V = await wrap(680, 80, 820, 330)
    const col = await layout(V)
    step('shift-a three in a column make a vertical one, the gap their mean', col.mode === 'vertical' && col.gap === 40 && JSON.stringify(await order(V)) === JSON.stringify(column), show(col))
    await click(900, 620)
    await click(150, 130)
    await settle(200)
    // The frame, not its child: pick it by its name label.
    const box = (await node(AL)).absoluteBounds
    const label = await toPage(box.x + 10, box.y)
    await page.mouse.click(label.x, label.y - 9)
    await until(async () => JSON.stringify(await selection()) === JSON.stringify([AL]), 40)
    step('shift-a the frame is selected by its label', JSON.stringify(await selection()) === JSON.stringify([AL]), JSON.stringify(await selection()))
  })

  await part('padding-field', async () => {
    const undo = (await undoStack()).length
    const input = kit.getByTestId('design-design-paddingHorizontal').locator('input')
    await input.click()
    await page.keyboard.press(`${mod}+a`)
    await page.keyboard.type('10, 30')
    await page.keyboard.press('Enter')
    await until(async () => JSON.stringify((await layout(AL)).padding) === '[10,30,10,30]', 40)
    step('padding-field "10, 30" sets 10 above and below, 30 at the sides', JSON.stringify((await layout(AL)).padding) === '[10,30,10,30]', show(await layout(AL)))
    step('padding-field one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    const v = kit.getByTestId('design-design-paddingVertical').locator('input')
    await v.click()
    await page.keyboard.press(`${mod}+a`)
    await page.keyboard.type('20 40')
    await page.keyboard.press('Enter')
    await until(async () => JSON.stringify((await layout(AL)).padding) === '[20,40,20,40]', 40)
    step('padding-field the other field takes a pair too', JSON.stringify((await layout(AL)).padding) === '[20,40,20,40]', show(await layout(AL)))
    await v.click()
    await page.keyboard.press(`${mod}+a`)
    await page.keyboard.type('12,5')
    await page.keyboard.press('Enter')
    await until(async () => (await layout(AL)).padding?.[0] === 12.5, 40)
    step('padding-field a comma between digits is still a decimal comma', JSON.stringify((await layout(AL)).padding) === '[12.5,40,12.5,40]', show(await layout(AL)))
    await page.keyboard.press(`${mod}+z`)
    await until(async () => JSON.stringify((await layout(AL)).padding) === '[20,40,20,40]', 40)
  })

  await part('drag', async () => {
    const undo = (await undoStack()).length
    const left = await band('padding-3')
    await page.mouse.move(left.x, left.y)
    await settle(150)
    step('drag hovering a band lights it alone', JSON.stringify(await lit()) === '["padding-3"]', JSON.stringify(await lit()))
    await dragPx(left, { x: left.x + 16, y: left.y })
    const one = await layout(AL)
    step('drag the left padding band sets the left padding', JSON.stringify(one.padding) === '[20,40,20,56]', show(one))
    step('drag one undo step', (await undoStack()).length === undo + 1 && (await undoStack())[0] === 'Padding', JSON.stringify((await undoStack()).slice(0, 2)))
    // Alt: the opposite side too, lit while held.
    const again = await band('padding-3')
    await page.mouse.move(again.x, again.y)
    await page.keyboard.down('Alt')
    await settle(150)
    const altLit = await lit()
    await page.keyboard.up('Alt')
    step('drag with Alt held over a band its opposite side lights too', JSON.stringify(altLit) === '["padding-1","padding-3"]', JSON.stringify(altLit))
    await dragPx(again, { x: again.x - 20, y: again.y }, { held: ['Alt'] })
    step('drag with Alt the opposite side follows', JSON.stringify((await layout(AL)).padding) === '[20,36,20,36]', show(await layout(AL)))
    const top = await band('padding-0')
    await dragPx(top, { x: top.x, y: top.y + 6 }, { held: ['Shift'] })
    step('drag with Shift too (top and bottom)', JSON.stringify((await layout(AL)).padding) === '[26,36,26,36]', show(await layout(AL)))
    const right = await band('padding-1')
    await dragPx(right, { x: right.x + 6, y: right.y }, { held: ['Alt', 'Shift'] })
    step('drag with Alt+Shift all four sides', JSON.stringify((await layout(AL)).padding) === '[30,30,30,30]', show(await layout(AL)))
    step('drag each drag its own undo step', (await undoStack()).length === undo + 4, JSON.stringify((await undoStack()).slice(0, 5)))
    const gap = await band('gap-0')
    await dragPx(gap, { x: gap.x + 14, y: gap.y })
    step('drag a gap band sets the gap', (await layout(AL)).gap === 64, show(await layout(AL)))
  })

  await part('type', async () => {
    const undo = (await undoStack()).length
    const top = await band('padding-0')
    await page.mouse.click(top.x, top.y)
    const field = kit.getByTestId('design-al-field')
    await until(async () => (await field.count()) === 1, 30)
    const shown = await field.inputValue().catch(() => null)
    const focused = await field.evaluate((el) => el === document.activeElement && el.selectionStart === 0 && el.selectionEnd === el.value.length).catch(() => false)
    step('type a click on a band opens a field with its value selected', shown === '30' && focused, `${shown} ${focused}`)
    await page.keyboard.type('24')
    await page.keyboard.press('Enter')
    await until(async () => (await layout(AL)).padding?.[0] === 24, 30)
    step('type Enter sets the value', JSON.stringify((await layout(AL)).padding) === '[24,30,30,30]' && (await field.count()) === 0, show(await layout(AL)))
    step('type one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    await settle(150)
    await page.keyboard.press('r')
    await until(async () => (await inspect()).tool === 'rectangle', 20)
    step('type the keys are the canvas\'s again after Enter', (await inspect()).tool === 'rectangle', (await inspect()).tool)
    await page.keyboard.press('v')
    await until(async () => (await inspect()).tool === 'move', 20)
    await settle(200)
    const gap = await band('gap-0')
    await page.mouse.click(gap.x, gap.y)
    await until(async () => (await field.count()) === 1, 30)
    await page.keyboard.type('99')
    await page.keyboard.press('Escape')
    await until(async () => (await field.count()) === 0, 30)
    await settle(200)
    step('type Escape keeps the value', (await layout(AL)).gap === 64 && (await undoStack()).length === undo + 1, show(await layout(AL)))
    step('type Escape keeps the selection', JSON.stringify(await selection()) === JSON.stringify([AL]), JSON.stringify(await selection()))
    await page.keyboard.press('r')
    await until(async () => (await inspect()).tool === 'rectangle', 20)
    step('type the keys are the canvas\'s again after Escape', (await inspect()).tool === 'rectangle', (await inspect()).tool)
    await page.keyboard.press('v')
    await until(async () => (await inspect()).tool === 'move', 20)
    // Alt+click types both sides.
    const left = await band('padding-3')
    await page.keyboard.down('Alt')
    await page.mouse.click(left.x, left.y)
    await page.keyboard.up('Alt')
    await until(async () => (await field.count()) === 1, 30)
    await page.keyboard.type('8')
    await page.keyboard.press('Enter')
    await until(async () => (await layout(AL)).padding?.[3] === 8, 30)
    step('type after an Alt+click both sides', JSON.stringify((await layout(AL)).padding) === '[24,8,30,8]', show(await layout(AL)))
  })

  await part('auto-gap', async () => {
    // Fixed width wider than the content, then Auto (space between) from the gap mode menu.
    await kit.getByTestId('design-design-sizingH').click()
    await settle(200)
    await kit.locator('.design-menu').getByText('Fixed', { exact: true }).first().click()
    await settle(200)
    const width = kit.getByTestId('design-design-width').locator('input')
    await width.click()
    await page.keyboard.press(`${mod}+a`)
    await page.keyboard.type('600')
    await page.keyboard.press('Enter')
    await until(async () => near((await node(AL)).width, 600), 30)
    await kit.getByTestId('design-design-gap-mode').click()
    await settle(200)
    await kit.getByTestId('design-design-gap-mode-spaceBetween').click()
    await until(async () => (await layout(AL)).mainAlign === 'spaceBetween', 30)
    await settle(300)
    // Content 300 wide, padding 8 + 8: two gaps of (600 - 16 - 300) / 2 = 142.
    const shownGap = await kitChromeGap()
    step('auto-gap the band shows the distance', near(shownGap ?? 0, 142), String(shownGap))
    const undo = (await undoStack()).length
    const gap = await band('gap-0')
    await dragPx(gap, { x: gap.x - 22, y: gap.y })
    const dragged = await layout(AL)
    step('auto-gap dragging it makes a fixed gap at the new value', dragged.mainAlign === 'min' && dragged.gap === 120, show(dragged))
    step('auto-gap one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    await kit.getByTestId('design-design-gap-mode').click()
    await settle(200)
    await kit.getByTestId('design-design-gap-mode-spaceBetween').click()
    await until(async () => (await layout(AL)).mainAlign === 'spaceBetween', 30)
    const input = kit.getByTestId('design-design-gap').locator('input')
    await input.click()
    await page.keyboard.press(`${mod}+a`)
    await page.keyboard.type('12')
    await page.keyboard.press('Enter')
    await until(async () => (await layout(AL)).mainAlign === 'min', 30)
    const typed = await layout(AL)
    const a = (await node(kids[0])).absoluteBounds
    const b = (await node(kids[1])).absoluteBounds
    step('auto-gap typing a gap in the panel makes it fixed and the layers follow', typed.mainAlign === 'min' && typed.gap === 12 && near(b.x - (a.x + a.width), 12), `${show(typed)} distance ${b.x - (a.x + a.width)}`)
  })

  await part('reorder', async () => {
    const before = await order(AL)
    const first = (await node(kids[0])).absoluteBounds
    const second = (await node(kids[1])).absoluteBounds
    // Select the child (a click in a selected frame selects its child), then drag it past the second.
    await click(first.x + 20, first.y + 20)
    await until(async () => JSON.stringify(await selection()) === JSON.stringify([kids[0]]), 30)
    const undo = (await undoStack()).length
    await drag({ x: first.x + 20, y: first.y + 20 }, { x: second.x + second.width + 10, y: first.y + 20 }, { steps: 14 })
    const after = await order(AL)
    step('reorder dragging a child past its neighbour reorders the auto layout', JSON.stringify(after) === JSON.stringify([kids[1], kids[0], kids[2]]), `${JSON.stringify(before)} → ${JSON.stringify(after)}`)
    step('reorder the child stays in the frame', (await node(kids[0])).parent === AL, String((await node(kids[0])).parent))
    step('reorder one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    await page.keyboard.press(`${mod}+z`)
    await until(async () => JSON.stringify(await order(AL)) === JSON.stringify(before), 30)
    step('reorder Cmd+Z puts it back', JSON.stringify(await order(AL)) === JSON.stringify(before), JSON.stringify(await order(AL)))
  })
} catch (error) {
  await finish(error)
}
await finish()

/** The first gap band's value as the canvas chrome gives it. */
async function kitChromeGap() {
  return hand.designKit(async () => {
    const chrome = JSON.parse(await window.designKit.invoke('chrome', window.designKit.canvas.current.view().zoom))
    return chrome.autoLayout?.gaps?.[0]?.value ?? null
  })
}
