// The left panel (2.2) against the engine, through the development host: node tests/left.mjs [chromium|webkit]
// Pages, the layers tree (selection, ranges, disclosure, rename, eye and padlock, drag with the
// engine's drop rules, the hover shared with the canvas, context menu, keys, filter), Assets (tiles
// from assets.list, place an instance, drag with drop.preview), the libraries view, libraries from
// files (add the workspace's library file, insert its component, change the file, review and accept
// the update, remove it), the variable picker (variables.pickFor), and the library sections of the
// style picker and Local variables (styles.libraries, variables.libraries). Prints PASS or FAIL per step.
import { readFileSync } from 'node:fs'
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
const shots = evidence(page, 'left', which)
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
  const node = (id) => designKit((n) => window.designKit.query('node', { id: n }), id)
  const layer = (id) => kit.getByTestId(`design-layer-${id}`)
  const engineHover = () => designKit(() => window.designKit.invoke('inspect').then((text) => JSON.parse(text).hover))
  // Where document pixel (x, y) is in the window.
  const onScreen = async (x, y) => {
    const surface = await kit.getByTestId('kit-canvas-surface').boundingBox()
    const view = await designKit(() => window.designKit.canvas.current.view())
    return { x: surface.x + (x - view.x) * view.zoom, y: surface.y + (y - view.y) * view.zoom }
  }
  // The middle of layer `id` in the window (`absoluteBounds` is page space).
  const middleOf = async (id) => {
    const b = (await node(id)).absoluteBounds
    const area = (await inspect()).area
    return onScreen(b.x - area.x + b.width / 2, b.y - area.y + b.height / 2)
  }
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true)
  await until(async () => (await layer(68).count()) === 1)

  step('lists the top-level layers', (await layer(178).count()) === 1 && (await layer(349).count()) === 1)

  // Selection from the tree.
  await layer(68).click()
  await until(async () => (await inspect()).selection.join() === '68')
  step('a click selects the layer', (await inspect()).selection.join() === '68')
  await layer(178).click({ modifiers: ['Shift'] })
  await until(async () => (await inspect()).selection.length === 3)
  step('Shift-click selects the listed range', (await inspect()).selection.sort().join() === '178,239,68'.split(',').sort().join(), (await inspect()).selection.join())
  await layer(349).click({ modifiers: ['Meta'] })
  await until(async () => (await inspect()).selection.includes(349))
  step('Cmd-click adds a layer', (await inspect()).selection.includes(349) && (await inspect()).selection.length === 4)

  // A selection made elsewhere opens its ancestors.
  await designKit(() => window.designKit.run('select.set', { ids: [79] }))
  await until(async () => (await layer(79).count()) === 1)
  step('reveals a layer selected on the canvas', (await layer(79).getAttribute('aria-selected')) === 'true')

  // Disclosure.
  await kit.getByTestId('design-layer-toggle-79').click()
  await until(async () => (await layer(80).count()) === 1)
  step('the disclosure opens a layer', (await layer(80).count()) === 1)
  await kit.getByTestId('design-layer-toggle-79').click()
  await until(async () => (await layer(80).count()) === 0)
  step('and closes it', (await layer(80).count()) === 0)

  // Keys: Down selects the next row.
  await layer(79).click()
  const rowsNow = await kit.locator('[role=treeitem]').evaluateAll((els) => els.map((el) => Number(el.dataset.rowId)).filter((id, i, all) => all.indexOf(id) === i))
  await page.keyboard.press('ArrowDown')
  const below = (await designKit(() => [...document.querySelectorAll('[role=treeitem]')].sort((a, b) => parseFloat(a.style.top) - parseFloat(b.style.top)).map((el) => Number(el.dataset.rowId))))
  const expectNext = below[below.indexOf(79) + 1]
  await until(async () => (await inspect()).selection.join() === String(expectNext))
  step('Down selects the next row', (await inspect()).selection.join() === String(expectNext), `${rowsNow.length} rows, next ${expectNext}`)
  await page.keyboard.press('ArrowUp')
  await until(async () => (await inspect()).selection.join() === '79')
  step('Up selects the row above', (await inspect()).selection.join() === '79', (await inspect()).selection.join())

  // Rename by double-click.
  const before = (await node(79)).name
  await layer(79).dblclick()
  const field = kit.getByTestId('design-layer-rename')
  await until(async () => (await field.count()) === 1)
  await field.fill('Renamed content')
  await field.press('Enter')
  await until(async () => (await node(79)).name === 'Renamed content')
  step('double-click renames the layer', (await node(79)).name === 'Renamed content')
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await node(79)).name === before)

  // Eye and padlock.
  await layer(79).hover()
  await kit.getByTestId('design-layer-eye-79').click()
  await until(async () => (await node(79)).visible === false)
  step('the eye hides the layer', (await node(79)).visible === false)
  step('a hidden layer keeps its eye shown', await kit.getByTestId('design-layer-eye-79').isVisible())
  await kit.getByTestId('design-layer-eye-79').click()
  await until(async () => (await node(79)).visible !== false)
  step('and shows it again', (await node(79)).visible !== false)

  // A padlock sweep across the rows of Home — Light: one step.
  await kit.getByTestId('design-layer-toggle-68').count()
  const kids = (await designKit(() => window.designKit.query('node', { id: 68, depth: 1 }))).children.map((c) => c.id)
  const top = kids[kids.length - 1]
  const bottom = kids[0]
  await layer(top).hover()
  const lockBox = await kit.getByTestId(`design-layer-lock-${top}`).boundingBox()
  const lastBox = await layer(bottom).boundingBox()
  const undoBefore = (await inspect()).undo
  await page.mouse.move(lockBox.x + 10, lockBox.y + 10)
  await page.mouse.down()
  await page.mouse.move(lockBox.x + 10, lockBox.y + 30, { steps: 4 })
  await page.mouse.move(lockBox.x + 10, lastBox.y + 16, { steps: 8 })
  await page.mouse.up()
  await until(async () => (await Promise.all(kids.map(node))).every((n) => n.locked === true))
  const locked = await Promise.all(kids.map(node))
  step('a padlock sweep locks every row it passes', locked.every((n) => n.locked === true), locked.map((n) => n.locked).join())
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await Promise.all(kids.map(node))).every((n) => !n.locked))
  step('one undo takes the sweep back', (await Promise.all(kids.map(node))).every((n) => !n.locked) && (await inspect()).undo === undoBefore)

  // Drag a row above another: node.reparent.
  const order = async () => (await designKit(() => window.designKit.query('node', { id: 68, depth: 1 }))).children.map((c) => c.id)
  const was = await order()
  const lowest = was[0]
  const highest = was[was.length - 1]
  const from = await layer(lowest).boundingBox()
  const to = await layer(highest).boundingBox()
  await page.mouse.move(from.x + 120, from.y + 16)
  await page.mouse.down()
  await page.mouse.move(from.x + 120, from.y + 6, { steps: 4 })
  await page.mouse.move(to.x + 120, to.y + 4, { steps: 10 })
  step('a drag shows the insertion line', (await kit.locator('.design-insertion-line').count()) === 1)
  await page.mouse.up()
  await until(async () => (await order()).at(-1) === lowest)
  step('dropping above the top row moves the layer to the top', (await order()).at(-1) === lowest, (await order()).join())
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await order()).join() === was.join())

  // The engine's drop rules (layers.dropInfo): a row inside an instance takes no drop.
  await designKit(() => window.designKit.run('select.set', { ids: [237] }))
  await until(async () => (await layer(237).count()) === 1)
  await designKit(() => window.designKit.run('select.set', { ids: [] }))
  await until(async () => (await inspect()).selection.length === 0)
  const lockedRow = await layer(237).boundingBox()
  const dragged = await layer(233).boundingBox()
  await page.mouse.move(dragged.x + 120, dragged.y + 16)
  await page.mouse.down()
  await page.mouse.move(dragged.x + 120, dragged.y + 6, { steps: 4 })
  await page.mouse.move(lockedRow.x + 120, lockedRow.y + 4, { steps: 10 })
  step('a row inside an instance refuses the drop', await until(async () => (await kit.getByTestId('design-layers').getAttribute('data-dragging')) === 'refused'))
  const parentBefore = (await node(233)).parent
  await page.mouse.up()
  await page.waitForTimeout(300)
  step('and dropping there moves nothing', (await node(233)).parent === parentBefore && (await inspect()).undo !== 'Reparent')

  // Holding over a closed frame opens it; the drop then lands on the row now under the pointer,
  // its first child, inside the frame (review #9: the rows from the drag's start were used).
  await designKit(() => window.designKit.run('select.set', { ids: [] }))
  await until(async () => (await inspect()).selection.length === 0)
  const homeKids = (await designKit(() => window.designKit.query('node', { id: 68, depth: 1 }))).children.map((c) => c.id)
  const firstKid = homeKids[homeKids.length - 1]
  // Every row closed and the list at its top, so the rows sit away from the auto-scroll edges.
  await designKit(async () => (await import('/src/kit/requests.ts')).dispatchUiRequests([{ kind: 'layers.collapseAll', data: null }]))
  await until(async () => (await layer(firstKid).count()) === 0)
  await kit.getByTestId('design-layers').evaluate((el) => {
    el.scrollTop = 0
  }).catch(() => undefined)
  await page.waitForTimeout(300)
  const mover = (await layer(178).count()) === 1 ? 178 : 239
  const moverParent = (await node(mover)).parent
  const moverBox = await layer(mover).boundingBox()
  const homeBox = await layer(68).boundingBox()
  await page.mouse.move(moverBox.x + 120, moverBox.y + 16)
  await page.mouse.down()
  await page.mouse.move(moverBox.x + 120, moverBox.y + 6, { steps: 4 })
  await page.mouse.move(homeBox.x + 120, homeBox.y + 16, { steps: 10 })
  const opened = await until(async () => (await layer(firstKid).count()) === 1, 30)
  const dragState = await kit.getByTestId('design-layers').getAttribute('data-dragging')
  await page.waitForTimeout(300)
  if (opened) {
    const kidBox = await layer(firstKid).boundingBox()
    await page.mouse.move(kidBox.x + 120, kidBox.y + 5, { steps: 4 })
    await page.waitForTimeout(300)
  }
  await page.mouse.up()
  await until(async () => (await node(mover)).parent === 68, 30)
  step('a drop after hold-to-open lands on the row under the pointer', opened && (await node(mover)).parent === 68, `opened ${opened} (${dragState}), mover ${mover}, parent ${(await node(mover)).parent}`)
  if ((await node(mover)).parent !== moverParent) await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await node(mover)).parent === moverParent)

  // The hover shared with the canvas: a hovered row outlines its layer, the canvas's hover its row.
  await designKit(() => window.designKit.run('select.set', { ids: [79] }))
  await until(async () => (await layer(79).count()) === 1 && (await layer(68).count()) === 1)
  await designKit(() => window.designKit.run('select.set', { ids: [] }))
  await until(async () => (await inspect()).selection.length === 0)
  await layer(68).hover()
  step('hovering a row sets the shared hover (setHover)', await until(async () => (await engineHover()) === 68), String(await engineHover()))
  const content = await middleOf(79)
  await page.mouse.move(content.x - 20, content.y - 20)
  await page.mouse.move(content.x, content.y, { steps: 4 })
  await until(async () => (await engineHover()) !== 68 && (await engineHover()) != null)
  const hovered = await engineHover()
  step('hovering the canvas outlines that layer\'s row', hovered != null && (await until(async () => (await kit.locator(`[data-row-id='${hovered}'][data-hovered]`).count()) === 1)), String(hovered))
  await layer(68).hover()
  await until(async () => (await engineHover()) === 68)
  step('only the hovered row is outlined', (await kit.locator('[data-hovered]').count()) <= 1)

  // Context menu.
  await layer(79).click({ button: 'right' })
  const menu = kit.getByTestId('design-context-menu')
  await until(async () => (await menu.count()) === 1)
  step('right-click opens the layer menu', (await menu.count()) === 1 && (await kit.getByTestId('design-context-edit.copy').count()) === 1)
  await kit.getByTestId('design-context-object.toggleVisible').click()
  await until(async () => (await node(79)).visible === false)
  step('its Show/Hide hides the layer', (await node(79)).visible === false)
  await designKit(() => window.designKit.run('edit.undo'))

  // Filter.
  await kit.getByTestId('design-layers-search').click()
  await kit.getByTestId('design-layers-filter').fill('status bar')
  await until(async () => (await kit.locator('[role=treeitem]').count()) > 0 && (await kit.locator('[role=treeitem]').allTextContents()).every((text) => text.toLowerCase().includes('status bar')))
  const found = await kit.locator('[role=treeitem]').allTextContents()
  step('the filter lists matching layers', found.length > 0 && found.every((text) => text.toLowerCase().includes('status bar')), found.join(' | '))
  await kit.getByTestId('design-layers-filter').press('Escape')
  await until(async () => (await kit.getByTestId('design-layers-filter').count()) === 0)

  // Pages: add, rename, delete.
  const pages = async () => (await designKit(() => window.designKit.query('pages'))).pages
  const count = (await pages()).length
  await kit.getByTestId('design-pages-add').click()
  await until(async () => (await pages()).length === count + 1)
  step('Add new page adds a page', (await pages()).length === count + 1)
  const added = (await pages()).at(-1)
  await kit.getByTestId(`design-page-${added.id}`).dblclick()
  await until(async () => (await kit.getByTestId('design-page-rename').count()) === 1)
  await kit.getByTestId('design-page-rename').fill('Second')
  await kit.getByTestId('design-page-rename').press('Enter')
  await until(async () => (await pages()).some((p) => p.name === 'Second'))
  step('double-click renames the page', (await pages()).some((p) => p.name === 'Second'))
  await kit.getByTestId(`design-page-${added.id}`).click({ button: 'right' })
  await kit.getByTestId('design-page-menu-delete').click()
  await until(async () => (await pages()).length === count)
  step('the page menu deletes the page', (await pages()).length === count)
  await until(async () => (await inspect()).page === 1 || (await pages()).some((p) => p.current && p.id === 1))
  if ((await inspect()).page !== 1) await kit.getByTestId('design-page-1').click()

  // Assets: a click places an instance in the middle of the view.
  await kit.getByTestId('design-rail-assets').click()
  await until(async () => (await kit.getByTestId('design-asset-55').count()) === 1)
  step('Assets lists the local components', (await kit.getByTestId('design-asset-55').count()) === 1 && (await kit.getByTestId('design-asset-28').count()) === 1 && (await kit.getByTestId('design-asset-20').count()) === 0)
  const layersBefore = (await inspect()).layers
  await kit.getByTestId('design-asset-55').click()
  await until(async () => (await inspect()).layers > layersBefore)
  const placed = await inspect()
  const placedNode = await node(placed.selection[0])
  step('a tile click places an instance', placed.layers > layersBefore && placedNode.type === 'INSTANCE', `${layersBefore} → ${placed.layers}`)
  await designKit(() => window.designKit.run('edit.undo'))
  step('a component set is one tile with its variant count (assets.list)', (await kit.getByTestId('design-asset-28').getAttribute('data-variants')) === '2' && (await kit.getByTestId('design-asset-55').getAttribute('data-set')) === null)
  step('no updates banner without library updates', (await kit.getByTestId('design-assets-updates').count()) === 0)

  // A drag onto an auto layout frame previews the drop (drop.preview) and places the instance in it.
  const tileBox = await kit.getByTestId('design-asset-55').boundingBox()
  const into = await middleOf(79)
  const layersBeforeDrop = (await inspect()).layers
  await page.mouse.move(tileBox.x + 30, tileBox.y + 30)
  await page.mouse.down()
  await page.mouse.move(tileBox.x + 50, tileBox.y + 40, { steps: 3 })
  await page.mouse.move(into.x, into.y, { steps: 12 })
  await page.mouse.move(into.x + 2, into.y + 2, { steps: 2 })
  const feedback = kit.getByTestId('design-asset-drop')
  step('dragging a tile over a frame previews the drop target', await until(async () => (await feedback.getAttribute('data-target')) === '79'), String(await feedback.getAttribute('data-target').catch(() => null)))
  step('and the insertion line in its auto layout', (await kit.getByTestId('design-asset-drop-line').count()) === 1)
  await page.mouse.up()
  await until(async () => (await inspect()).layers > layersBeforeDrop)
  const droppedId = (await inspect()).selection[0]
  step('the drop places an instance inside that frame', (await inspect()).layers > layersBeforeDrop && (await node(droppedId)).parent === 79, String((await node(droppedId)).parent))
  step('the preview goes away after the drop', (await feedback.count()) === 0)
  await designKit(() => window.designKit.run('edit.undo'))
  await until(async () => (await inspect()).layers === layersBeforeDrop)
  // A tile drag the browser cancels places nothing, and the next release does not either (review #25d).
  await page.mouse.move(tileBox.x + 30, tileBox.y + 30)
  await page.mouse.down()
  await page.mouse.move(tileBox.x + 50, tileBox.y + 40, { steps: 3 })
  await page.mouse.move(into.x, into.y, { steps: 12 })
  await until(async () => (await feedback.count()) === 1)
  await frame().evaluate(() => window.dispatchEvent(new PointerEvent('pointercancel', { pointerId: 1, isPrimary: true, bubbles: true })))
  const tileCancelled = await until(async () => (await feedback.count()) === 0, 20)
  await page.mouse.up()
  await page.waitForTimeout(400)
  step('a cancelled tile drag places nothing', tileCancelled && (await inspect()).layers === layersBeforeDrop, `${tileCancelled} ${(await inspect()).layers}`)
  while ((await inspect()).layers > layersBeforeDrop) await designKit(() => window.designKit.run('edit.undo'))

  // The libraries view lists what this file can publish (library.list).
  await kit.getByTestId('design-assets-libraries').click()
  await until(async () => (await kit.getByTestId('design-libraries').count()) === 1)
  step('the libraries view lists the publishable components', await until(async () => (await kit.getByTestId('design-libraries').textContent()).includes('Trail row')))
  await kit.getByTestId('design-libraries-back').click()
  await until(async () => (await kit.getByTestId('design-asset-55').count()) === 1)

  await kit.getByTestId('design-assets-search').fill('chip')
  await until(async () => (await kit.getByTestId('design-asset-55').count()) === 0)
  step('the asset search filters tiles', (await kit.getByTestId('design-asset-55').count()) === 0 && (await kit.getByTestId('design-asset-19').count()) === 1)
  await kit.getByTestId('design-assets-search').fill('')

  // Libraries from files: "Add library file…" takes the library file the host's picker answers
  // (the dev host's workspace holds `libraries/Design System.kdesign`), its components show under its
  // name, a tile places an instance, and when the file is published again (a new version of it in
  // the workspace) the update shows, is reviewed and accepted; then the library is removed.
  await kit.getByTestId('design-assets-libraries').click()
  await until(async () => (await kit.getByTestId('design-libraries').count()) === 1)
  step('the libraries view offers "Add library file…" with the host\'s picker', (await kit.getByTestId('design-libraries-add-file').count()) === 1)
  await kit.getByTestId('design-libraries-add-file').click()
  const row = kit.getByTestId('design-library-Design System')
  step('Add library file… adds the picked library', await until(async () => (await row.count()) === 1, 100))
  const picks = await page.evaluate(() => ({ asked: window.devHost.linkPicks, files: [...window.devHost.files.keys()] }))
  step('Add library file… asks the host for a .kdesign to link (links/pick), and nothing is copied', picks.asked.length === 1 && JSON.stringify(picks.asked[0].accept) === '["kdesign"]' && picks.files.filter((name) => name.endsWith('.kdesign')).length === 2, JSON.stringify(picks))
  const links = await designKit(() => window.designKit.query('library.files'))
  step('the design names the library file by its workspace path', links.files.length === 1 && links.files[0].path === 'libraries/Design System.kdesign', JSON.stringify(links))
  step('the library row shows its version and assets', await until(async () => ((await row.textContent()) ?? '').includes('Version 1')), await row.textContent())
  await kit.getByTestId('design-libraries-back').click()
  const libraryId = links.files[0].id
  const section = kit.getByTestId(`design-assets-library-${libraryId}`)
  step('Assets lists the library under its name', await until(async () => ((await section.textContent().catch(() => '')) ?? '').includes('Design System')))
  const libraryTile = section.locator('[aria-label="Button/Primary"]')
  step('with its published components as tiles', await until(async () => (await libraryTile.count()) === 1))
  step('drawn from the library (libraryThumbnail)', await until(async () => (await libraryTile.getAttribute('data-thumb')) === 'true'))
  step('and its styles', ((await section.textContent()) ?? '').includes('Brand/Primary'))
  const layersBeforeLibrary = (await inspect()).layers
  await libraryTile.click()
  await until(async () => (await inspect()).layers > layersBeforeLibrary)
  const instance = (await inspect()).selection[0]
  step('a library tile places an instance of its component', (await inspect()).layers > layersBeforeLibrary && (await node(instance)).type === 'INSTANCE')
  step('no updates banner while the library is current', (await kit.getByTestId('design-assets-updates').count()) === 0)
  const fillsBefore = JSON.stringify((await node(instance)).fills)
  const published = readFileSync(new URL('./fixtures/Design System v2.kdesign', import.meta.url)).toString('base64')
  await page.evaluate((data) => {
    const bytes = Uint8Array.from(atob(data), (c) => c.charCodeAt(0))
    const old = window.devHost.files.get('libraries/Design System.kdesign')
    window.devHost.files.set('libraries/Design System.kdesign', { bytes: bytes.buffer, version: (old?.version ?? 1) + 1 })
  }, published)
  step('the library file published again shows the updates banner', await until(async () => (await kit.getByTestId('design-assets-updates').count()) === 1, 150))
  await kit.getByTestId('design-assets-updates-review').click()
  step('its review lists the changed component', await until(async () => ((await kit.getByTestId('design-assets-updates-list').textContent().catch(() => '')) ?? '').includes('Button/Primary')))
  await kit.getByTestId('design-assets-update').click()
  step('Update accepts it: the instance follows its library', await until(async () => JSON.stringify((await node(instance)).fills) !== fillsBefore), JSON.stringify((await node(instance)).fills))
  step('and the banner goes away', await until(async () => (await kit.getByTestId('design-assets-updates').count()) === 0))
  await kit.getByTestId('design-assets-libraries').click()
  await until(async () => (await row.count()) === 1)
  await row.hover()
  await kit.getByTestId('design-library-remove-Design System').click()
  step('Remove library takes it out of this file', await until(async () => (await row.count()) === 0) && (await designKit(() => window.designKit.query('library.files'))).files.length === 0)
  await kit.getByTestId('design-libraries-back').click()
  for (let i = 0; i < 4; i += 1) await designKit(() => window.designKit.run('edit.undo'))
  step('undo takes the library steps back', await until(async () => (await inspect()).layers === layersBeforeLibrary && (await designKit(() => window.designKit.query('library.files'))).files.length === 0), String((await inspect()).layers))

  // A drag in colour picker `testId` cut short by Escape, which closes (unmounts) the picker:
  // the drag must still end as one step named `label` (review #16).
  const toolbarInteraction = () => designKit(() => window.designKit.query('toolbar').then((t) => t.interaction))
  const undoLabels = () => designKit(() => window.designKit.invoke('query', 'history', '{}').then((text) => JSON.parse(text).undo))
  const dragThenEscape = async (testId, label) => {
    const sv = await kit.getByTestId(`${testId}-sv`).boundingBox()
    await page.mouse.move(sv.x + sv.width * 0.3, sv.y + sv.height * 0.3)
    await page.mouse.down()
    for (let i = 1; i <= 6; i += 1) {
      await page.mouse.move(sv.x + sv.width * 0.3 + i * 8, sv.y + sv.height * 0.3 + i * 4)
      await page.waitForTimeout(30)
    }
    await page.keyboard.press('Escape')
    await page.mouse.up()
    const closed = await until(async () => (await kit.getByTestId(testId).count()) === 0)
    // The drag's queued updates and its commit run in order on the engine's worker: on a slow
    // machine they can take longer than a fixed pause.
    await until(async () => (await toolbarInteraction()) === null)
    await page.waitForTimeout(400)
    const open = await toolbarInteraction()
    // (Hovering a style row previews it: an "Apply style" step may follow.)
    const labels = (await undoLabels()).slice(0, 3)
    step(`a ${label} drag cut short by closing its picker ends as one step`, closed && open === null && labels.filter((l) => l === label).length === 1, `${closed} open=${open} ${JSON.stringify(labels)}`)
  }

  // The Local variables window: create a number variable, rename it, set it, delete it.
  const variables = async () => (await designKit(() => window.designKit.query('variables'))).variables
  await kit.getByTestId('design-rail-variables').click()
  await until(async () => (await kit.getByTestId('design-variables').count()) === 1)
  step('the rail opens Local variables', (await kit.getByTestId('design-variables').count()) === 1)
  const varsBefore = (await variables()).length
  await kit.getByTestId('design-variables-create').click()
  await kit.getByTestId('design-variables-create-number').click()
  await until(async () => (await kit.getByTestId('design-variables-rename').count()) === 1)
  await kit.getByTestId('design-variables-rename').fill('Gutter')
  await kit.getByTestId('design-variables-rename').press('Enter')
  await until(async () => (await variables()).some((v) => v.name === 'Gutter'))
  const gutter = (await variables()).find((v) => v.name === 'Gutter')
  step('+ Create variable makes a variable and renames it', (await variables()).length === varsBefore + 1 && Boolean(gutter))
  const cell = kit.getByTestId(`design-variables-cell-${gutter.id}-${Object.keys(gutter.values)[0]}`).locator('input')
  await cell.fill('24')
  await cell.press('Enter')
  await until(async () => (await variables()).find((v) => v.id === gutter.id).values[Object.keys(gutter.values)[0]].value === 24)
  step('a value cell sets the value', (await variables()).find((v) => v.id === gutter.id).values[Object.keys(gutter.values)[0]].value === 24)
  await kit.getByTestId(`design-variables-row-${gutter.id}`).click({ button: 'right' })
  await kit.getByTestId('design-variables-delete').click()
  await until(async () => !(await variables()).some((v) => v.id === gutter.id))
  step('the row menu deletes the variable', !(await variables()).some((v) => v.id === gutter.id))
  const colorVar = (await variables()).find((v) => Object.values(v.values).some((value) => value.type === 'color'))
  await kit.getByTestId(`design-variables-row-${colorVar.id}`).locator('.design-vars-color-swatch').first().click()
  await until(async () => (await kit.getByTestId('design-variables-color').count()) === 1)
  await dragThenEscape('design-variables-color', 'Edit variable')
  await designKit(() => window.designKit.run('edit.undo'))
  await kit.getByTestId('design-variables-close').click()
  await until(async () => (await kit.getByTestId('design-variables').count()) === 0)

  // The style picker: create a colour style from the selection, then apply it to another layer.
  // The picker opens on `inspect.selection`: wait for the selection to reach it.
  const selectOnly = async (id) => {
    await designKit((n) => window.designKit.run('select.set', { ids: [n] }), id)
    await until(async () => JSON.stringify(await designKit(() => window.designKit.inspect.selection)) === JSON.stringify([id]))
  }
  await selectOnly(239)
  const openPicker = () => designKit(async () => (await import('/src/kit/requests.ts')).dispatchUiRequests([{ kind: 'stylePicker.open', data: { ids: window.designKit.inspect.selection, slot: 'fill', y: 200 } }]))
  await openPicker()
  await until(async () => (await kit.getByTestId('design-style-picker').count()) === 1)
  step('stylePicker.open shows the style picker', (await kit.getByTestId('design-style-picker').count()) === 1)
  await kit.getByTestId('design-style-create').click()
  await until(async () => (await kit.getByTestId('design-style-editor').count()) === 1)
  await kit.getByTestId('design-style-name').fill('Brand/Night')
  await kit.getByTestId('design-style-back').click()
  const styles = async () => designKit(() => window.designKit.query('styles'))
  await until(async () => (await styles()).some((st) => st.name === 'Brand/Night'))
  const night = (await styles()).find((st) => st.name === 'Brand/Night')
  step('"+" creates a style and the editor renames it', Boolean(night))
  await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('design-style-picker').count()) === 0)
  await selectOnly(68)
  await openPicker()
  await until(async () => (await kit.getByTestId(`design-style-${night.id}`).count()) === 1)
  await kit.getByTestId(`design-style-${night.id}`).click()
  await until(async () => (await node(68)).styles?.fill === night.id)
  step('a click applies the style', (await node(68)).styles?.fill === night.id)
  // Its paint's colour picker (Edit style), closed mid-drag.
  if (!(await kit.getByTestId('design-style-picker').count())) await openPicker()
  await until(async () => (await kit.getByTestId(`design-style-${night.id}`).count()) === 1)
  await kit.getByTestId(`design-style-${night.id}`).hover()
  await kit.getByTestId(`design-style-edit-${night.id}`).click()
  await until(async () => (await kit.getByTestId('design-style-editor').count()) === 1)
  await kit.getByTestId('design-style-editor').locator('.design-style-prop[role=button]').first().click()
  await until(async () => (await kit.getByTestId('design-style-color').count()) === 1)
  const beforeStyleDrag = (await undoLabels()).length
  await dragThenEscape('design-style-color', 'Edit style')
  if (await kit.getByTestId('design-style-picker').count()) await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('design-style-picker').count()) === 0)
  while ((await undoLabels()).length > beforeStyleDrag) await designKit(() => window.designKit.run('edit.undo'))

  // The variable picker lists what the engine offers (variables.pickFor): the field's scope…
  const openVariables = (data) => designKit(async (d) => (await import('/src/kit/requests.ts')).dispatchUiRequests([{ kind: 'variablePicker.open', data: d }]), data)
  const picker = kit.getByTestId('design-variable-picker')
  await designKit(() => window.designKit.run('variable.update', { variable: 'VariableID:2', scopes: ['TEXT_FILL'] }))
  await openVariables({ target: { bind: { ids: [68], field: 'fills' } }, kind: 'color', x: 300, y: 200 })
  await until(async () => (await kit.getByTestId('design-variable-pick-VariableID:1').count()) === 1)
  step('the variable picker leaves out variables scoped elsewhere', (await kit.getByTestId('design-variable-pick-VariableID:1').count()) === 1 && (await kit.getByTestId('design-variable-pick-VariableID:2').count()) === 0 && (await kit.getByTestId('design-variable-pick-VariableID:11').count()) === 0)
  await kit.getByTestId('design-variable-picker-search').fill('brand')
  step('its search filters by name', await until(async () => (await kit.getByTestId('design-variable-pick-VariableID:1').count()) === 0 && (await kit.getByTestId('design-variable-pick-VariableID:7').count()) === 1))
  await kit.getByTestId('design-variable-pick-VariableID:7').click()
  await until(async () => JSON.stringify((await node(68)).fills?.[0]).includes('VariableID:7') || JSON.stringify(await node(68)).includes('VariableID:7'))
  step('picking binds the field', JSON.stringify(await node(68)).includes('VariableID:7'))
  await designKit(() => window.designKit.run('edit.undo'))
  await designKit(() => window.designKit.run('edit.undo'))
  // …and alias loops.
  const theme = (await variables()).find((v) => v.id === 'VariableID:3')
  const mode = Object.keys(theme.values)[0]
  await designKit((m) => window.designKit.run('variable.set', { variable: 'VariableID:3', mode: m, value: { alias: 'VariableID:1' } }), mode)
  await openVariables({ target: { alias: { variable: 'VariableID:1', mode } }, x: 300, y: 200 })
  await until(async () => (await kit.getByTestId('design-variable-pick-VariableID:4').count()) === 1)
  step('an alias picker leaves out the variable and what would loop', (await kit.getByTestId('design-variable-pick-VariableID:4').count()) === 1 && (await kit.getByTestId('design-variable-pick-VariableID:1').count()) === 0 && (await kit.getByTestId('design-variable-pick-VariableID:3').count()) === 0)
  await page.keyboard.press('Escape')
  await until(async () => (await picker.count()) === 0)
  await designKit(() => window.designKit.run('edit.undo'))

  // Library styles and collections (styles.libraries, variables.libraries): the enabled library's
  // published styles follow under its name in the style picker, read-only, and a click copies one
  // in and applies it (library.applyStyle); its collections follow under its name in Local
  // variables, read-only, with "Extend collection" (library.extendCollection).
  {
    const path = 'libraries/Design System.kdesign'
    const data = await page.evaluate((p) => {
      const bytes = new Uint8Array(window.devHost.files.get(p).bytes)
      let text = ''
      for (let i = 0; i < bytes.length; i += 0x8000) text += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
      return btoa(text)
    }, path)
    const added = await designKit(async ({ p, d }) => {
      await window.designKit.invoke('lendLibrary', p, Uint8Array.from(atob(d), (c) => c.charCodeAt(0)))
      return window.designKit.run('library.add', { path: p })
    }, { p: path, d: data })
    const lib = added.id
    const libStyles = await designKit(() => window.designKit.query('styles.libraries', { type: 'paint' }))
    const brand = libStyles.libraries.find((l) => l.id === lib)?.styles.find((st) => st.name === 'Brand/Primary')
    step('styles.libraries answers the library\'s published paint styles', Boolean(brand), JSON.stringify(libStyles).slice(0, 300))
    await selectOnly(239)
    await openPicker()
    const libSection = kit.getByTestId(`design-style-library-${lib}`)
    step('the style picker lists the library\'s styles under its name', await until(async () => ((await libSection.textContent().catch(() => '')) ?? '').includes('Design System')) && (await kit.getByTestId(`design-library-style-${lib}-${brand.id}`).count()) === 1)
    step('"This file" heads the local styles then', (await kit.getByTestId('design-style-section-local').textContent()) === 'This file')
    step('library styles have no editor button', (await kit.getByTestId(`design-style-edit-${brand.id}`).count()) === 0)
    await kit.getByTestId(`design-library-style-${lib}-${brand.id}`).click({ button: 'right' })
    await page.waitForTimeout(150)
    step('a right-click on a library style opens no editor', (await kit.getByTestId('design-style-editor').count()) === 0)
    await kit.getByTestId(`design-library-style-${lib}-${brand.id}`).click()
    const copyOf = async () => {
      const fill = (await node(239)).styles?.fill
      const copies = (await designKit(() => window.designKit.query('styles.libraries'))).copies
      return fill && copies[fill] ? { fill, ...copies[fill] } : null
    }
    step('a click copies the library style in and applies it', await until(async () => (await copyOf())?.style === brand.id), JSON.stringify(await copyOf()))
    const copy = await copyOf()
    await until(async () => (await kit.getByTestId('design-style-picker').count()) === 0)
    await openPicker()
    await until(async () => (await kit.getByTestId(`design-library-style-${lib}-${brand.id}`).count()) === 1)
    step('the applied copy shows as its library\'s row, checked', (await kit.getByTestId(`design-library-style-${lib}-${brand.id}`).getAttribute('data-applied')) === 'true' && (await kit.getByTestId(`design-style-${copy.fill}`).count()) === 0)
    const libGroup = kit.getByTestId(`design-style-library-variables-${lib}`)
    step('its colour variables follow as "{library} · {collection}"', ((await libGroup.first().textContent()) ?? '').startsWith('Design System · Theme'))
    await libGroup.first().locator('.design-var-row').first().click()
    const boundCopy = async () => {
      const linked = (await designKit(() => window.designKit.query('variables.libraries'))).copies.variables
      return linked.length === 1 && JSON.stringify(await node(239)).includes(`"${linked[0]}"`)
    }
    step('a click on one copies it in and binds the fill', await until(boundCopy))
    await until(async () => (await kit.getByTestId('design-style-picker').count()) === 0)

    const libVars = (await designKit(() => window.designKit.query('variables.libraries'))).libraries.find((l) => l.id === lib)
    const themeColl = libVars?.collections.find((c) => c.name === 'Theme')
    step('variables.libraries answers the library\'s collections', Boolean(themeColl) && libVars.variables.some((v) => v.collection === themeColl.id))
    await kit.getByTestId('design-rail-variables').click()
    await until(async () => (await kit.getByTestId('design-variables').count()) === 1)
    const heading = kit.getByTestId(`design-variables-library-${lib}`)
    step('Local variables lists the library under its name', await until(async () => ((await heading.textContent().catch(() => '')) ?? '').includes('Design System')))
    await kit.getByTestId(`design-variables-library-collection-${lib}-${themeColl.id}`).click()
    const note = kit.getByTestId('design-variables-library-note')
    step('its collection opens read-only', await until(async () => ((await note.textContent().catch(() => '')) ?? '') === 'From the library "Design System": read-only'))
    const surface = libVars.variables.find((v) => v.collection === themeColl.id)
    step('with each mode\'s value shown', (await kit.getByTestId(`design-variables-library-row-${surface.id}`).count()) === 1 && (await kit.getByTestId(`design-variables-library-cell-${surface.id}-${themeColl.modes[1].id}`).textContent()).length > 0)
    step('and nothing to create, add or rename', (await kit.getByTestId('design-variables-create').count()) === 0 && (await kit.getByTestId('design-variables-add-mode').count()) === 0 && (await kit.locator('[data-testid="design-variables-table"] input').count()) === 0)
    await kit.getByTestId('design-variables-extend').click()
    const extended = async () => (await designKit(() => window.designKit.query('variables'))).collections.find((c) => c.extends && c.name === 'Theme (extended)')
    step('Extend collection makes a collection of this file and opens it', await until(async () => Boolean(await extended())) && (await until(async () => (await kit.getByTestId(`design-variables-collection-${(await extended()).id}`).getAttribute('data-selected')) === 'true')))
    const linked = (await designKit(() => window.designKit.query('variables.libraries'))).copies.collections
    step('the linked copy is not among this file\'s collections', linked.length === 1 && (await kit.getByTestId(`design-variables-collection-${linked[0]}`).count()) === 0, JSON.stringify(linked))
    await kit.getByTestId('design-variables-close').click()
    await until(async () => (await kit.getByTestId('design-variables').count()) === 0)
    for (let i = 0; i < 4; i += 1) await designKit(() => window.designKit.run('edit.undo'))
    step('undo takes the library steps back', await until(async () => (await designKit(() => window.designKit.query('library.files'))).files.length === 0))
  }

  // The Design panel's local styles (upstream `local_styles`): per type, by folder; a click opens the editor.
  await designKit(() => window.designKit.run('select.none', {}))
  await designKit(() => window.designKit.run('style.create', { type: 'paint', name: 'Brand/Primary', value: [{ type: 'solid', color: '#00875F' }] }))
  await until(async () => (await kit.getByTestId('design-local-styles').getByText('Primary', { exact: true }).count()) === 1)
  step('the Design panel lists local styles by folder', (await kit.getByTestId('design-local-styles').getByText('Brand', { exact: true }).count()) === 1 && (await kit.getByTestId('design-local-styles').getByText('Color styles').count()) === 1)
  await kit.getByTestId('design-local-styles').getByText('Primary', { exact: true }).click()
  await until(async () => (await kit.getByTestId('design-style-editor').count()) === 1)
  step('a local style opens its editor', (await kit.getByTestId('design-style-editor').count()) === 1)
  for (let i = 0; i < 3 && (await kit.getByTestId('design-style-picker').count()); i += 1) {
    await page.keyboard.press('Escape')
    await page.waitForTimeout(150)
  }
  // A row dragged by its grip moves into the folder it lands in (`style.move`): Brand/Primary to the top.
  await designKit(() => window.designKit.run('style.create', { type: 'paint', name: 'Accent', value: [{ type: 'solid', color: '#5645D4' }] }))
  const styleList = async () => designKit(() => window.designKit.query('styles'))
  await until(async () => (await styleList()).some((st) => st.name === 'Accent'))
  const primary = (await styleList()).find((st) => st.name === 'Brand/Primary')
  const colorRows = kit.getByTestId('design-local-styles-fill').locator('.design-local-style')
  await until(async () => (await colorRows.count()) >= 2)
  const firstRow = await colorRows.first().boundingBox()
  const gripBox = await kit.getByTestId(`design-local-style-grip-${primary.id}`).boundingBox()
  await page.mouse.move(gripBox.x + gripBox.width / 2, gripBox.y + gripBox.height / 2)
  await page.mouse.down()
  await page.mouse.move(gripBox.x + gripBox.width / 2, firstRow.y + 6, { steps: 6 })
  step('dragging a style shows the insertion line', await until(async () => (await kit.getByTestId('design-local-styles-fill').locator('.design-insertion-line').count()) === 1))
  await page.mouse.up()
  await until(async () => (await styleList()).find((st) => st.id === primary.id)?.name === 'Primary')
  const moved = await styleList()
  const paints = moved.filter((st) => st.type === 'paint')
  const at = paints.findIndex((st) => st.id === primary.id)
  step('dropping a style on the top row moves it out of its folder, before that row', at >= 0 && paints[at].name === 'Primary' && paints[at + 1]?.name === 'Accent', paints.map((st) => st.name).join(','))
  step('a drag by the grip does not open the editor', (await kit.getByTestId('design-style-picker').count()) === 0)
  await designKit(() => window.designKit.run('edit.undo'))
  // A right-click on a folder renames it (`style.renameFolder`).
  await until(async () => (await kit.getByTestId('design-local-folder-Brand').count()) === 1)
  await kit.getByTestId('design-local-folder-Brand').click({ button: 'right' })
  await kit.getByTestId('design-local-folder-menu-rename').click()
  await until(async () => (await kit.getByTestId('design-local-folder-rename').count()) === 1)
  await kit.getByTestId('design-local-folder-rename').fill('Tokens')
  await kit.getByTestId('design-local-folder-rename').press('Enter')
  step('Rename folder renames the styles in it', await until(async () => (await styleList()).some((st) => st.id === primary.id && st.name === 'Tokens/Primary')))
  await designKit(() => window.designKit.run('edit.undo'))
  await designKit(() => window.designKit.run('edit.undo'))
  await designKit(() => window.designKit.run('edit.undo'))

  // The page context menu is upstream's plain popup buttons.
  if ((await kit.locator('.design-page-row').count()) === 0) await kit.getByTestId('design-rail-file').click()
  await until(async () => (await kit.locator('.design-page-row').count()) > 0)
  await kit.locator('.design-page-row').first().click({ button: 'right' })
  await until(async () => (await kit.getByTestId('design-page-menu').count()) === 1)
  step('the page menu has 24 px button rows', (await kit.getByTestId('design-page-menu-rename').boundingBox())?.height === 24)
  await page.keyboard.press('Escape')

  // Collapse panel.
  await kit.getByTestId('design-left-collapse').click()
  await until(async () => (await kit.getByTestId('design-left-panel').count()) === 0)
  step('Collapse panel closes the panel', (await kit.getByTestId('design-left-panel').count()) === 0)
  {
    // The canvas takes the panel's room; the right panel stays at the right edge.
    const right = await kit.getByTestId('design-right').boundingBox()
    step('the right panel stays at the right with the left panel closed', right && Math.abs(right.x + right.width - 1440) < 1 && right.width === 240, JSON.stringify(right))
  }
  await kit.getByTestId('design-rail-file').click()
  await until(async () => (await kit.getByTestId('design-left-panel').count()) === 1)
  step('the rail opens it again', (await kit.getByTestId('design-left-panel').count()) === 1 && (await until(async () => (await layer(68).count()) === 1)), `${await kit.getByTestId('design-left-panel').count()}`)
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
