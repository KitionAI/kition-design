// The Design panel's structure (WP3) in a real browser, through the development host:
// node tests/design.mjs [chromium|webkit]. Starts the Vite dev server itself, opens dev.html with
// the sample, drives the panel's controls and checks the engine's document after each.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
const page = await context.newPage()
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
const step = (name, ok, detail = '') => {
  if (!ok) failures += 1
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` - ${detail}` : ''}`)
}
const until = async (check, tries = 60) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(100)
  }
  return false
}

// Sample ids (public/sample/Sample.kdesign): an auto layout frame, a top-level frame, a rectangle in it,
// a component, and an instance of it.
const AL_FRAME = 79
const TOP_FRAME = 68
const RECT = 177
const COMPONENT = 29
const INSTANCE = 105
const SET = 28 // the Button component set
const IN_INSTANCE = 237 // a text layer inside an instance
const IN_COMPONENT = 30 // an instance inside the component

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  const designKit = (body, arg) => frame().evaluate(body, arg)
  const node = (id) => designKit((i) => window.designKit.query('node', { id: i }), id)
  const select = async (ids) => {
    await designKit((next) => window.designKit.run('select.set', { ids: next }), ids)
    await until(async () => (await designKit(() => window.designKit.inspect.selection)).join() === ids.join())
    await page.waitForTimeout(250)
  }
  const byTest = (id) => kit.getByTestId(id)
  /** Types into a field (a number or text field's input) and commits with Enter. */
  const type = async (testId, text) => {
    const input = byTest(testId).locator('input')
    await input.click()
    await input.fill(text)
    await input.press('Enter')
  }
  const undo = async () => {
    await kit.getByTestId('design-undo').click()
    await page.waitForTimeout(250)
  }
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true)
  await page.waitForTimeout(600)

  // Nothing selected: the page's sections.
  await select([])
  const panel = byTest('design-design-panel')
  const pageText = await panel.textContent()
  step('nothing selected shows Page, Color profile, Local variables, Local styles, Export', ['Page', 'Color profile', 'Local variables', 'Local styles', 'Export page as PNG'].every((label) => pageText.includes(label)))
  step('Open variables counts the local variables', /Open variables \(\d+\)/.test(pageText))
  await type('design-design-page-background', 'FFEECC')
  step('the page background field sets the page colour', await until(async () => (await designKit(() => window.designKit.inspect.background)).toUpperCase().includes('FFEECC')))
  await undo()

  // An auto layout frame: header, Position, Auto layout.
  await select([AL_FRAME])
  step('the header names the selection', (await byTest('design-design-type').textContent()).includes('Frame'))
  step('an auto layout frame shows Position, Auto layout and Appearance', (await byTest('design-design-position').count()) === 1 && (await byTest('design-design-layout').textContent()).includes('Auto layout') && (await byTest('design-design-appearance').count()) === 1)
  await type('design-design-x', '24')
  step('X writes the frame-relative position', await until(async () => (await node(AL_FRAME)).x === 24), String((await node(AL_FRAME)).x))
  await type('design-design-y', '+10')
  step('Y takes relative math', await until(async () => (await node(AL_FRAME)).y === 58), String((await node(AL_FRAME)).y))
  await undo()
  await undo()

  await byTest('design-design-direction-horizontal').click()
  step('the direction segments set the auto layout direction', await until(async () => (await node(AL_FRAME)).layout.mode === 'horizontal'))
  await byTest('design-design-align-cell-2-2').click()
  step('the alignment grid sets main and cross alignment', await until(async () => {
    const l = (await node(AL_FRAME)).layout
    return l.mainAlign === 'max' && l.crossAlign === 'max'
  }))
  await type('design-design-gap', '12')
  step('the gap field sets the gap', await until(async () => (await node(AL_FRAME)).layout.gap === 12))
  await byTest('design-design-gap-mode').click()
  await byTest('design-design-gap-mode-spaceBetween').click()
  step('the auto gap menu sets space between', await until(async () => (await node(AL_FRAME)).layout.mainAlign === 'spaceBetween'))
  await byTest('design-design-padding-individual').click()
  step('individual padding shows a field per side', await until(async () => (await byTest('design-design-paddingLeft').count()) === 1 && (await byTest('design-design-paddingBottom').count()) === 1))
  await type('design-design-paddingLeft', '30')
  step('the left padding field sets one side', await until(async () => (await node(AL_FRAME)).layout.padding[3] === 30))
  await byTest('design-design-padding-individual').click()
  await byTest('design-design-wrap').click()
  step('the wrap toggle wraps the layout', await until(async () => (await node(AL_FRAME)).layout.wrap === true))
  await byTest('design-design-sizingV').click()
  await byTest('design-design-sizingV-fixed').click()
  step('the sizing menu sets fixed height', await until(async () => (await node(AL_FRAME)).sizing.vertical === 'fixed'))
  await byTest('design-design-sizingH').click()
  await byTest('design-design-minWidth-add').click()
  step('Add min width adds a limit row', await until(async () => (await node(AL_FRAME)).minWidth !== undefined && (await byTest('design-design-minWidth').count()) === 1))
  await byTest('design-design-strokes').click()
  await byTest('design-design-strokes-excluded').click()
  step('Strokes: excluded from layout', await until(async () => (await node(AL_FRAME)).layout.strokesIncluded === false))
  await byTest('design-design-clip').click()
  step('Clip content toggles clipping', await until(async () => (await node(AL_FRAME)).clip === true))

  // Remove auto layout from the header's button, and undo it.
  await byTest('design-design-layout-remove').click()
  step('Remove auto layout', await until(async () => (await node(AL_FRAME)).layout === undefined))
  await undo()
  step('undo brings the auto layout back', await until(async () => (await node(AL_FRAME)).layout !== undefined))

  // A grid: track sizes from the engine's resolved geometry (layout.gridGeometry).
  await byTest('design-design-direction-grid').click()
  await until(async () => (await node(AL_FRAME)).layout.mode === 'grid')
  await until(async () => (await byTest('design-design-track-columns-0-sizing').count()) === 1)
  await byTest('design-design-track-columns-0-sizing').click()
  await byTest('design-design-track-columns-0-sizing-hug').click()
  await until(async () => (await node(AL_FRAME)).layout.columnTracks?.[0]?.sizing === 'hug')
  const geometry = await designKit((id) => window.designKit.query('layout.gridGeometry', { id }), AL_FRAME)
  const hugSize = String(Math.round(geometry.columns[0].size * 100) / 100)
  step('a hug track shows the size the engine resolves', await until(async () => (await byTest('design-design-track-columns-0').locator('input').inputValue()) === hugSize), `${await byTest('design-design-track-columns-0').locator('input').inputValue()} vs ${hugSize}`)
  await undo()
  await undo()

  // A scrub on the opacity lead is one undo step.
  const before = (await node(AL_FRAME)).opacity ?? 1
  const lead = byTest('design-design-opacity').locator('.design-field-lead')
  await lead.scrollIntoViewIfNeeded()
  const box = await lead.boundingBox()
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.down()
  for (let i = 1; i <= 10; i += 1) await page.mouse.move(box.x + box.width / 2 - i * 4, box.y + box.height / 2)
  await page.mouse.up()
  const scrubbed = await until(async () => ((await node(AL_FRAME)).opacity ?? 1) < before)
  step('scrubbing the opacity lead changes the opacity', scrubbed, String((await node(AL_FRAME)).opacity))
  await page.waitForTimeout(300)
  await undo()
  step('one undo takes the whole scrub back', await until(async () => ((await node(AL_FRAME)).opacity ?? 1) === before), String((await node(AL_FRAME)).opacity))

  // A rectangle: constraints, opacity, corners, blend.
  await select([RECT])
  // The W/H link reads and writes constrainProportions.
  const link = byTest('design-design-constrain')
  step('the W/H link shows the layer\'s state', (await link.getAttribute('aria-pressed')) === String((await node(RECT)).constrainProportions === true))
  await link.click()
  step('the W/H link turns constrain proportions on', await until(async () => (await node(RECT)).constrainProportions === true && (await link.getAttribute('aria-pressed')) === 'true'))
  await undo()
  await until(async () => (await node(RECT)).constrainProportions !== true)
  await byTest('design-design-constraint-h').click()
  await byTest('design-design-constraint-h-right').click()
  step('the horizontal constraint menu sets Right', await until(async () => (await node(RECT)).constraints.horizontal === 'right'))
  await type('design-design-opacity', '50')
  step('opacity takes a percentage', await until(async () => (await node(RECT)).opacity === 0.5))
  await type('design-design-radius', '6')
  step('corner radius sets every corner', await until(async () => (await node(RECT)).cornerRadius === 6))
  await byTest('design-design-corners-individual').click()
  await type('design-design-topLeftRadius', '2')
  step('individual corners set one corner', await until(async () => JSON.stringify((await node(RECT)).cornerRadius) === '[2,6,6,6]'), JSON.stringify((await node(RECT)).cornerRadius))
  await byTest('design-design-corners-individual').click()
  await byTest('design-design-blend-toggle').click()
  await byTest('design-design-blend').click()
  await byTest('design-design-blend-multiply').click()
  step('the blend menu sets the blend mode', await until(async () => String((await node(RECT)).blend).toLowerCase() === 'multiply'), String((await node(RECT)).blend))
  await byTest('design-design-visibility').click()
  step('Show/Hide hides the layer', await until(async () => (await node(RECT)).visible === false))
  await undo()

  // Parent facts from the engine: the status block, Position inside an instance.
  await select([AL_FRAME])
  step('a frame inside a frame shows no handoff status', (await byTest('design-design-status').count()) === 0)
  await select([IN_INSTANCE])
  step('Position is disabled inside an instance', (await byTest('design-design-position').getAttribute('data-disabled')) === 'true')

  // Spacing between top-level frames in a row (arrange.spacingInfo, arrange.spacing).
  await select([TOP_FRAME, 178])
  const spacing = byTest('design-design-spacing-h').locator('input')
  await until(async () => (await spacing.count()) === 1)
  step('the Spacing row shows the gap between the selection', (await spacing.inputValue()) === '80', await spacing.inputValue().catch(() => 'none'))
  const x178 = (await node(178)).x
  await type('design-design-spacing-h', '100')
  step('the Spacing field moves the layers apart', await until(async () => (await node(178)).x === x178 + 20), `${x178} → ${(await node(178)).x}`)
  await undo()
  await until(async () => (await node(178)).x === x178)

  // Typed math never answers a value that is not finite (review #25c).
  const math = await designKit(() => import('/src/ui/math.ts').then((m) => {
    // What each edit does to a value of 10 (null: no edit); JSON would turn Infinity into null.
    const at = (edit) => (edit === null ? 'none' : String(edit.kind === 'set' ? edit.value : edit.apply(10)))
    return { set: at(m.parseEdit('*1e308', 10)), relative: at(m.parseEdit('*1e308', null)), divide: at(m.parseEdit('/1e-320', 10)) }
  }))
  step('typed math that overflows commits nothing', Object.values(math).every((v) => v === 'none' || Number.isFinite(Number(v))), JSON.stringify(math))

  // Number fields on a mixed selection (X of two frames at different x).
  const xs = async () => [(await node(TOP_FRAME)).x, (await node(178)).x]
  const [xa, xb] = await xs()
  const movedBy = async (d) => {
    const [a, b] = await xs()
    return a === xa + d && b === xb + d
  }
  // Enter commits once: `+10` moves each layer 10, not 20 (review #6).
  await type('design-design-x', '+10')
  await until(async () => movedBy(10))
  await page.waitForTimeout(400)
  step('Enter commits a relative edit once', await movedBy(10), JSON.stringify(await xs()))
  await undo()
  step('so one undo takes it back', await until(async () => movedBy(0)), JSON.stringify(await xs()))
  // Scrubbing moves each layer by the drag's delta from where it started (review #8).
  const xLead = byTest('design-design-x').locator('.design-field-lead')
  const xBox = await xLead.boundingBox()
  const xInput = byTest('design-design-x').locator('input')
  let shownDelta = null
  const scrubX = async (steps, finish) => {
    await page.mouse.move(xBox.x + xBox.width / 2, xBox.y + xBox.height / 2)
    await page.mouse.down()
    for (let i = 1; i <= steps; i += 1) {
      await page.mouse.move(xBox.x + xBox.width / 2 + i * 3, xBox.y + xBox.height / 2)
      await page.waitForTimeout(30)
    }
    await page.waitForTimeout(300)
    // A mixed field shows the drag's delta while it scrubs.
    shownDelta = Number(await xInput.inputValue())
    await finish()
  }
  await scrubX(8, () => page.mouse.up())
  await page.waitForTimeout(400)
  const [sa, sb] = await xs()
  step('a scrub moves each layer of a mixed selection by the drag\'s delta', shownDelta > 0 && sa - xa === shownDelta && sb - xb === shownDelta, `${xa},${xb} → ${sa},${sb} (delta ${shownDelta})`)
  await undo()
  await until(async () => movedBy(0))
  // After Escape cancels a scrub, the next typed value still commits on Tab (review #7).
  await scrubX(4, async () => {
    await page.keyboard.press('Escape')
    await page.mouse.up()
  })
  step('Escape cancels the scrub', await until(async () => movedBy(0)), JSON.stringify(await xs()))
  await xInput.click()
  await xInput.fill('+5')
  await xInput.press('Tab')
  step('a value typed after a cancelled scrub commits', await until(async () => movedBy(5)), JSON.stringify(await xs()))
  await undo()
  await until(async () => movedBy(0))

  // Selection colors: upstream's nested scroll area is as tall as the room left in the panel's
  // viewport below it (at least 64): a long panel shows two rows.
  await select([TOP_FRAME])
  const colorsList = byTest('design-design-colors-list')
  await until(async () => (await colorsList.count()) === 1)
  const listHeight = await colorsList.evaluate((el) => el.getBoundingClientRect().height)
  step('Selection colors shows two rows in a long panel', listHeight === 64, String(listHeight))
  step('and scrolls to the rest', await colorsList.evaluate((el) => el.scrollHeight > el.clientHeight))

  // Variable modes from the engine (variable.modeRows): Auto names the inherited mode.
  await select([TOP_FRAME])
  const theme = byTest('design-design-mode-Theme')
  await until(async () => (await theme.count()) === 1)
  step('Variable modes show Auto with the inherited mode', (await theme.textContent()).includes('Auto (Light)'), await theme.textContent())
  await theme.click()
  await byTest('design-design-mode-Theme-Dark').click()
  step('picking a mode sets the layer\'s own mode', await until(async () => Object.values((await node(TOP_FRAME)).varModes ?? {}).some((m) => String(m).endsWith('mode2'))))
  step('and the dropdown shows it', await until(async () => (await theme.textContent()) === 'Dark'), await theme.textContent())
  await undo()

  // Export: add a setting, change its format, export through the host.
  await select([TOP_FRAME])
  step('a top-level frame shows its handoff status', (await byTest('design-design-status').count()) === 1)
  await byTest('design-design-export-add').click()
  step('+ adds an export setting', await until(async () => ((await node(TOP_FRAME)).exports ?? []).length === 1))
  await byTest('design-design-export-0-format').click()
  await byTest('design-design-export-0-format-svg').click()
  step('the format menu sets SVG', await until(async () => (await node(TOP_FRAME)).exports[0].format === 'svg'))
  await byTest('design-design-export-run').click()
  step('Export writes the file through the host', await until(async () => (await page.evaluate(() => [...window.devHost.files.keys()])).some((name) => name.endsWith('.svg'))), JSON.stringify(await page.evaluate(() => [...window.devHost.files.keys()])))
  // The setting's SVG options reach the file (exportNodeWith).
  const svgText = () => page.evaluate(() => {
    const entry = [...window.devHost.files.entries()].filter(([name]) => name.endsWith('.svg')).pop()
    const data = entry?.[1]
    return data == null ? '' : typeof data === 'string' ? data : new TextDecoder().decode(data instanceof Uint8Array ? data : new Uint8Array(data.bytes ?? data))
  })
  const withoutIds = await svgText()
  await byTest('design-design-export-0-more').click()
  await byTest('design-design-export-0-svgIdAttribute').click()
  await until(async () => (await node(TOP_FRAME)).exports[0].svgIdAttribute === true)
  await page.keyboard.press('Escape')
  await page.evaluate(() => window.devHost.files.clear())
  await byTest('design-design-export-run').click()
  await until(async () => (await svgText()) !== '')
  const ids = (text) => (text.match(/\sid="/g) ?? []).length
  step('Include id attribute reaches the exported SVG', ids(await svgText()) > ids(withoutIds), `${ids(withoutIds)} → ${ids(await svgText())} ids`)

  // Layout grid: add one and open its settings.
  await byTest('design-design-grid-add').click()
  step('+ adds a layout grid', await until(async () => ((await node(TOP_FRAME)).grids ?? []).length === 1))
  await byTest('design-design-grid-0-settings').click()
  await type('design-design-grid-0-size', '16')
  step('the grid settings set the size', await until(async () => (await node(TOP_FRAME)).grids[0].size === 16))

  // Frame presets from the type menu.
  await byTest('design-design-type').click()
  await byTest('design-design-preset-iPhone SE').click()
  step('a frame preset sets the size', await until(async () => {
    const n = await node(TOP_FRAME)
    return n.width === 320 && n.height === 568
  }))

  // A component: create a boolean property.
  await select([COMPONENT])
  const props = async () => (await designKit((id) => window.designKit.query('componentProperties', { id }), COMPONENT)).properties.length
  const count = await props()
  const namesBefore = (await designKit((id) => window.designKit.query('componentProperties', { id }), COMPONENT)).properties.map((p) => p.name)
  await byTest('design-design-add-property').click()
  await byTest('design-design-add-property-boolean').click()
  step('the create property menu adds a boolean property', await until(async () => (await props()) === count + 1))
  step('a component offers Add variant (canAddVariant)', (await byTest('design-design-add-variant').count()) === 1)
  const added = (await designKit((id) => window.designKit.query('componentProperties', { id }), COMPONENT)).properties.find((p) => p.type === 'boolean' && !namesBefore.includes(p.name))?.name

  // A layer inside the component: bind its visibility to the new property (propRefs).
  await select([IN_COMPONENT])
  const bindVisible = byTest('design-design-bind-visible')
  await until(async () => (await bindVisible.count()) === 1)
  step('an unbound field says Not bound', (await bindVisible.textContent()) === 'Not bound')
  await bindVisible.click()
  await byTest('design-design-bind-visible-menu').getByText(added, { exact: true }).click()
  step('binding a property shows its name (propRefs)', await until(async () => (await node(IN_COMPONENT)).propRefs?.visible === added && (await bindVisible.textContent()) === added), JSON.stringify((await node(IN_COMPONENT)).propRefs))
  await undo()

  // Two main components combine as variants (canCombineVariants); a set has a description.
  await select([COMPONENT, 55])
  step('two components offer Combine as variants', (await byTest('design-design-combine').count()) === 1)
  await select([SET])
  const description = byTest('design-design-description-field')
  await until(async () => (await description.count()) === 1)
  step('a component set shows its description', (await description.inputValue()) === ((await node(SET)).description ?? ''))
  await description.fill('Primary and secondary buttons')
  await byTest('design-design-type').focus().catch(() => undefined)
  await description.evaluate((el) => el.blur())
  step('editing it sets the set\'s description', await until(async () => (await node(SET)).description === 'Primary and secondary buttons'), String((await node(SET)).description))
  step('a set offers Add variant', (await byTest('design-design-add-variant').count()) === 1)

  // An instance: a text property.
  await select([INSTANCE])
  step('an instance shows its instance block', (await byTest('design-design-instance').count()) === 1)
  await type('design-design-instance-Title', 'Moraine Lake')
  step('a text property sets the instance property', await until(async () => (await node(INSTANCE)).componentProperties?.Title === 'Moraine Lake'))

  // Nothing selected again; the page's Export button.
  await select([])
  await byTest('design-design-export-page').click()
  step('Export page as PNG writes the page', await until(async () => (await page.evaluate(() => [...window.devHost.files.keys()])).some((name) => name.endsWith('.png'))))
} catch (error) {
  failures += 1
  console.log(`FAIL ${error instanceof Error ? error.message.split('\n')[0] : String(error)}`)
} finally {
  if (errors.length) console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
