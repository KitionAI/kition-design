// Ruler guides by hand (plan 2026-11-26 stage 8, R-B16): node tests/hand/guides.mjs [chromium|webkit]
// Real pointer and keys on a new page with a frame F (100, 100, 400 x 300) at 100 %, rulers on
// (Shift+R): a drag out of the top ruler onto F adds a horizontal guide of F where it is
// released; one out of the left ruler near F's centre line lands on it (snapping); a guide
// dragged moves, with Alt the drag leaves a copy, dragged back onto its ruler it goes; a click
// selects a guide, then Delete removes it and the arrows nudge it (Shift: 10); a guide released
// off any frame is the page's. Each edit is one undo step. Read back from the engine's chrome.
// Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-guides', which)
const { page, kit, designKit, step, until, part, finish, mod, surface, inspect, run, query, toPage, viewAt, settle, undoStack } = hand

const near = (a, b, within = 0.01) => Math.abs(a - b) <= within
/** The ruler guides the canvas draws (the engine's chrome). */
const guides = () => designKit(async () => JSON.parse(await window.designKit.invoke('chrome', window.designKit.canvas.current.view().zoom)).guides)
const show = (list) => JSON.stringify(list.map((g) => `${g.page ? 'page' : g.owner}:${g.vertical ? 'v' : 'h'}${g.offset}${g.selected ? '*' : ''}`))
/** A real drag between two browser page points, in `steps` moves; `held` keys down round it. */
const drag = async (from, to, { steps = 10, held = [] } = {}) => {
  for (const key of held) await page.keyboard.down(key)
  await page.mouse.move(from.x, from.y)
  await page.mouse.down()
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(from.x + ((to.x - from.x) * i) / steps, from.y + ((to.y - from.y) * i) / steps)
    await page.waitForTimeout(16)
  }
  await page.waitForTimeout(150)
  await page.mouse.up()
  for (const key of held) await page.keyboard.up(key)
  await settle(300)
}
/** The guides of frame `owner` running `vertical`, by offset. */
const offsets = async (owner, vertical) => (await guides()).filter((g) => g.owner === owner && !g.page && g.vertical === vertical).map((g) => g.offset).sort((a, b) => a - b)

try {
  let F = null
  await part('setup', async () => {
    const pages = (await inspect()).pages.length
    await kit.getByTestId('design-pages-add').click()
    await until(async () => (await inspect()).pages.length === pages + 1 && (await inspect()).layers === 0, 60)
    await settle()
    const made = await run('frame.add', { x: 100, y: 100, width: 400, height: 300, name: 'F' })
    F = made?.id ?? (await inspect()).selection[0]
    await run('node.set', { id: F, props: { fill: '#ffffff' } })
    await viewAt(1, 400, 300)
    const away = await toPage(800, 600)
    await page.mouse.click(away.x, away.y)
    await settle()
    const checked = async () => (await query('checked')).ids.includes('view.rulers')
    if (!(await checked())) await page.keyboard.press('Shift+r')
    await until(checked, 30)
    await settle(300)
    step('setup rulers on', await checked())
  })

  await part('add', async () => {
    const undo = (await undoStack()).length
    const target = await toPage(300, 202)
    await drag({ x: target.x, y: surface.y + 10 }, target)
    await until(async () => (await offsets(F, false)).length === 1, 30)
    const list = await guides()
    step('add a drag out of the top ruler adds a horizontal guide of the frame where it is released', JSON.stringify(await offsets(F, false)) === '[102]', show(list))
    step('add one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    // Two pixels off the frame's vertical centre line (page x 300): it lands on it.
    const centre = await toPage(302, 250)
    await drag({ x: surface.x + 10, y: centre.y }, centre)
    await until(async () => (await offsets(F, true)).length === 1, 30)
    step('add a guide out of the left ruler lines up with the frame\'s centre', JSON.stringify(await offsets(F, true)) === '[200]', show(await guides()))
  })

  await part('move', async () => {
    const on = await toPage(150, 202)
    await page.mouse.move(on.x, on.y - 8)
    await page.mouse.move(on.x, on.y, { steps: 3 })
    await settle(200)
    const cursor = await designKit(() => document.querySelector('.design-canvas-root')?.getAttribute('data-cursor') ?? null)
    step('move the cursor over a horizontal guide is a vertical resize', cursor === 'ns-resize', String(cursor))
    await drag(on, await toPage(150, 262))
    step('move dragging a guide moves it', JSON.stringify(await offsets(F, false)) === '[162]', show(await guides()))
    await drag(await toPage(150, 262), await toPage(150, 282), { held: ['Alt'] })
    step('move with Alt the drag leaves a copy', JSON.stringify(await offsets(F, false)) === '[162,182]', show(await guides()))
    const from = await toPage(150, 282)
    await drag(from, { x: from.x, y: surface.y + 8 })
    step('move dragged back onto its ruler a guide goes', JSON.stringify(await offsets(F, false)) === '[162]', show(await guides()))
  })

  await part('keys', async () => {
    const on = await toPage(150, 262)
    await page.mouse.click(on.x, on.y)
    await settle(300)
    const selected = (await guides()).find((g) => g.selected)
    step('keys a click selects a guide', selected?.owner === F && selected.offset === 162 && (await inspect()).selection.length === 0, show(await guides()))
    await page.keyboard.press('Delete')
    await until(async () => (await offsets(F, false)).length === 0, 30)
    step('keys Delete removes the selected guide', (await offsets(F, false)).length === 0, show(await guides()))
    await page.keyboard.press(`${mod}+z`)
    await until(async () => (await offsets(F, false)).length === 1, 30)
    step('keys Cmd+Z brings it back', JSON.stringify(await offsets(F, false)) === '[162]', show(await guides()))
    const vertical = await toPage(300, 250)
    await page.mouse.click(vertical.x, vertical.y)
    await settle(300)
    await page.keyboard.press('ArrowRight')
    await until(async () => JSON.stringify(await offsets(F, true)) === '[201]', 20)
    await page.keyboard.press('Shift+ArrowRight')
    await until(async () => JSON.stringify(await offsets(F, true)) === '[211]', 20)
    step('keys the arrows nudge a selected guide, Shift by 10', JSON.stringify(await offsets(F, true)) === '[211]', show(await guides()))
    await page.keyboard.press('Escape')
    await settle(200)
    step('keys Escape lets it go', !(await guides()).some((g) => g.selected), show(await guides()))
  })

  await part('page', async () => {
    const off = await toPage(700, 560)
    await drag({ x: surface.x + 10, y: off.y }, off)
    await until(async () => (await guides()).some((g) => g.page), 30)
    const pageGuide = (await guides()).find((g) => g.page)
    step('page a guide released off any frame is the page\'s', pageGuide?.vertical === true && near(pageGuide.offset, 700), show(await guides()))
    // A page guide drags as a frame's does.
    await drag(await toPage(700, 650), await toPage(740, 650))
    const moved = (await guides()).find((g) => g.page)
    step('page a page guide drags too', near(moved?.offset ?? 0, 740), show(await guides()))
  })
} catch (error) {
  await finish(error)
}
await finish()
