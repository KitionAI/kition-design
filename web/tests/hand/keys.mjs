// The keyboard always works (plan 2026-11-26 stage 1): node tests/hand/keys.mjs [chromium|webkit]
// Real keys and clicks only, the way a person works: no `.focus()` before a key, no command run in
// place of a key. Each part leaves the keyboard somewhere a person would (a field after Enter, the
// Actions palette after Escape, a posted comment, a layer row, a toolbar button, the variables
// window) and then presses a key the canvas must take. Results are read back from the engine.
// Starts its own Vite server unless DESIGN_ORIGIN names a running one. Prints PASS or FAIL per step.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { evidence } from '../evidence.mjs'

const which = process.argv[2] ?? 'chromium'
let server = null
let origin = process.env.DESIGN_ORIGIN
if (!origin) {
  server = await createServer({ configFile: new URL('../../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
  await server.listen()
  origin = `http://127.0.0.1:${server.httpServer.address().port}`
}
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
const page = await context.newPage()
const shots = evidence(page, 'hand-keys', which)
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

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  const designKit = (body, arg) => frame().evaluate(body, arg)
  const inspect = () => designKit(() => window.designKit.inspect)
  const run = (command, params = {}) => designKit(([command, params]) => window.designKit.run(command, params), [command, params])
  const node = (id) => designKit((id) => window.designKit.query('node', { id }), id)
  const view = () => designKit(() => window.designKit.canvas.current.view())
  /** The tool as the engine has it and as the toolbar shows it. */
  const tool = () => designKit(() => ({ engine: window.designKit.inspect.tool, shown: window.designKit.tools.active }))
  const toolIs = async (id) => {
    const now = await tool()
    return now.engine === id && now.shown === id
  }
  const focused = () => designKit(() => {
    const at = document.activeElement
    return at ? `${at.tagName.toLowerCase()}${at.getAttribute('data-testid') ? `[${at.getAttribute('data-testid')}]` : ''}.${String(at.className).split(' ')[0]}` : 'none'
  })
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true, 300)
  await until(async () => (await designKit(() => window.designKit.tools.tools.length)) > 0, 100)
  await page.waitForTimeout(800)
  const mod = (await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control'
  const surface = await kit.getByTestId('kit-canvas-surface').boundingBox()
  /** A document point (page coordinates) on the browser page. */
  const toPage = async (x, y) => {
    const [v, area] = await designKit(() => [window.designKit.canvas.current.view(), window.designKit.inspect.area])
    return { x: surface.x + (x - area.x - v.x) * v.zoom, y: surface.y + (y - area.y - v.y) * v.zoom }
  }
  const centre = async (id) => {
    const b = (await node(id)).absoluteBounds
    return toPage(b.x + b.width / 2, b.y + b.height / 2)
  }
  /** An empty spot of the canvas (its top-right corner, clear of the sample's frames). */
  const empty = { x: surface.x + surface.width - 50, y: surface.y + 50 }

  // K6: the keyboard works as soon as the editor opens, before anything is clicked.
  await part('K6', async () => {
    await page.keyboard.press('r')
    step('K6 the first key after opening picks a tool (R)', await until(() => toolIs('rectangle'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
    await page.keyboard.press('v')
    await until(() => toolIs('move'), 40)
  })

  // A rectangle of its own, clear of the sample's frames, to edit.
  const made = await run('shape.rectangle', { x: 200, y: 1000, width: 120, height: 80 })
  const rect = made?.id ?? (await inspect()).selection[0]
  await run('select.set', { ids: [] })
  const selectRect = async () => {
    const at = await centre(rect)
    await page.mouse.click(at.x, at.y)
    await until(async () => JSON.stringify((await inspect()).selection) === JSON.stringify([rect]))
  }
  const x = async () => (await node(rect)).absoluteBounds.x
  /** Types `value` in a Design panel field and presses `key` (Enter or Escape), as a person does. */
  const typeInField = async (testId, value, key = 'Enter') => {
    const input = kit.getByTestId(testId).locator('input')
    await input.click()
    await page.keyboard.type(value)
    await page.keyboard.press(key)
  }

  // H2: after Enter (or Escape) in a panel field the keyboard is the canvas's again.
  await part('H2', async () => {
    await page.keyboard.press('Escape')
    await selectRect()
    await typeInField('design-design-x', '80')
    await until(async () => (await x()) === 80)
    step('H2 a typed X commits on Enter', (await x()) === 80, String(await x()))
    await page.keyboard.press('r')
    step('H2 R after Enter in a field picks the Rectangle tool', await until(() => toolIs('rectangle'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
    await page.keyboard.press('Escape')
    await until(() => toolIs('move'), 40)
    await selectRect()
    await typeInField('design-design-x', '90')
    await until(async () => (await x()) === 90)
    await page.keyboard.press(`${mod}+z`)
    step('H2 Cmd+Z after Enter in a field undoes the typed value', await until(async () => (await x()) === 80, 40), `x ${await x()}, focus ${await focused()}`)
    await selectRect()
    await typeInField('design-design-x', '100')
    await until(async () => (await x()) === 100)
    const layers = (await inspect()).layers
    await page.keyboard.press('Delete')
    step('H2 Delete after Enter in a field deletes the layer', await until(async () => (await inspect()).layers === layers - 1, 40), `${(await inspect()).layers} vs ${layers}, focus ${await focused()}`)
    await page.keyboard.press(`${mod}+z`)
    await until(async () => (await inspect()).layers === layers)
    await selectRect()
    await typeInField('design-design-x', '120', 'Escape')
    await page.keyboard.press('r')
    step('H2 R after Escape in a field picks the Rectangle tool', await until(() => toolIs('rectangle'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
    await page.keyboard.press('Escape')
    await until(() => toolIs('move'), 40)
  })

  // Cmd+F still opens Find and replace after a field had the keyboard (the walkthrough saw nothing).
  await part('Cmd+F', async () => {
    await selectRect()
    await typeInField('design-design-x', '80')
    await page.keyboard.press(`${mod}+f`)
    step('Cmd+F after Enter in a field opens Find and replace', await until(async () => (await kit.getByTestId('design-dialog-find').count()) === 1, 40), `focus ${await focused()}`)
    await page.keyboard.press('Escape')
    await until(async () => (await kit.getByTestId('design-dialog-find').count()) === 0)
  })

  // The Actions palette closed with Escape leaves the keyboard to the canvas.
  await part('palette', async () => {
    await page.mouse.click(empty.x, empty.y)
    await page.keyboard.press(`${mod}+k`)
    await until(async () => (await kit.getByTestId('design-actions-palette').count()) === 1)
    await page.keyboard.press('Escape')
    await until(async () => (await kit.getByTestId('design-actions-palette').count()) === 0)
    await page.keyboard.press('r')
    step('R after the Actions palette closed with Escape picks the Rectangle tool', await until(() => toolIs('rectangle'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
    await page.keyboard.press('v')
    step('and V picks Move again', await until(() => toolIs('move'), 40), JSON.stringify(await tool()))
  })

  // C12: after a comment is posted, V (and Escape) leave the Comment tool.
  await part('C12', async () => {
    await page.mouse.click(empty.x, empty.y)
    await page.keyboard.press('c')
    await until(() => toolIs('comment'), 40)
    await page.mouse.click(empty.x - 40, empty.y + 60)
    await until(async () => (await kit.getByTestId('design-comment-draft-text').count()) === 1)
    await page.keyboard.type('Looks good')
    await page.keyboard.press('Enter')
    await until(async () => (await kit.getByTestId('design-comment-draft').count()) === 0)
    await page.waitForTimeout(300)
    await page.keyboard.press('v')
    step('C12 V after posting a comment picks Move', await until(() => toolIs('move'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
    await page.keyboard.press('c')
    await until(() => toolIs('comment'), 40)
    await page.mouse.click(empty.x - 40, empty.y + 140)
    await until(async () => (await kit.getByTestId('design-comment-draft-text').count()) === 1)
    await page.keyboard.type('Second note')
    await page.keyboard.press('Enter')
    await until(async () => (await kit.getByTestId('design-comment-draft').count()) === 0)
    await page.waitForTimeout(300)
    // The thread the post opened closes first (as with any open thread), then the tool goes.
    await page.keyboard.press('Escape')
    step('C12 Escape after posting a comment closes the thread it opened', await until(async () => (await kit.getByTestId('design-comment-thread').count()) === 0, 40), `focus ${await focused()}`)
    await page.keyboard.press('Escape')
    step('C12 and the next Escape leaves the Comment tool', await until(() => toolIs('move'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
  })

  // C15: Escape after a click on a toolbar button goes back to Move.
  await part('C15', async () => {
    await kit.getByTestId('design-tool-shape').click()
    await until(async () => (await tool()).engine !== 'move', 40)
    const chosen = (await tool()).engine
    await page.keyboard.press('Escape')
    step('C15 Escape after clicking a toolbar tool goes back to Move', await until(() => toolIs('move'), 40), `${chosen} -> ${JSON.stringify(await tool())}, focus ${await focused()}`)
  })

  // The toolbar shows the tool a command chose (a panel's or an agent's tool.*), not only its own.
  await part('toolbar', async () => {
    await designKit(() => void window.designKit.editor.run('tool.ellipse'))
    step('the toolbar follows the tool a command chose', await until(() => toolIs('ellipse'), 40), JSON.stringify(await tool()))
    const pressed = await kit.getByTestId('design-tool-shape').getAttribute('aria-pressed')
    step('and its button shows pressed', pressed === 'true', String(pressed))
    await page.keyboard.press('v')
    await until(() => toolIs('move'), 40)
  })

  // E9: a clicked layer row keeps the arrows (the list moves) and leaves the tool letters to the tools.
  await part('E9', async () => {
    await page.keyboard.press('Escape')
    const rows = await kit.locator('.design-layer-row').evaluateAll((els) => els.filter((el) => el.offsetParent && el.style.visibility !== 'hidden').map((el) => Number(el.getAttribute('data-row-id'))))
    const first = rows[0]
    await kit.locator(`[data-row-id='${first}'] .design-layer-name`).click()
    await until(async () => JSON.stringify((await inspect()).selection) === JSON.stringify([first]))
    const before = (await node(first)).absoluteBounds
    await page.keyboard.press('r')
    step('E9 R on a clicked layer row picks the Rectangle tool', await until(() => toolIs('rectangle'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
    await page.keyboard.press('v')
    await until(() => toolIs('move'), 40)
    await kit.locator(`[data-row-id='${first}'] .design-layer-name`).click()
    await page.keyboard.press('ArrowDown')
    const moved = await until(async () => (await inspect()).selection[0] !== first, 30)
    const after = (await node(first)).absoluteBounds
    step('E9 Down on a layer row selects the next row (and moves nothing)', moved && after.x === before.x && after.y === before.y, `${JSON.stringify((await inspect()).selection)} ${JSON.stringify(after)}`)
  })

  // The login page walk: Cmd+R renames inline, Enter, then Shift+A adds auto layout to the empty frame.
  await part('rename then Shift+A', async () => {
    const added = await run('frame.add', { x: 600, y: 1000, width: 200, height: 120, name: 'Empty' })
    const id = added?.id ?? (await inspect()).selection[0]
    await run('select.set', { ids: [] })
    const at = await centre(id)
    await page.mouse.click(at.x, at.y)
    await until(async () => JSON.stringify((await inspect()).selection) === JSON.stringify([id]))
    await page.keyboard.press(`${mod}+r`)
    await until(async () => (await kit.getByTestId('design-layer-rename').count()) === 1)
    await page.keyboard.type('Screen')
    await page.keyboard.press('Enter')
    await until(async () => (await node(id)).name === 'Screen')
    step('Cmd+R renames the frame inline', (await node(id)).name === 'Screen', (await node(id)).name)
    await page.keyboard.press('Shift+A')
    const mode = async () => (await node(id)).layout?.mode ?? 'none'
    step('Shift+A right after the inline rename adds auto layout to the empty frame', await until(async () => (await mode()) !== 'none', 40), `${await mode()}, focus ${await focused()}`)
  })

  // E4: renaming a layer, Tab saves the name and goes on renaming the next row; Shift+Tab the row before.
  await part('E4', async () => {
    await page.keyboard.press('Escape')
    const rows = await kit.locator('.design-layer-row').evaluateAll((els) => els.filter((el) => el.offsetParent && el.style.visibility !== 'hidden').map((el) => Number(el.getAttribute('data-row-id'))))
    const [first, second] = rows
    const names = [(await node(first)).name, (await node(second)).name]
    await kit.locator(`[data-row-id='${first}'] .design-layer-name`).dblclick()
    await until(async () => (await kit.getByTestId('design-layer-rename').count()) === 1)
    await page.keyboard.type('Renamed one')
    await page.keyboard.press('Tab')
    await page.waitForTimeout(400)
    const field = kit.getByTestId('design-layer-rename')
    const next = (await field.count()) === 1 ? await field.inputValue() : null
    step('E4 Tab saves the name and renames the next row', (await node(first)).name === 'Renamed one' && next === names[1] && (await focused()).includes('design-layer-rename'), `first ${(await node(first)).name}, field ${next}, focus ${await focused()}`)
    await page.keyboard.type('Renamed two')
    await page.keyboard.press('Shift+Tab')
    await page.waitForTimeout(400)
    const back = (await field.count()) === 1 ? await field.inputValue() : null
    step('E4 Shift+Tab saves it and goes back a row', (await node(second)).name === 'Renamed two' && back === 'Renamed one', `second ${(await node(second)).name}, field ${back}`)
    await page.keyboard.press('Escape')
    await until(async () => (await kit.getByTestId('design-layer-rename').count()) === 0)
    for (const [id, name] of [[first, names[0]], [second, names[1]]]) await run('object.rename', { ids: [id], name })
  })

  // E10: a new page starts with its name to type; Enter keeps it and adds no second page.
  await part('E10', async () => {
    const pages = async () => (await designKit(() => window.designKit.query('pages'))).pages
    const count = (await pages()).length
    const was = (await inspect()).page
    await kit.getByTestId('design-pages-add').click()
    const opened = await until(async () => (await kit.getByTestId('design-page-rename').count()) === 1 && (await focused()).includes('design-page-rename'), 40)
    step('E10 a new page opens its name for typing', opened, `focus ${await focused()}`)
    await page.keyboard.type('Login')
    await page.keyboard.press('Enter')
    await until(async () => (await pages()).some((p) => p.name === 'Login'), 40)
    await page.waitForTimeout(300)
    const now = await pages()
    step('E10 the name typed is the page\'s, and Enter adds no other page', now.length === count + 1 && now.some((p) => p.name === 'Login'), now.map((p) => p.name).join(', '))
    await page.keyboard.press('r')
    step('E10 after Enter the keys are the canvas\'s again', await until(() => toolIs('rectangle'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
    await page.keyboard.press('v')
    await until(() => toolIs('move'), 40)
    const before = new Set((await designKit(() => window.designKit.query('pages'))).pages.slice(0, count).map((p) => p.id))
    if ((await inspect()).page !== was) await run('page.select', { id: was })
    for (const p of await pages()) if (!before.has(p.id)) await run('page.delete', { id: p.id })
    await page.waitForTimeout(400)
  })

  // A21: the number keys set the selection's opacity (1…9: 10 %…90 %, 0: 100 %, two quick digits: exact).
  await part('A21', async () => {
    await page.keyboard.press('Escape')
    await selectRect()
    const opacity = async () => (await node(rect)).opacity ?? 1
    await page.keyboard.press('5')
    step('A21 5 sets 50 % opacity', await until(async () => Math.abs((await opacity()) - 0.5) < 1e-6, 40), String(await opacity()))
    await page.waitForTimeout(800)
    await page.keyboard.press('4')
    await page.keyboard.press('5')
    step('A21 4 then 5 sets 45 %', await until(async () => Math.abs((await opacity()) - 0.45) < 1e-6, 40), String(await opacity()))
    await page.waitForTimeout(800)
    await page.keyboard.press('0')
    step('A21 0 sets 100 %', await until(async () => Math.abs((await opacity()) - 1) < 1e-6, 40), String(await opacity()))
    await page.mouse.click(empty.x, empty.y)
    await until(async () => (await inspect()).selection.length === 0)
    const pageBefore = (await inspect()).page
    await page.keyboard.press('3')
    await page.waitForTimeout(400)
    step('A21 a digit with nothing selected changes nothing', (await opacity()) === 1 && (await inspect()).page === pageBefore)
  })

  // C2: A is the Frame tool's second key.
  await part('C2', async () => {
    await page.keyboard.press('a')
    step('C2 A picks the Frame tool', await until(() => toolIs('frame'), 40), JSON.stringify(await tool()))
    await page.keyboard.press('Escape')
    await until(() => toolIs('move'), 40)
  })

  // B13: N and Shift+N go to the next and previous top-level frame.
  await part('B13', async () => {
    await page.mouse.click(empty.x, empty.y)
    await until(async () => (await inspect()).selection.length === 0)
    const start = await view()
    await page.keyboard.press('n')
    const moved = await until(async () => {
      const now = await view()
      return Math.abs(now.x - start.x) > 1 || Math.abs(now.zoom - start.zoom) > 1e-6
    }, 40)
    const next = await view()
    step('B13 N zooms to the next frame', moved, `${JSON.stringify(start)} -> ${JSON.stringify(next)}, focus ${await focused()}`)
    await page.keyboard.press('Shift+N')
    const back = await until(async () => Math.abs((await view()).x - next.x) > 1, 40)
    step('B13 Shift+N zooms to the previous frame', back, JSON.stringify(await view()))
    await page.keyboard.press('Escape')
    await until(async () => (await inspect()).selection.length === 0)
  })

  // B12: with nothing selected the arrows pan the view 64 px (Shift: 320 px), whatever the zoom.
  await part('B12', async () => {
    await page.mouse.click(empty.x, empty.y)
    await until(async () => (await inspect()).selection.length === 0)
    const start = await view()
    await page.keyboard.press('ArrowRight')
    await until(async () => Math.abs((await view()).x - start.x) > 1e-6, 30)
    const right = await view()
    step('B12 Right pans the view 64 px right', Math.abs((right.x - start.x) * right.zoom - 64) < 0.5 && Math.abs(right.y - start.y) < 1e-6, `${((right.x - start.x) * right.zoom).toFixed(2)} px`)
    await page.keyboard.press('Shift+ArrowDown')
    await until(async () => Math.abs((await view()).y - right.y) > 1e-6, 30)
    const down = await view()
    step('B12 Shift+Down pans it 320 px down', Math.abs((down.y - right.y) * down.zoom - 320) < 0.5, `${((down.y - right.y) * down.zoom).toFixed(2)} px`)
    step('B12 panning is no edit', (await inspect()).selection.length === 0 && !(await designKit(() => window.designKit.editor.session.dirty)) === !(await designKit(() => window.designKit.editor.session.dirty)))
  })

  // I3: Escape closes the Local variables window, and the keyboard is the canvas's after it.
  await part('I3', async () => {
    await kit.getByTestId('design-rail-variables').click()
    await until(async () => (await kit.getByTestId('design-variables').count()) === 1)
    await page.keyboard.press('Escape')
    step('I3 Escape closes the Local variables window', await until(async () => (await kit.getByTestId('design-variables').count()) === 0, 30))
    await page.keyboard.press('r')
    step('I3 then R picks the Rectangle tool', await until(() => toolIs('rectangle'), 40), `${JSON.stringify(await tool())}, focus ${await focused()}`)
    await page.keyboard.press('Escape')
    await until(() => toolIs('move'), 40)
  })

  // R-A3: a double-click on a frame's name label renames it there (Enter commits, Escape keeps the name).
  await part('rename label', async () => {
    await page.mouse.click(empty.x, empty.y)
    const before = await view()
    await page.keyboard.press('Shift+1')
    await until(async () => (await view()).zoom !== before.zoom || (await view()).x !== before.x, 30)
    await page.waitForTimeout(400)
    const labelAt = async (id) => {
      const chrome = await designKit(async () => JSON.parse(await window.designKit.invoke('chrome', window.designKit.canvas.current.view().zoom)))
      const label = chrome.labels.find((l) => l.id === id)
      if (!label) return null
      const [a, , c] = label.quad
      const v = await view()
      return { x: surface.x + ((a[0] + c[0]) / 2 - v.x) * v.zoom, y: surface.y + ((a[1] + c[1]) / 2 - v.y) * v.zoom, name: label.name }
    }
    const doc = await designKit(() => window.designKit.query('document', { depth: 1 }))
    const target = doc.pages[0].children.find((c) => c.type === 'FRAME')
    const at = await labelAt(target.id)
    await page.mouse.dblclick(at.x, at.y)
    const field = kit.getByTestId('design-label-rename')
    step('a double-click on a frame label opens a field over it', await until(async () => (await field.count()) === 1, 40), `label at ${Math.round(at.x)},${Math.round(at.y)}`)
    step('holding the name', (await field.inputValue().catch(() => null)) === target.name)
    await page.keyboard.type('Login')
    await page.keyboard.press('Enter')
    step('Enter renames the frame', await until(async () => (await node(target.id)).name === 'Login', 40), (await node(target.id)).name)
    step('and the field goes', (await field.count()) === 0)
    await page.keyboard.press('r')
    step('the keyboard is the canvas\'s after it (R)', await until(() => toolIs('rectangle'), 40), `focus ${await focused()}`)
    await page.keyboard.press('Escape')
    await until(() => toolIs('move'), 40)
    const again = await labelAt(target.id)
    await page.mouse.dblclick(again.x, again.y)
    await until(async () => (await field.count()) === 1, 40)
    await page.keyboard.type('Nope')
    await page.keyboard.press('Escape')
    await until(async () => (await field.count()) === 0, 40)
    await page.waitForTimeout(300)
    step('Escape keeps the name', (await node(target.id)).name === 'Login' && (await field.count()) === 0, (await node(target.id)).name)
  })

  // F8 (keyboard marquee) is not offered: the shortcuts dialog does not list it.
  await part('F8', async () => {
    await page.mouse.click(empty.x, empty.y)
    await page.keyboard.press('Control+Shift+Slash')
    await until(async () => (await kit.getByTestId('design-dialog-shortcuts').count()) === 1)
    await kit.getByTestId('design-shortcuts-search').fill('Select with a box')
    await page.waitForTimeout(400)
    step('the shortcuts dialog lists no keyboard marquee (F8)', (await kit.getByTestId('design-shortcut-row').count()) === 0, String(await kit.getByTestId('design-shortcut-row').count()))
    await kit.getByTestId('design-dialog-shortcuts-close').click()
  })
} catch (error) {
  failures += 1
  shots.failed('error')
  console.log(`FAIL ${error instanceof Error ? error.message.split('\n').slice(0, 12).join(' / ') : String(error)}`)
} finally {
  if (errors.length) console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
  await shots.settled()
  await browser.close()
  await server?.close()
}
process.exit(failures ? 1 : 0)
