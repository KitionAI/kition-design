// The editor's flow in a real browser, through the development host: node tests/flow.mjs [chromium|webkit]
// Starts the Vite dev server itself, opens dev.html with the sample, and prints PASS or FAIL per step.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { evidence } from './evidence.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
const page = await context.newPage()
const shots = evidence(page, 'flow', which)
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

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  // The editor's state, as development builds expose it (`window.designKit`).
  const designKit = (body) => frame().evaluate(body)
  const inspect = () => designKit(() => window.designKit.inspect)
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true)
  await page.waitForTimeout(800)
  const canvas = await kit.getByTestId('kit-canvas-surface').boundingBox()
  /** A document pixel on the page. */
  const toPage = async (x, y) => {
    const view = await designKit(() => window.designKit.canvas.current.view())
    return { x: canvas.x + (x - view.x) * view.zoom, y: canvas.y + (y - view.y) * view.zoom }
  }

  step('the shell draws the toolbar, rail and header', (await kit.getByTestId('design-toolbar').count()) === 1 && (await kit.getByTestId('design-rail').count()) === 1 && (await kit.getByTestId('design-tab-design').getAttribute('aria-selected')) === 'true')

  // Draw a rectangle on empty canvas.
  await kit.getByTestId('design-caret-shape').click()
  await kit.getByTestId('design-flyout-tool-rectangle').click()
  await until(async () => (await kit.getByTestId('design-tool-shape').getAttribute('aria-pressed')) === 'true')
  step('chooses Rectangle from the shape flyout', (await kit.getByTestId('design-tool-shape').getAttribute('aria-pressed')) === 'true')
  const before = (await inspect()).layers
  const from = { x: canvas.x + 120, y: canvas.y + canvas.height - 200 }
  await page.mouse.move(from.x, from.y)
  await page.mouse.down()
  await page.mouse.move(from.x + 50, from.y + 30, { steps: 6 })
  await page.mouse.move(from.x + 90, from.y + 60, { steps: 6 })
  await page.mouse.up()
  await until(async () => (await inspect()).layers === before + 1)
  const drawn = await inspect()
  step('draws a rectangle in the engine', drawn.layers === before + 1 && drawn.selection.length === 1, `${before} → ${drawn.layers}`)

  await kit.getByTestId('design-undo').click()
  await until(async () => (await inspect()).layers === before)
  step('undoes it', (await inspect()).layers === before)

  // Select a frame by its name label, as upstream does.
  await kit.getByTestId('design-tool-move').click()
  await page.keyboard.press('Escape')
  const document = await designKit(() => window.designKit.query('document', { depth: 1 }))
  const area = (await inspect()).area
  const dark = document.pages[0].children.find((child) => child.name === 'Home — Dark')
  const label = await toPage(dark.absoluteBounds.x - area.x + 20, dark.absoluteBounds.y - area.y)
  await page.mouse.click(label.x, label.y - 9)
  await until(async () => (await inspect()).selection.includes(dark.id))
  step('selects a frame by its label', (await inspect()).selection.includes(dark.id))
  step('the Design panel follows the selection', await until(async () => (await kit.getByTestId('design-design-panel').textContent()).includes('Frame')))

  // An auto layout padding drag is one undo step.
  await designKit(() => window.designKit.run('select.set', { ids: [79] }))
  await designKit(() => window.designKit.viewTo('view.zoomSelection'))
  await page.waitForTimeout(600)
  const padding = async () => (await designKit(() => window.designKit.query('node', { id: 79 }))).layout.padding[3]
  const zone = kit.getByTestId('design-al-padding-3')
  await until(async () => (await zone.count()) === 1)
  const box = await zone.boundingBox()
  const left = await padding()
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.down()
  for (let i = 1; i <= 8; i += 1) await page.mouse.move(box.x + box.width / 2 + i * 6, box.y + box.height / 2)
  await page.mouse.up()
  await until(async () => (await padding()) > left)
  const dragged = await padding()
  step('drags the left padding', dragged > left, `${left} → ${dragged}`)
  await page.waitForTimeout(300)
  await kit.getByTestId('design-undo').click()
  await until(async () => (await padding()) === left)
  step('one undo takes the whole drag back', (await padding()) === left, `${await padding()}`)
  // A drag the browser cancels (pointercancel) puts the padding back (review #25d).
  const zoneNow = await zone.boundingBox()
  await page.mouse.move(zoneNow.x + zoneNow.width / 2, zoneNow.y + zoneNow.height / 2)
  await page.mouse.down()
  for (let i = 1; i <= 8; i += 1) await page.mouse.move(zoneNow.x + zoneNow.width / 2 + i * 6, zoneNow.y + zoneNow.height / 2)
  const draggedAgain = await until(async () => (await padding()) > left)
  await zone.dispatchEvent('pointercancel', { pointerId: 1, isPrimary: true })
  await page.mouse.up()
  await until(async () => (await designKit(() => window.designKit.query('toolbar'))).interaction === null)
  step('a cancelled padding drag puts the padding back', draggedAgain && (await until(async () => (await padding()) === left)), `${left} → ${await padding()}`)
  if ((await padding()) !== left) await kit.getByTestId('design-undo').click()
  await until(async () => (await padding()) === left)

  // Interactions that overlap stay one undo step each (review #15): B starts before A's commit
  // has landed, and A's commit must not close B.
  const history = () => designKit(() => window.designKit.invoke('query', 'history', '{}').then((text) => JSON.parse(text).undo))
  const opacity = async () => (await designKit(() => window.designKit.query('node', { id: 79 }))).opacity ?? 1
  const near = async (value) => Math.abs((await opacity()) - value) < 1e-4
  const settled = () => until(async () => (await designKit(() => window.designKit.query('toolbar'))).interaction === null)
  const overlapped = await designKit(async () => {
    const set = (value) => ['node.set', { ids: [79], props: { opacity: value } }]
    const a = window.designKit.interaction('Overlap A')
    a.update(...set(0.9))
    const b = window.designKit.interaction('Overlap B')
    b.update(...set(0.8))
    const doneA = a.commit()
    b.update(...set(0.7))
    await doneA
    b.update(...set(0.6))
    await b.commit()
    return true
  })
  await settled()
  const overlapHistory = (await history()).slice(0, 2)
  step('overlapping interactions are one step each', overlapped && overlapHistory.join() === 'Overlap B,Overlap A' && (await near(0.6)), `${JSON.stringify(overlapHistory)} ${await opacity()}`)
  await designKit(() => window.designKit.run('edit.undo'))
  step('one undo takes the second one back whole', await until(async () => (await near(0.9))), String(await opacity()))
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await near(1)))
  // B begins right after A's commit was asked for, before it landed (a slow worker: A's update
  // takes 300 ms, so A's commit waits behind it).
  await designKit(async () => {
    const session = window.designKit.editor.session
    const run = session.run
    let runs = 0
    session.run = (...args) => (++runs === 1 ? new Promise((resolve) => setTimeout(resolve, 300)).then(() => run.apply(session, args)) : run.apply(session, args))
    const set = (value) => ['node.set', { ids: [79], props: { opacity: value } }]
    const a = window.designKit.interaction('Overlap A')
    a.update(...set(0.9))
    const doneA = a.commit()
    const b = window.designKit.interaction('Overlap B')
    b.update(...set(0.8))
    await new Promise((resolve) => setTimeout(resolve, 100))
    b.update(...set(0.7))
    await doneA
    await b.commit()
    session.run = run
  })
  await settled()
  const afterCommit = (await history()).slice(0, 2)
  step('an interaction begun while the last one commits is its own step', afterCommit.join() === 'Overlap B,Overlap A' && (await near(0.7)), `${JSON.stringify(afterCommit)} ${await opacity()}`)
  await designKit(() => window.designKit.run('edit.undo'))
  step('one undo takes it back whole', await until(async () => (await near(0.9))), String(await opacity()))
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await near(1)))

  // A beginInteraction that fails for a reason other than a missing method does not turn
  // grouping off for the session (review #20).
  await designKit(() => {
    const doc = window.designKit.doc
    const invoke = doc.invoke.bind(doc)
    let failed = false
    doc.invoke = (name, ...args) => {
      if (name === 'beginInteraction' && !failed) {
        failed = true
        return Promise.reject(new Error('the engine is busy'))
      }
      return invoke(name, ...args)
    }
  })
  await designKit(async () => {
    const failed = window.designKit.interaction('Refused')
    failed.update('node.set', { ids: [79], props: { opacity: 0.95 } })
    await failed.commit()
  })
  await settled()
  await designKit(async () => {
    const next = window.designKit.interaction('Grouped')
    next.update('node.set', { ids: [79], props: { opacity: 0.5 } })
    await new Promise((resolve) => setTimeout(resolve, 50))
    next.update('node.set', { ids: [79], props: { opacity: 0.4 } })
    await next.commit()
  })
  await settled()
  const afterRefusal = (await history())[0]
  step('the next interaction after a failed begin is still one step', afterRefusal === 'Grouped' && (await designKit(() => window.designKit.interactions)) === true, `${afterRefusal} ${await designKit(() => window.designKit.interactions)}`)
  await designKit(() => window.designKit.run('edit.undo'))
  step('and one undo takes it back', await until(async () => (await near(0.95))), String(await opacity()))
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await near(1)))

  // Save through the host.
  await kit.getByTestId('design-redo').click()
  await until(async () => (await padding()) === dragged)
  await kit.getByTestId('design-save').click()
  await until(async () => (await page.evaluate(() => window.devHost.files.get('Sample.kdesign')?.version ?? 0)) > 1)
  step('saves through the host', (await page.evaluate(() => window.devHost.files.get('Sample.kdesign')?.version ?? 0)) > 1)
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
