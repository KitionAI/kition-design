// The paints package in a real browser, through the development host: node tests/paint.mjs [chromium|webkit]
// Fill, Stroke, Effects, Typography and the colour picker against the engine; prints PASS or FAIL per step.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { evidence, traceEvents, traced } from './evidence.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
const page = await context.newPage()
const shots = evidence(page, 'paint', which)
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

const SAVE = 199 // the "Save" frame: a 22 % white fill, a 35 % white stroke, a background blur
const TITLE = 38 // a text layer (Inter SemiBold 17)
const LINE = 231 // an open vector path with a stroke
// A 2 × 2 PNG (no colour profile).
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR4nGP8z8DAwMDAxMDAwMDAAAANHQEDasKb6QAAAABJRU5ErkJggg==', 'base64')

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  const designKit = (body, arg) => frame().evaluate(body, arg)
  const node = (id) => designKit((n) => window.designKit.query('node', { id: n }), id)
  const inspect = () => designKit(() => window.designKit.inspect)
  const select = (ids) => designKit((list) => window.designKit.run('select.set', { ids: list }), ids)
  // The undo steps' labels, the next one first (the engine's own record).
  const history = () => designKit(() => window.designKit.invoke('query', 'history', '{}').then((text) => JSON.parse(text).undo))
  const undo = async () => {
    // A drag's last updates and its commit land first.
    await until(async () => (await designKit(() => window.designKit.query('toolbar'))).interaction === null)
    await page.waitForTimeout(150)
    await kit.getByTestId('design-undo').click()
    await page.waitForTimeout(250)
  }
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true)
  await page.waitForTimeout(600)

  // ---- Fill ----
  await select([SAVE])
  const fillRow = kit.getByTestId('design-fill-0')
  await until(async () => (await fillRow.count()) === 1)
  step('Fill shows the frame paint', (await kit.getByTestId('design-fill-0-hex').textContent()) === 'FFFFFF' && (await kit.getByTestId('design-fill-0-opacity').locator('input').inputValue()) === '22%')

  await kit.getByTestId('design-fill-0-hex').click()
  await kit.locator('input.design-inline-text').fill('ff0000')
  await kit.locator('input.design-inline-text').press('Enter')
  await until(async () => (await node(SAVE)).fills[0].color === '#FF0000')
  step('typing a hex sets the fill (fill.set)', (await node(SAVE)).fills[0].color === '#FF0000')

  const opacityInput = kit.getByTestId('design-fill-0-opacity').locator('input')
  await opacityInput.click()
  await opacityInput.fill('50')
  await opacityInput.press('Enter')
  await until(async () => Math.abs((await node(SAVE)).fills[0].opacity - 0.5) < 0.01)
  step('typing an opacity sets it', Math.abs((await node(SAVE)).fills[0].opacity - 0.5) < 0.01)

  await kit.getByTestId('design-fill-0-visible').click()
  await until(async () => (await node(SAVE)).fills[0].visible === false)
  step('the eye hides the paint', (await node(SAVE)).fills[0].visible === false)
  await kit.getByTestId('design-fill-0-visible').click()
  await until(async () => (await node(SAVE)).fills[0].visible !== false)

  // ---- The colour picker ----
  await kit.getByTestId('design-fill-0-swatch').click()
  const picker = kit.getByTestId('design-color-picker')
  await until(async () => (await picker.count()) === 1)
  step('the swatch opens the colour picker', (await picker.count()) === 1)

  const before = (await node(SAVE)).fills[0].color
  const sv = await kit.getByTestId('design-color-picker-sv').boundingBox()
  await page.mouse.move(sv.x + 30, sv.y + 30)
  await page.mouse.down()
  for (let i = 1; i <= 6; i += 1) await page.mouse.move(sv.x + 30 + i * 20, sv.y + 30 + i * 15)
  await page.mouse.up()
  await until(async () => (await node(SAVE)).fills[0].color !== before)
  const dragged = (await node(SAVE)).fills[0].color
  step('dragging in the square changes the colour', dragged !== before, `${before} → ${dragged}`)
  await until(async () => (await designKit(() => window.designKit.query('toolbar'))).interaction === null)
  step('the drag is one step named after its interaction (history)', await until(async () => (await history())[0] === 'Color'), JSON.stringify((await history()).slice(0, 2)))
  step('and the Undo label says so (inspect)', await until(async () => (await inspect()).undo === 'Color'), String((await inspect()).undo))
  step('On this page shows translucent colours with their alpha (documentColors css)', (await kit.getByTestId('design-color-picker-page').locator('[data-color="#FFFFFFD9"]').count()) === 1)
  await page.waitForTimeout(300)
  await undo()
  await until(async () => (await node(SAVE)).fills[0].color === before)
  step('one undo takes the whole colour drag back', (await node(SAVE)).fills[0].color === before, `${(await node(SAVE)).fills[0].color}`)

  const hex = kit.getByTestId('design-color-picker-hex').locator('input')
  await hex.click()
  await hex.fill('3366CC')
  await hex.press('Enter')
  await until(async () => (await node(SAVE)).fills[0].color === '#3366CC')
  step('the picker hex field writes the colour', (await node(SAVE)).fills[0].color === '#3366CC')

  await kit.getByTestId('design-color-picker-model').click()
  await kit.getByTestId('design-color-picker-model-RGB').click()
  const red = kit.getByTestId('design-color-picker-rgb-0').locator('input')
  await red.click()
  await red.fill('255')
  await red.press('Enter')
  await until(async () => (await node(SAVE)).fills[0].color === '#FF66CC')
  step('the RGB fields write the colour', (await node(SAVE)).fills[0].color === '#FF66CC')
  await kit.getByTestId('design-color-picker-model').click()
  await kit.getByTestId('design-color-picker-model-Hex').click()

  await kit.getByTestId('design-color-picker-kind-linear').click()
  await until(async () => (await node(SAVE)).fills[0].type === 'gradient')
  step('Linear turns the paint into a gradient', (await node(SAVE)).fills[0].kind === 'linear')
  step('the gradient stop bar shows its stops', await until(async () => (await kit.getByTestId('design-color-picker-stop-1').count()) === 1))
  step('the canvas shows the gradient handles (paintHandles)', await until(async () => (await kit.getByTestId('design-gradient-handles').count()) === 1))

  await designKit(() => window.designKit.viewTo('view.zoomSelection'))
  await page.waitForTimeout(600)
  const handle = kit.getByTestId('design-gradient-handle-1')
  await until(async () => (await handle.count()) === 1)
  const hb = await handle.boundingBox()
  const fromTo = (await node(SAVE)).fills[0].to
  await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2)
  await page.mouse.down()
  for (let i = 1; i <= 5; i += 1) await page.mouse.move(hb.x + hb.width / 2 + i * 8, hb.y + hb.height / 2 - i * 4)
  await page.mouse.up()
  await until(async () => JSON.stringify((await node(SAVE)).fills[0].to) !== JSON.stringify(fromTo))
  step('dragging the end handle moves the gradient', JSON.stringify((await node(SAVE)).fills[0].to) !== JSON.stringify(fromTo))
  step('the handle drag is one "Edit gradient" step', await until(async () => (await history())[0] === 'Edit gradient'), JSON.stringify((await history()).slice(0, 2)))
  await page.waitForTimeout(300)
  await undo()
  await until(async () => JSON.stringify((await node(SAVE)).fills[0].to) === JSON.stringify(fromTo))
  step('one undo takes the whole handle drag back', JSON.stringify((await node(SAVE)).fills[0].to) === JSON.stringify(fromTo))

  await kit.getByTestId('design-color-picker-bar').click({ position: { x: 100, y: 8 } })
  await until(async () => (await node(SAVE)).fills[0].stops?.length === 3)
  step('clicking the bar adds a stop', (await node(SAVE)).fills[0].stops.length === 3)
  await kit.getByTestId('design-color-picker-remove-stop').click()
  await until(async () => (await node(SAVE)).fills[0].stops?.length === 2)
  step('Remove stop takes it away', (await node(SAVE)).fills[0].stops.length === 2)

  await kit.getByTestId('design-color-picker-kind-solid').click()
  await until(async () => (await node(SAVE)).fills[0].type === 'solid')
  step('Solid turns it back into a colour', (await node(SAVE)).fills[0].type === 'solid')

  // The eyedropper samples the canvas (`samplePixel`).
  const sampled = (await node(SAVE)).fills[0].color
  await kit.getByTestId('design-color-picker-eyedropper').click()
  await until(async () => (await kit.getByTestId('design-eyedropper').count()) === 1)
  const canvas = await kit.getByTestId('kit-canvas-surface').boundingBox()
  await page.mouse.click(canvas.x + 20, canvas.y + 20)
  await until(async () => (await node(SAVE)).fills[0].color !== sampled)
  step('the eyedropper picks a colour off the canvas', (await node(SAVE)).fills[0].color !== sampled, `${sampled} → ${(await node(SAVE)).fills[0].color}`)

  // Choose image: the file becomes this fill in place, one step (image.place { index }).
  const fillsBefore = (await node(SAVE)).fills
  await kit.getByTestId('design-color-picker-file').setInputFiles({ name: 'dots.png', mimeType: 'image/png', buffer: PNG })
  await until(async () => (await node(SAVE)).fills[0].type === 'image')
  const imaged = (await node(SAVE)).fills
  step('a chosen image replaces the fill in place', imaged[0].type === 'image' && imaged.length === fillsBefore.length, `${fillsBefore.map((f) => f.type)} → ${imaged.map((f) => f.type)}`)
  step('the image settings show its colour profile (image.info)', await until(async () => ((await kit.locator('.design-cp-profile').textContent().catch(() => '')) ?? '').includes('Unknown — treated as sRGB')))
  await undo()
  step('one undo takes the image back', await until(async () => JSON.stringify((await node(SAVE)).fills) === JSON.stringify(fillsBefore)))

  await page.keyboard.press('Escape')
  await until(async () => (await picker.count()) === 0)
  step('Escape closes the picker', (await picker.count()) === 0)

  // Paste fill follows the engine's clipboard (clipboard.paintState).
  await traceEvents(frame())
  await kit.getByTestId('design-fill-0').click({ button: 'right' })
  await until(async () => (await kit.getByTestId('design-fill-0-menu').count()) === 1)
  const pasteBefore = await kit.getByTestId('design-fill-0-menu').getByText('Paste fill', { exact: true }).count()
  await kit.getByTestId('design-fill-0-menu').getByText('Copy fill', { exact: true }).click()
  await until(async () => (await designKit(() => window.designKit.invoke('query', 'clipboard.paintState', '{}').then((text) => JSON.parse(text).fill))) === true)
  await kit.getByTestId('design-fill-0').click({ button: 'right' })
  await until(async () => (await kit.getByTestId('design-fill-0-menu').count()) === 1)
  step('Copy fill offers Paste fill', pasteBefore === 0 && (await kit.getByTestId('design-fill-0-menu').getByText('Paste fill', { exact: true }).count()) === 1, `before ${pasteBefore}, clipboard ${await designKit(() => window.designKit.invoke('query', 'clipboard.paintState', '{}'))}, menu ${JSON.stringify(await kit.getByTestId('design-fill-0-menu').allTextContents())}, events ${await traced(frame())}, fill.copy run by the session: ${await designKit(async (id) => { try { await window.designKit.editor.session.run('fill.copy', { ids: [id], index: 0 }); return `ok, clipboard ${await window.designKit.invoke('query', 'clipboard.paintState', '{}')}` } catch (error) { return `error ${error?.message ?? error}` } }, SAVE)}`)
  await page.keyboard.press('Escape')

  // Add a fill: it goes on top and its picker opens; remove it again.
  const count = (await node(SAVE)).fills.length
  await kit.getByTestId('design-fill-add').click()
  await until(async () => (await node(SAVE)).fills.length === count + 1)
  step('Add fill adds a paint on top and opens its picker', (await node(SAVE)).fills.length === count + 1 && (await until(async () => (await picker.count()) === 1)))
  await kit.getByTestId(`design-fill-${count}-remove`).click()
  await until(async () => (await node(SAVE)).fills.length === count)
  step('Remove takes it away and closes its picker', (await node(SAVE)).fills.length === count && (await until(async () => (await picker.count()) === 0)))

  // A scrub in the picker's number fields that Escape cancels puts the paint back and leaves no
  // step (review #25d): a pattern fill's scale.
  if (await picker.count()) await page.keyboard.press('Escape')
  await until(async () => (await picker.count()) === 0)
  await designKit(([id, source]) => window.designKit.run('node.set', { ids: [id], props: { fills: [{ type: 'pattern', source, scale: 1, spacing: [0, 0] }] } }), [SAVE, TITLE])
  await until(async () => (await node(SAVE)).fills[0]?.type === 'pattern')
  await page.waitForTimeout(300)
  await kit.getByTestId('design-fill-0-swatch').click()
  const scaleLead = kit.getByTestId('design-color-picker').locator('[data-tip="Pattern scale"]')
  await until(async () => (await scaleLead.count()) === 1)
  const historyBefore = (await history()).length
  const lb = await scaleLead.boundingBox()
  await page.mouse.move(lb.x + lb.width / 2, lb.y + lb.height / 2)
  await page.mouse.down()
  for (let i = 1; i <= 6; i += 1) {
    await page.mouse.move(lb.x + lb.width / 2 + i * 4, lb.y + lb.height / 2)
    await page.waitForTimeout(30)
  }
  const scrubbedScale = await until(async () => (await node(SAVE)).fills[0].scale > 1, 30)
  await page.keyboard.press('Escape')
  await page.mouse.up()
  await until(async () => (await designKit(() => window.designKit.query('toolbar'))).interaction === null)
  await page.waitForTimeout(300)
  step('Escape cancels a scrub in the colour picker', scrubbedScale && (await node(SAVE)).fills[0].scale === 1 && (await history()).length === historyBefore, `${scrubbedScale} scale ${(await node(SAVE)).fills[0].scale}, steps ${historyBefore} → ${(await history()).length}`)
  if (await picker.count()) await page.keyboard.press('Escape')
  await until(async () => (await picker.count()) === 0)
  while ((await history()).length > historyBefore) await designKit(() => window.designKit.run('edit.undo'))
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await node(SAVE)).fills[0]?.type !== 'pattern')

  // ---- Stroke ----
  const weight = kit.getByTestId('design-stroke-weight').locator('input')
  await weight.click()
  await weight.fill('3')
  await weight.press('Enter')
  await until(async () => (await node(SAVE)).strokeWeight === 3)
  step('the stroke weight field sets strokeWeight', (await node(SAVE)).strokeWeight === 3)
  await kit.getByTestId('design-stroke-align').click()
  step('the align menu has no check marks (upstream menu_item rows)', !(await kit.getByTestId('design-stroke-align-menu').textContent()).includes('✓') && (await kit.getByTestId('design-stroke-align-menu').locator('.design-menu-check-space').count()) === 0)
  await kit.getByTestId('design-stroke-align-outside').click()
  await until(async () => (await node(SAVE)).strokeAlign === 'outside')
  step('the align dropdown sets strokeAlign', (await node(SAVE)).strokeAlign === 'outside')
  await kit.getByTestId('design-stroke-advanced').click()
  await kit.getByTestId('design-stroke-style').click()
  await kit.getByTestId('design-stroke-style-dash').click()
  await until(async () => ((await node(SAVE)).dashes ?? []).length === 2)
  step('Advanced stroke: Dash sets a dash pattern', ((await node(SAVE)).dashes ?? []).length === 2)
  await kit.getByTestId('design-stroke-sides').click()
  await kit.getByTestId('design-stroke-sides-top').click()
  await until(async () => Array.isArray((await node(SAVE)).strokeSides))
  step('Individual strokes ▸ Top sets the sides', JSON.stringify((await node(SAVE)).strokeSides) === '[3,0,0,0]', JSON.stringify((await node(SAVE)).strokeSides))
  // Brush rows draw their outline art (strokePreviewOutline).
  await kit.getByTestId('design-stroke-brush').click()
  step('the brush menu draws each brush as an outline', await until(async () => (await kit.getByTestId('design-stroke-brush-menu').locator('.design-brush-art path[d^="M"]').count()) === 8))
  await kit.getByTestId('design-stroke-brush-ink').click()
  await until(async () => (await node(SAVE)).brush?.preset === 'ink')
  step('picking a brush sets it and draws it in the button', (await node(SAVE)).brush?.preset === 'ink' && (await until(async () => (await kit.getByTestId('design-stroke-brush').locator('.design-brush-art').count()) === 1)))
  await undo()
  await kit.getByTestId('design-stroke-advanced').click()

  // A scrub of a mixed stroke weight moves each layer's weight by the drag's delta, from where it
  // started (review #8).
  // A frame given SAVE's stroke at another weight.
  const OTHER = 68
  await designKit((id) => window.designKit.run('node.set', { ids: [id], props: { strokes: [{ type: 'solid', color: '#FFFFFF', opacity: 0.35 }], strokeWeight: 7 } }), OTHER)
  await select([SAVE, OTHER])
  const weights = async () => [(await node(SAVE)).strokeWeight, (await node(OTHER)).strokeWeight]
  const weightField = kit.getByTestId('design-stroke-weight')
  step('a mixed stroke weight shows Mixed', await until(async () => (await weightField.count()) === 1 && (await weightField.getAttribute('data-mixed')) === 'true'), JSON.stringify(await weights()))
  await page.waitForTimeout(250)
  const [wa, wb] = await weights()
  const weightLead = await weightField.locator('.design-field-lead').boundingBox()
  await page.mouse.move(weightLead.x + weightLead.width / 2, weightLead.y + weightLead.height / 2)
  await page.mouse.down()
  for (let i = 1; i <= 8; i += 1) {
    await page.mouse.move(weightLead.x + weightLead.width / 2 + i * 3, weightLead.y + weightLead.height / 2)
    await page.waitForTimeout(30)
  }
  await page.waitForTimeout(300)
  const weightDelta = Number(await weightField.locator('input').inputValue())
  await page.mouse.up()
  await page.waitForTimeout(400)
  const [va, vb] = await weights()
  step('a scrub moves each stroke weight of a mixed selection by the drag\'s delta', weightDelta > 0 && va - wa === weightDelta && vb - wb === weightDelta, `${wa},${wb} → ${va},${vb} (delta ${weightDelta})`)
  await undo()
  await undo()

  // ---- Effects ----
  const effects = (await node(SAVE)).effects.length
  await kit.getByTestId('design-effect-add').click()
  await until(async () => (await node(SAVE)).effects.length === effects + 1)
  step('Add effect adds a drop shadow', (await node(SAVE)).effects[effects]?.type === 'dropShadow')
  await kit.getByTestId(`design-effect-${effects}-settings`).click()
  const blur = kit.getByTestId(`design-effect-${effects}-radius`).locator('input')
  await blur.click()
  await blur.fill('12')
  await blur.press('Enter')
  await until(async () => (await node(SAVE)).effects[effects].radius === 12)
  step('the shadow settings set its blur', (await node(SAVE)).effects[effects].radius === 12)
  const lead = await kit.getByTestId(`design-effect-${effects}-offsetX`).locator('.design-field-lead').boundingBox()
  const x0 = (await node(SAVE)).effects[effects].offset[0]
  await page.mouse.move(lead.x + lead.width / 2, lead.y + lead.height / 2)
  await page.mouse.down()
  for (let i = 1; i <= 6; i += 1) await page.mouse.move(lead.x + lead.width / 2 + i * 4, lead.y + lead.height / 2)
  await page.mouse.up()
  await until(async () => (await node(SAVE)).effects[effects].offset[0] !== x0)
  step('scrubbing X moves the shadow', (await node(SAVE)).effects[effects].offset[0] !== x0)
  await page.waitForTimeout(300)
  await undo()
  await until(async () => (await node(SAVE)).effects[effects].offset[0] === x0)
  step('one undo takes the scrub back', (await node(SAVE)).effects[effects].offset[0] === x0, `${x0} → ${(await node(SAVE)).effects[effects].offset[0]} (${(await inspect()).undo})`)
  await kit.getByTestId(`design-effect-${effects}-type`).click()
  await kit.getByTestId(`design-effect-${effects}-type-layerBlur`).click()
  await until(async () => (await node(SAVE)).effects[effects].type === 'layerBlur')
  step('the type dropdown turns it into a layer blur', (await node(SAVE)).effects[effects].type === 'layerBlur')
  await kit.getByTestId(`design-effect-${effects}-remove`).click()
  await until(async () => (await node(SAVE)).effects.length === effects)
  step('Remove takes the effect away', (await node(SAVE)).effects.length === effects)

  // The background blur's progressive ramp shows canvas handles.
  await kit.getByTestId('design-effect-0-settings').click()
  await kit.getByTestId('design-effect-0-blur-type').click()
  await kit.getByTestId('design-effect-0-blur-type-progressive').click()
  await until(async () => (await node(SAVE)).effects[0].blurType === 'progressive')
  step('Progressive blur shows its canvas handles', await until(async () => (await kit.getByTestId('design-blur-handles').count()) === 1))

  // ---- Stroke caps, read back from the layer (startCap, endCap) ----
  await select([LINE])
  await designKit((id) => window.designKit.run('node.set', { ids: [id], props: { startCap: 'round', endCap: 'triangleArrow' } }), LINE)
  await until(async () => (await kit.getByTestId('design-stroke-advanced').count()) === 1)
  await kit.getByTestId('design-stroke-advanced').click()
  step('the end point shows the layer\'s cap', await until(async () => (await kit.getByTestId('design-stroke-endCap').textContent()) === 'Triangle arrow'), await kit.getByTestId('design-stroke-endCap').textContent().catch(() => ''))
  step('and the start point its own', (await kit.getByTestId('design-stroke-startCap').textContent()) === 'Round')
  await kit.getByTestId('design-stroke-advanced').click()
  await undo()

  // ---- Typography ----
  await select([TITLE])
  const section = kit.getByTestId('design-typography-section')
  await until(async () => (await section.count()) === 1)
  step('Typography shows the font', (await kit.getByTestId('design-font-family').textContent()) === 'Inter' && (await kit.getByTestId('design-font-style').textContent()) === 'SemiBold')
  const size = kit.getByTestId('design-font-size').locator('input')
  await size.click()
  await size.fill('24')
  await size.press('Enter')
  await until(async () => (await node(TITLE)).fontSize === 24)
  step('the size field sets fontSize', (await node(TITLE)).fontSize === 24)
  await kit.getByTestId('design-font-size-menu-button').click()
  await kit.getByTestId('design-font-size-32').click()
  await until(async () => (await node(TITLE)).fontSize === 32)
  step('the size menu sets fontSize', (await node(TITLE)).fontSize === 32)
  await kit.getByTestId('design-font-style').click()
  await kit.getByTestId('design-font-style-Bold').click()
  await until(async () => (await node(TITLE)).fontStyle === 'Bold')
  step('the style menu sets fontStyle', (await node(TITLE)).fontStyle === 'Bold')
  await kit.getByTestId('design-line-height').locator('button').click()
  await kit.getByTestId('design-line-height').locator('input').fill('150%')
  await kit.getByTestId('design-line-height').locator('input').press('Enter')
  await until(async () => (await node(TITLE)).lineHeight === '150%')
  step('the line height field takes 150%', (await node(TITLE)).lineHeight === '150%', String((await node(TITLE)).lineHeight))
  await kit.getByTestId('design-text-align-center').click()
  await until(async () => (await node(TITLE)).textAlign === 'center')
  step('alignment sets textAlign', (await node(TITLE)).textAlign === 'center')
  // Paragraph spacing and vertical alignment are read back from the layer.
  await designKit((id) => window.designKit.run('node.set', { ids: [id], props: { paragraphSpacing: 14, textAlignVertical: 'bottom' } }), TITLE)
  const spacingValue = () => designKit(() => (document.querySelector('[data-testid="design-paragraph-spacing"] input')?.value ?? document.querySelector('[data-testid="design-paragraph-spacing"]')?.textContent ?? 'none').trim())
  step('paragraph spacing shows the layer\'s value', await until(async () => (await spacingValue()) === '14'), await spacingValue())
  step('vertical alignment shows the layer\'s value', await until(async () => (await kit.getByTestId('design-text-align-v-bottom').getAttribute('aria-pressed')) === 'true' || (await kit.getByTestId('design-text-align-v-bottom').getAttribute('aria-checked')) === 'true'))
  await kit.getByTestId('design-text-align-v-middle').click()
  step('the middle button sets it', await until(async () => ['middle', 'center'].includes((await node(TITLE)).textAlignVertical)), String((await node(TITLE)).textAlignVertical))

  await kit.getByTestId('design-font-family').click()
  const fonts = kit.getByTestId('design-font-picker')
  await until(async () => (await fonts.count()) === 1)
  step('the family picker lists the fonts', await until(async () => (await kit.getByTestId('design-font-Inter').count()) === 1))
  step('families are drawn in their own font (textPreview)', await until(async () => (await kit.getByTestId('design-font-Inter').locator('img').count()) === 1))
  await kit.getByTestId('design-font-search').fill('droid')
  await until(async () => (await kit.getByTestId('design-font-Inter').count()) === 0)
  await kit.getByTestId('design-font-search').press('Enter')
  await until(async () => (await node(TITLE)).fontFamily === 'Droid Sans Fallback')
  step('Enter picks the first match', (await node(TITLE)).fontFamily === 'Droid Sans Fallback', String((await node(TITLE)).fontFamily))

  await kit.getByTestId('design-type-settings-button').click()
  await until(async () => (await kit.getByTestId('design-type-settings').count()) === 1)
  await kit.getByTestId('design-ts-decoration-underline').click()
  await until(async () => (await node(TITLE)).textDecoration === 'underline')
  step('Type settings: Underline sets textDecoration', (await node(TITLE)).textDecoration === 'underline')
  await kit.getByTestId('design-ts-case-upper').click()
  await until(async () => (await designKit((id) => window.designKit.query('selection').then((s) => s.text?.runs?.[0]?.style?.case), TITLE)) === 'upper')
  step('Type settings: Uppercase sets the case', true)
  await kit.getByTestId('design-ts-tab-details').click()
  step('Details lists OpenType features', await until(async () => (await kit.getByTestId('design-ts-feature-tnum').count()) === 1))
  const tnum = kit.getByTestId('design-ts-feature-tnum')
  const tnumOn = (await tnum.getAttribute('aria-checked')) === 'true'
  await traceEvents(frame())
  await tnum.click()
  await until(async () => (await node(TITLE)).features?.tnum === !tnumOn)
  step('a feature toggle sets it on the layer (features)', (await node(TITLE)).features?.tnum === !tnumOn, `${JSON.stringify((await node(TITLE)).features)} was ${tnumOn}, shows ${await tnum.getAttribute('aria-checked')}, font ${(await node(TITLE)).fontFamily}, selection ${JSON.stringify((await inspect()).selection)}, events ${await traced(frame())}`)
  // Read back after the selection moves away and returns.
  await select([SAVE])
  await until(async () => (await kit.getByTestId('design-type-settings').count()) === 0)
  await select([TITLE])
  await until(async () => (await kit.getByTestId('design-type-settings-button').count()) === 1)
  await kit.getByTestId('design-type-settings-button').click()
  await kit.getByTestId('design-ts-tab-details').click()
  step('and Details shows it again from the layer', await until(async () => (await kit.getByTestId('design-ts-feature-tnum').getAttribute('aria-checked')) === String(!tnumOn)))
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
