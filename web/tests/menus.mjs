// The menus package in a real browser (2.7, 2.5 §17-18): node tests/menus.mjs [chromium|webkit]
// The main menu and the top bar agree, checks follow the engine, the Actions palette, the keymap
// (no key runs twice), the dialogs, the context menus, comments, and View > Show/Hide UI (Focus). Starts its own Vite server
// unless DESIGN_ORIGIN names a running one. Prints PASS or FAIL per step.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
let server = null
let origin = process.env.DESIGN_ORIGIN
if (!origin) {
  server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
  await server.listen()
  origin = `http://127.0.0.1:${server.httpServer.address().port}`
}
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
const page = await context.newPage()
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
if (process.env.SHOTS) page.on('console', (m) => m.type() === 'error' && console.log('CONSOLE', m.text().slice(0, 300)))
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

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  const designKit = (body, arg) => frame().evaluate(body, arg)
  const inspect = () => designKit(() => window.designKit.inspect)
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true, 120)
  await page.waitForTimeout(800)
  const canvas = await kit.getByTestId('kit-canvas-surface').boundingBox()
  const toPage = async (x, y) => {
    const view = await designKit(() => window.designKit.canvas.current.view())
    return { x: canvas.x + (x - view.x) * view.zoom, y: canvas.y + (y - view.y) * view.zoom }
  }
  // The editor's Cmd is the platform's (navigator.platform): Meta on a Mac, Control elsewhere.
  const mod = (await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control'
  const labels = (locator) => locator.locator('[role^="menuitem"]').evaluateAll((rows) => rows.map((row) => (row.querySelector('.design-menu-label, [class*="label"]')?.textContent ?? row.textContent ?? '').trim()))
  const checkedIds = () => designKit(async () => (await window.designKit.query('checked')).ids)
  const focusCanvas = async () => {
    await page.mouse.click(canvas.x + canvas.width - 40, canvas.y + 40)
    await designKit(() => window.designKit.run('select.set', { ids: [] }))
  }

  // 1. The main menu: Actions…, the top menus, Edit ▸ Copy as and Pick color's I.
  await kit.getByTestId('design-main-menu-button').click()
  await until(async () => (await kit.getByTestId('design-main-menu').count()) === 1)
  const top = (await labels(kit.getByTestId('design-main-menu'))).slice(1)
  step('the main menu lists the top menus without Plugins', top.join(',') === 'File,Edit,View,Object,Text,Arrange,Vector,Preferences,Libraries,Help', top.join(','))
  await kit.getByTestId('design-main-menu-edit').hover()
  await until(async () => (await kit.getByTestId('design-main-menu-edit-menu').count()) === 1)
  const edit = await labels(kit.getByTestId('design-main-menu-edit-menu'))
  step('Edit holds Copy as ▸ after Redo, and Pick color', edit[2] === 'Copy as' && edit.includes('Pick color'), edit.slice(0, 4).join(','))
  const pick = await kit.getByTestId('design-menu-edit.pickColor').locator('.design-menu-keys').textContent()
  step('Pick color shows its key I', pick?.trim() === 'I', pick ?? '')
  await kit.getByTestId('design-main-menu-view').hover()
  await until(async () => (await kit.getByTestId('design-main-menu-view-menu').count()) === 1)
  const shownChecks = await kit.getByTestId('design-main-menu-view-menu').locator('[role="menuitemcheckbox"][aria-checked="true"]').count()
  const engineChecks = (await checkedIds()).filter((id) => ['view.pixelGrid', 'view.layoutGrids', 'view.rulers', 'view.slices', 'view.comments', 'view.pixelPreview', 'view.maskOutlines', 'view.frameOutlines', 'view.proofSrgb'].includes(id)).length
  step('View checks follow the engine', shownChecks === engineChecks, `${shownChecks} vs ${engineChecks}`)
  const rulersBefore = (await checkedIds()).includes('view.rulers')
  await kit.getByTestId('design-menu-view.rulers').click()
  await until(async () => (await checkedIds()).includes('view.rulers') !== rulersBefore)
  step('View ▸ Rulers toggles the engine', (await checkedIds()).includes('view.rulers') !== rulersBefore)
  step('the menu closes after running an item', (await kit.getByTestId('design-main-menu').count()) === 0)

  // 2. The top bar's Edit menu lists what the main menu's does.
  await kit.getByTestId('design-menu-Edit').click()
  await page.waitForTimeout(400)
  const bar = await kit.locator('[role="menu"]').last().locator('[role^="menuitem"]').evaluateAll((rows) => rows.map((row) => row.textContent.trim()))
  const same = edit.every((label) => bar.some((row) => row.startsWith(label)))
  step('the top bar Edit menu agrees with the main menu', same && bar.length >= edit.length, `${bar.length} rows`)
  await page.keyboard.press('Escape')
  await page.waitForTimeout(200)

  // Libraries ▸ Publish library… (upstream's place, between Open libraries and Hide when publishing) opens the libraries view.
  await kit.getByTestId('design-main-menu-button').click()
  await until(async () => (await kit.getByTestId('design-main-menu').count()) === 1)
  await kit.getByTestId('design-main-menu-libraries').hover()
  await until(async () => (await kit.getByTestId('design-main-menu-libraries-menu').count()) === 1)
  const libs = await labels(kit.getByTestId('design-main-menu-libraries-menu'))
  step('Libraries lists Publish library… between Open libraries and Hide when publishing', libs.join(',') === 'Open libraries,Publish library…,Hide when publishing', libs.join(','))
  await kit.getByTestId('design-menu-libraries.publish').click()
  step('Publish library… opens the libraries view with its publish form', await until(async () => (await kit.getByTestId('design-libraries-publish').count()) === 1))
  await page.keyboard.press('Escape')
  await page.waitForTimeout(200)

  // 3. The Actions palette: Cmd+K, search, Enter runs, Recents.
  await focusCanvas()
  await page.keyboard.press(`${mod}+k`)
  await until(async () => (await kit.getByTestId('design-actions-palette').count()) === 1)
  step('Cmd+K opens the Actions palette', (await kit.getByTestId('design-actions-palette').count()) === 1)
  {
    // Upstream: as wide as the toolbar pill plus its 1 px stroke a side, 8 px above it; tabs 24 tall with 8 px sides.
    const pal = await kit.getByTestId('design-actions-palette').boundingBox()
    const pill = await kit.getByTestId('design-toolbar').boundingBox()
    step('the palette sits over the pill, its width and 1 px stroke', Math.abs(pal.x - pill.x) < 0.5 && Math.abs(pal.width - (pill.width + 2)) < 0.5 && Math.abs(pill.y - (pal.y + pal.height) - 7) < 1, `${pal.x},${pal.width},${pal.y + pal.height} vs ${pill.x},${pill.width},${pill.y}`)
    const tab = await kit.locator('.design-palette-tab').first().boundingBox()
    step('the palette tabs are 24 px tall', Math.abs(tab.height - 24) < 0.5, String(tab.height))
  }
  await page.keyboard.type('rulers')
  await until(async () => (await kit.getByTestId('design-palette-row-command:view.rulers').count()) === 1)
  const before = (await checkedIds()).includes('view.rulers')
  await page.keyboard.press('Enter')
  await until(async () => (await checkedIds()).includes('view.rulers') !== before)
  step('Enter runs the highlighted action', (await checkedIds()).includes('view.rulers') !== before && (await kit.getByTestId('design-actions-palette').count()) === 0)
  await page.keyboard.press(`${mod}+k`)
  await until(async () => (await kit.getByTestId('design-actions-palette').count()) === 1)
  const first = await kit.getByTestId('design-palette-list').locator('.design-palette-section').first().textContent()
  step('Recents lists it first', first === 'Recents' && (await kit.getByTestId('design-palette-row-command:view.rulers').count()) >= 1, first ?? '')
  await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('design-actions-palette').count()) === 0)

  // 4. Keyboard shortcuts: Ctrl+Shift+?, search, Escape.
  await focusCanvas()
  await page.keyboard.press('Control+Shift+Slash')
  await until(async () => (await kit.getByTestId('design-dialog-shortcuts').count()) === 1)
  step('Ctrl+Shift+? opens Keyboard shortcuts', (await kit.getByTestId('design-dialog-shortcuts').count()) === 1)
  await kit.getByTestId('design-shortcuts-search').fill('flatten')
  await until(async () => (await kit.getByTestId('design-shortcut-row').count()) > 0 && (await kit.getByTestId('design-shortcut-row').count()) < 10)
  const found = await kit.getByTestId('design-shortcut-row').evaluateAll((rows) => rows.map((row) => row.textContent))
  step('the search finds Flatten with both keys', found.some((row) => row.includes('Flatten') && row.includes('or')), found.join(' | '))
  await kit.getByTestId('design-dialog-shortcuts-close').click()

  // 5. Find and replace through the frame's Cmd+F (one dialog, no parameters form).
  await focusCanvas()
  await page.keyboard.press(`${mod}+f`)
  await until(async () => (await kit.getByTestId('design-dialog-find').count()) === 1)
  step('Cmd+F opens Find and replace', (await kit.getByTestId('design-dialog-find').count()) === 1 && (await kit.locator('.kui-dialog').count()) === 0)
  await kit.getByTestId('design-find-input').fill('Light')
  await kit.getByTestId('design-find-find').click()
  await until(async () => (await inspect()).selection.length > 0)
  step('Find selects the matches', (await inspect()).selection.length > 0, `${(await inspect()).selection.length} selected`)
  {
    // Upstream's plain egui window: 338 wide, a 40 px title band whose triangle folds it to the title.
    const box = await kit.getByTestId('design-dialog-find').boundingBox()
    step('Find and replace is the plain window, 338 wide', Math.abs(box.width - 338) < 1, String(box.width))
    await kit.getByTestId('design-dialog-find-fold').click()
    await until(async () => (await kit.getByTestId('design-find-input').count()) === 0)
    step('its triangle folds it to the title band', (await kit.getByTestId('design-find-input').count()) === 0 && Math.abs((await kit.getByTestId('design-dialog-find').boundingBox()).height - 42) < 2)
    await kit.getByTestId('design-dialog-find-fold').click()
    await until(async () => (await kit.getByTestId('design-find-input').count()) === 1)
    step('and unfolds it', (await kit.getByTestId('design-find-input').count()) === 1)
  }
  await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('design-dialog-find').count()) === 0)
  step('Escape closes the dialog', (await kit.getByTestId('design-dialog-find').count()) === 0)

  // 6. Rename layers: two layers, F2 (the keymap's second key), a pattern, one undo step.
  const doc = await designKit(() => window.designKit.query('document', { depth: 1 }))
  const frames = doc.pages[0].children.filter((child) => child.type === 'FRAME').slice(0, 2)
  await designKit((ids) => window.designKit.run('select.set', { ids }), frames.map((f) => f.id))
  await page.mouse.move(canvas.x + canvas.width - 40, canvas.y + 40)
  await frame().evaluate(() => document.querySelector('[data-testid="kit-canvas-surface"]')?.focus())
  await page.keyboard.press('F2')
  await until(async () => (await kit.getByTestId('design-dialog-rename').count()) === 1)
  step('F2 with two layers opens Rename layers', (await kit.getByTestId('design-dialog-rename').count()) === 1)
  step('and no inline rename starts in Layers', (await kit.getByTestId('design-layer-rename').count()) === 0 && JSON.stringify((await inspect()).selection) === JSON.stringify(frames.map((f) => f.id)))
  await kit.getByTestId('design-rename-to').fill('Screen $n')
  if (process.env.SHOTS) {
    await page.waitForTimeout(500)
    await page.screenshot({ path: '/tmp/wp6/test-rename.png' })
    console.log('DEBUG', await kit.getByTestId('design-rename-to').inputValue(), await kit.locator('.design-dialog').count())
  }
  await until(async () => (await kit.getByTestId('design-rename-preview').textContent()).includes('Screen 2'))
  const preview = await kit.getByTestId('design-rename-preview').textContent()
  step('the preview numbers them', preview.includes('Screen 1') && preview.includes('Screen 2'), preview)
  await kit.getByTestId('design-rename-apply').click()
  const names = async () => (await designKit(() => window.designKit.query('document', { depth: 1 }))).pages[0].children.filter((c) => c.type === 'FRAME').map((c) => c.name)
  await until(async () => (await names()).filter((name) => /^Screen \d$/.test(name)).length === 2)
  step('Rename renames both', (await names()).filter((name) => /^Screen \d$/.test(name)).length === 2, (await names()).join(','))
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await names()).filter((name) => /^Screen \d$/.test(name)).length === 0)
  step('one undo takes it back', (await names()).filter((name) => /^Screen \d$/.test(name)).length === 0)

  // 7. No key runs twice: Cmd+G groups, Cmd+Backspace ungroups (the engine's Delete must not run).
  await page.mouse.click(canvas.x + canvas.width - 40, canvas.y + 40)
  await designKit((ids) => window.designKit.run('select.set', { ids }), frames.map((f) => f.id))
  const layers = (await inspect()).layers
  await page.keyboard.press(`${mod}+g`)
  await until(async () => (await inspect()).layers === layers + 1)
  step('Cmd+G groups (the frame binding)', (await inspect()).layers === layers + 1)
  await page.keyboard.press(`${mod}+Backspace`)
  await until(async () => (await inspect()).layers === layers)
  await page.waitForTimeout(300)
  step('Cmd+Backspace ungroups and deletes nothing', (await inspect()).layers === layers, `${(await inspect()).layers} vs ${layers}`)

  // 8. The canvas context menu: right-click a frame selects it; Select layer ▸; an item runs.
  await designKit(() => window.designKit.run('select.set', { ids: [] }))
  const target = frames[0]
  const area = (await inspect()).area
  const point = await toPage(target.absoluteBounds.x - area.x + target.absoluteBounds.width / 2, target.absoluteBounds.y - area.y + 20)
  await page.mouse.click(point.x, point.y, { button: 'right' })
  await until(async () => (await kit.getByTestId('design-context-menu').count()) === 1)
  step('a right-click opens the context menu', (await kit.getByTestId('design-context-menu').count()) === 1)
  await until(async () => (await inspect()).selection.length > 0)
  step('and selects what it lands on', (await inspect()).selection.length > 0)
  await kit.getByTestId('design-context-select-layer').hover()
  await until(async () => (await kit.getByTestId('design-context-select-layer-menu').count()) === 1)
  const under = await kit.getByTestId('design-context-select-layer-menu').locator('[role^="menuitem"]').count()
  step('Select layer ▸ lists the layers under the point', under > 0, `${under}`)
  // Upstream's availability: every offered command is enabled, even Detach on a frame.
  const greyed = async (id) => (await kit.getByTestId(`design-context-${id}`).getAttribute('aria-disabled')) === 'true'
  step('the menu greys nothing that is offered (Detach on a frame, as upstream)', !(await greyed('component.detach')) && !(await greyed('edit.copy')) && !(await greyed('edit.paste')))
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await page.waitForTimeout(200)
  await designKit(() => window.designKit.run('select.set', { ids: [] }))
  const empty = { x: canvas.x + canvas.width - 30, y: canvas.y + 30 }
  await page.mouse.click(empty.x, empty.y, { button: 'right' })
  await until(async () => (await kit.getByTestId('design-context-menu').count()) === 1)
  const plain = await labels(kit.getByTestId('design-context-menu'))
  step('without a selection: Paste here, Select all and the view items', plain.join(',') === 'Paste here,Select all,Show/Hide UI,Zoom to fit,Rulers,Pixel grid', plain.join(','))
  await kit.getByTestId('design-context-select.all').click()
  await until(async () => (await inspect()).selection.length > 1)
  step('Select all runs from it', (await inspect()).selection.length > 1)

  // A right-clicked ruler guide adds Remove guide on top.
  await designKit(() => window.designKit.run('select.set', { ids: [] }))
  if (!(await checkedIds()).includes('view.rulers')) await designKit(() => window.designKit.editor.run('view.rulers'))
  const guides = async () => designKit(async () => JSON.parse(await window.designKit.invoke('chrome', window.designKit.canvas.current.view().zoom)).guides)
  // A guide of a top-level frame (the chrome draws frame guides; page guides span no frame).
  const guidesBefore = (await guides()).length
  await designKit((id) => window.designKit.run('guide.add', { vertical: true, offset: 100, frame: id }), target.id)
  await designKit(() => window.designKit.refresh())
  await until(async () => (await guides()).length === guidesBefore + 1)
  const shown = (await guides()).find((guide) => guide.owner === target.id && guide.vertical && Math.abs(guide.offset - 100) < 0.5)
  const at = await toPage((shown.a[0] + shown.b[0]) / 2, (shown.a[1] + shown.b[1]) / 2)
  await page.mouse.click(at.x, at.y, { button: 'right' })
  await until(async () => (await kit.getByTestId('design-context-guide.remove').count()) === 1)
  step('a right-clicked guide offers Remove guide', (await kit.getByTestId('design-context-guide.remove').count()) === 1)
  await kit.getByTestId('design-context-guide.remove').click()
  await until(async () => (await guides()).length === guidesBefore)
  step('Remove guide removes it', (await guides()).length === guidesBefore)

  // 9. Preferences and Color profile (an engine UI request).
  await designKit(() => window.designKit.run('select.set', { ids: [] }))
  await kit.getByTestId('design-main-menu-button').click({ timeout: 5000 })
  await kit.getByTestId('design-main-menu-preferences').hover({ timeout: 5000 })
  if (process.env.SHOTS) {
    await page.screenshot({ path: '/tmp/wp6/test-prefs.png' })
    console.log('DEBUG', await designKit(async () => { try { return (await window.designKit.query('menus')).length } catch (e) { return String(e) } }), await designKit(() => window.designKit.editor.session.failure))
  }
  await until(async () => (await kit.getByTestId('design-menu-app.preferences').count()) === 1)
  await kit.getByTestId('design-menu-app.preferences').click({ timeout: 5000 })
  await until(async () => (await kit.getByTestId('design-dialog-preferences').count()) === 1)
  const keep = async () => (await designKit(() => window.designKit.query('prefs'))).keepTool
  const was = await keep()
  await kit.getByTestId('design-pref-keepTool').click()
  await until(async () => (await keep()) !== was)
  step('Preferences toggles Keep tool selected', (await keep()) !== was)
  await kit.getByTestId('design-pref-keepTool').click()
  await kit.getByTestId('design-dialog-preferences-close').click()
  await designKit(() => window.designKit.editor.run('file.colorProfile'))
  await until(async () => (await kit.getByTestId('design-dialog-color-profile').count()) === 1)
  step('File ▸ Color profile… opens its dialog from the engine', (await kit.getByTestId('design-dialog-color-profile').count()) === 1)
  await kit.getByTestId('design-profile-displayP3').click()
  step('another profile offers Keep appearance', (await kit.getByTestId('design-profile-convert').count()) === 1)
  await kit.getByTestId('design-dialog-color-profile-close').click()

  // File ▸ Save to version history opens the version panel's save form; its Save saves a version.
  await designKit(() => window.designKit.editor.run('file.saveVersion'))
  await until(async () => (await kit.getByTestId('design-version-save-form').count()) === 1)
  step('Save to version history opens the save form', (await kit.getByTestId('design-version-save-form').count()) === 1)
  const versionCount = async () => (await designKit(() => window.designKit.query('versions'))).length
  const versionsBefore = await versionCount()
  await kit.getByTestId('design-version-save-form-submit').click()
  await until(async () => (await versionCount()) === versionsBefore + 1)
  step('its Save saves a version', (await versionCount()) === versionsBefore + 1 && (await kit.getByTestId('design-version-save-form').count()) === 0)
  await kit.getByTestId('design-version-close').click()
  await until(async () => (await kit.getByTestId('design-version-history').count()) === 0)

  // View ▸ Panels ▸ Reset panel widths puts a dragged panel back to its width.
  const leftWidth = () => kit.locator('.design-left').evaluate((el) => Math.round(el.getBoundingClientRect().width))
  const edge = await kit.getByTestId('design-edge-left').boundingBox()
  await page.mouse.move(edge.x + edge.width / 2, edge.y + edge.height / 2)
  await page.mouse.down()
  await page.mouse.move(edge.x + edge.width / 2 + 60, edge.y + edge.height / 2, { steps: 6 })
  await page.mouse.up()
  await until(async () => (await leftWidth()) !== 240)
  const dragged = await leftWidth()
  await designKit(() => window.designKit.editor.run('view.panelWidths'))
  await until(async () => (await leftWidth()) === 240)
  step('Reset panel widths puts the left panel back', dragged > 240 && (await leftWidth()) === 240, `${dragged} → ${await leftWidth()}`)

  // The help button: Keyboard shortcuts and About; Getting started (no home screen) and Help page
  // (the dev host serves no host/openUrl) are left out.
  await kit.getByTestId('design-help').click()
  const help = await kit.locator('.design-menu').last().locator('[role^="menuitem"]').evaluateAll((rows) => rows.map((row) => row.textContent.trim()))
  step('the help menu lists only what works here', help.length === 2 && help[0].startsWith('Keyboard shortcuts') && help[1].startsWith('About'), help.join(','))
  await page.keyboard.press('Escape')

  // 10. Comments: add one, its pin shows with View ▸ Comments, the thread opens and resolves.
  const added = await designKit(async () => {
    const r = await window.designKit.run('comment.add', { x: 100, y: 100, text: 'Check the spacing' })
    await window.designKit.refresh()
    return r
  })
  const pinId = added?.id
  await until(async () => (await kit.locator('[data-testid^="design-comment-pin-"]').count()) > 0)
  step('a comment shows its pin', (await kit.locator('[data-testid^="design-comment-pin-"]').count()) > 0, JSON.stringify(added))
  await kit.locator('[data-testid^="design-comment-pin-"]').first().click()
  await until(async () => (await kit.getByTestId('design-comment-thread').count()) === 1)
  step('a click opens its thread', (await kit.getByTestId('design-comment-thread').count()) === 1)
  await kit.getByTestId('design-comment-composer').fill('Done')
  await page.keyboard.press('Enter')
  const thread = async () => (await designKit(async () => (await window.designKit.query('comments')).comments))?.find((c) => c.id === pinId)
  await until(async () => ((await thread())?.replies?.length ?? 0) === 1)
  step('Enter posts a reply', ((await thread())?.replies?.length ?? 0) === 1)
  await kit.getByTestId('design-comment-resolve').click()
  await until(async () => (await thread())?.resolved === true)
  step('Resolve resolves the thread', (await thread())?.resolved === true)
  await kit.getByTestId('design-comment-close').click()
  // A pin drag the browser cancels moves nothing and opens nothing (review #25d).
  const pin = kit.getByTestId(`design-comment-pin-${pinId}`)
  const pinAt = { x: (await thread()).x, y: (await thread()).y }
  const pinBox = await pin.boundingBox()
  await page.mouse.move(pinBox.x + pinBox.width / 2, pinBox.y + pinBox.height / 2)
  await page.mouse.down()
  await page.mouse.move(pinBox.x + pinBox.width / 2 + 40, pinBox.y + pinBox.height / 2 + 30, { steps: 6 })
  await pin.dispatchEvent('pointercancel', { pointerId: 1, isPrimary: true })
  await page.mouse.up()
  await page.waitForTimeout(400)
  const pinNow = await thread()
  step("a cancelled pin drag moves nothing and opens nothing", typeof pinAt.x === "number" && pinNow.x === pinAt.x && pinNow.y === pinAt.y && (await kit.getByTestId('design-comment-thread').count()) === 0, `${JSON.stringify(pinAt)} → ${pinNow.x},${pinNow.y}`)

  // The Comment tool: the Comments panel lists threads; a press on the canvas starts a draft.
  await designKit(() => window.designKit.tools.select('comment'))
  await until(async () => (await kit.getByTestId('design-comments-panel').count()) === 1)
  step('the Comment tool shows the Comments panel', (await kit.getByTestId('design-comments-panel').count()) === 1)
  await kit.getByTestId('design-comments-filter').click()
  await kit.locator('.design-menu [role^="menuitem"]').filter({ hasText: 'All comments' }).click()
  await until(async () => (await kit.getByTestId(`design-comments-row-${pinId}`).count()) === 1)
  step('All comments lists the resolved thread', (await kit.getByTestId(`design-comments-row-${pinId}`).count()) === 1)
  await kit.getByTestId(`design-comments-row-${pinId}`).click()
  await until(async () => (await kit.getByTestId('design-comment-thread').count()) === 1)
  step('a row opens its thread', (await kit.getByTestId('design-comment-thread').count()) === 1)
  await page.keyboard.press('Escape')
  const spot = { x: canvas.x + canvas.width / 2, y: canvas.y + canvas.height / 2 }
  await page.mouse.click(spot.x, spot.y)
  await until(async () => (await kit.getByTestId('design-comment-draft').count()) === 1)
  step('a press with the Comment tool starts a draft', (await kit.getByTestId('design-comment-draft').count()) === 1)
  const count = async () => (await designKit(async () => (await window.designKit.query('comments', { page: 'all' })).comments.length))
  const had = await count()
  await kit.getByTestId('design-comment-draft-text').fill('A new note')
  await page.keyboard.press('Enter')
  await until(async () => (await count()) === had + 1)
  step('Enter posts the draft as a thread', (await count()) === had + 1)
  await designKit(() => window.designKit.tools.select('move'))

  // View > Panels > Show/Hide UI is kit-editor's Focus display (designKit.platform.display-modes/show-hide-ui);
  // with nothing selected, Escape leaves it.
  await focusCanvas()
  const frameApp = kit.locator('.kui-app')
  const modeBefore = await frameApp.getAttribute('data-mode')
  await kit.getByTestId('design-menu-View').click()
  await kit.locator('[role^="menuitem"]').filter({ hasText: /^Panels/ }).hover()
  await kit.getByTestId('design-control-view.toggleUi').click()
  step('View > Show/Hide UI enters Focus', await until(async () => (await frameApp.getAttribute('data-mode')) === 'focus'))
  await page.keyboard.press('Escape')
  step('Escape leaves the Focus Show/Hide UI entered', await until(async () => (await frameApp.getAttribute('data-mode')) === modeBefore), `${modeBefore} -> ${await frameApp.getAttribute('data-mode')}`)
} catch (error) {
  failures += 1
  console.log(`FAIL ${error instanceof Error ? error.message.split('\n').slice(0, 12).join(' / ') : String(error)}`)
} finally {
  if (errors.length) console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
  await browser.close()
  await server?.close()
}
process.exit(failures ? 1 : 0)
