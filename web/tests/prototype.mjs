// The prototype package in a real browser, through the development host: node tests/prototype.mjs [chromium|webkit]
// The Prototype tab (flows, settings, interactions and the interaction editor), connections on the
// canvas, the preview pane and the presenter, then Dev Mode's steps (tests/prototype-dev.mjs).
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { evidence, traceEvents, traced } from './evidence.mjs'
import { devSteps } from './prototype-dev.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
const page = await context.newPage()
const shots = evidence(page, 'prototype', which)
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
  const designKit = (body, arg) => frame().evaluate(body, arg)
  const query = (name, params = {}) => designKit(([n, p]) => window.designKit.query(n, p), [name, params])
  const run = (command, params = {}) => designKit(([c, p]) => window.designKit.run(c, p), [command, params])
  const proto = () => query('prototype')
  const interactions = async (id) => (await query('node', { id })).interactions ?? []
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true)
  await page.waitForTimeout(800)
  const canvas = await kit.getByTestId('kit-canvas-surface').boundingBox()
  const toPage = async (x, y) => {
    const view = await designKit(() => window.designKit.canvas.current.view())
    return { x: canvas.x + (x - view.x) * view.zoom, y: canvas.y + (y - view.y) * view.zoom }
  }
  /** Types into a field as a person does: focus it first (it selects its text on focus), then type and press Enter. */
  const type = async (input, text) => {
    await input.click()
    await page.waitForTimeout(150)
    await input.fill(text)
    await input.press('Enter')
  }
  const HOME = 68
  const CHIP = 19

  async function playerSteps() {
    const screen = kit.getByTestId('design-proto-present-player-screen')
    const canvasOf = kit.getByTestId('design-proto-present-player-canvas')
    const shown = async () => Number(await canvasOf.getAttribute('data-frame'))
    const drawn = async () => Number((await canvasOf.getAttribute('data-drawn')) ?? 0)
    const centre = async () => {
      const b = await screen.boundingBox()
      return { x: b.x + b.width / 2, y: b.y + b.height / 2, b }
    }
    /** A checksum of what the canvas shows. */
    const picture = (c) => {
      const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data
      let h = 0
      for (let i = 0; i < d.length; i += 97) h = (h * 31 + d[i]) | 0
      return String(h)
    }
    const add = (id, interaction) => run('proto.interaction.add', { ids: [id], ...interaction })
    const clear = (id) => run('proto.interaction.clear', { ids: [id] })
    await run('proto.settings', { device: 'None', responsive: false })
    for (const id of [HOME, TRAIL, DARK]) await clear(id)
    await add(HOME, { trigger: { type: 'onClick' }, actions: [{ type: 'navigate', dest: TRAIL }], transition: { type: 'dissolve' }, duration_ms: 400, easing: 'linear' })
    await add(HOME, { trigger: { type: 'keyPress', key: 'O' }, actions: [{ type: 'openOverlay', dest: CHIP, position: 'center', close_on_outside: true, background: null, anchor: 'screen' }], transition: { type: 'instant' }, duration_ms: 300 })
    await add(HOME, { trigger: { type: 'keyPress', key: 'L' }, actions: [{ type: 'openUrl', url: 'https://example.com/trail' }], transition: { type: 'instant' }, duration_ms: 300 })
    await add(TRAIL, { trigger: { type: 'onClick' }, actions: [{ type: 'back' }], transition: { type: 'push', direction: 'right' }, duration_ms: 300, easing: 'easeOut' })
    await add(DARK, { trigger: { type: 'afterDelay', ms: 1500 }, actions: [{ type: 'navigate', dest: HOME }], transition: { type: 'instant' }, duration_ms: 300 })

    await run('proto.present', { start: HOME })
    step('player: Present opens on proto.present', await until(async () => (await kit.getByTestId('design-proto-present').count()) === 1 && (await shown()) === HOME))
    step('player: the engine draws the screen', await until(async () => (await drawn()) > 0 && (await canvasOf.evaluate((c) => c.width)) > 0))
    const homePixel = await canvasOf.evaluate(picture)
    let at = await centre()
    const before = await drawn()
    await page.mouse.click(at.x, at.y)
    step('player: a click on a hotspot changes the frame', await until(async () => (await shown()) === TRAIL && (await kit.getByTestId('design-proto-present-frame').textContent()).includes('Trail')))
    await page.waitForTimeout(700)
    step('player: the dissolve draws in-between frames', (await drawn()) - before >= 3, `${(await drawn()) - before} pictures`)
    // The dissolve renders its two screens once and composites them in the page every frame.
    const composited = Number((await canvasOf.getAttribute('data-composited')) ?? 0)
    step('player: the dissolve is composited from two pictures', composited >= 8, `${composited} composited frames`)
    const trailPixel = await canvasOf.evaluate(picture)
    step('player: and the pixels are the new frame', trailPixel !== homePixel, `${homePixel} → ${trailPixel}`)
    at = await centre()
    await page.mouse.click(at.x, at.y)
    step('player: Back goes back', await until(async () => (await shown()) === HOME))

    // Inspect: the layer under the pointer, outlined, its properties in a panel at the right.
    await kit.getByTestId('design-proto-present-inspect-button').click()
    step('player: Inspect opens its panel', await until(async () => (await kit.getByTestId('design-proto-present-inspect').count()) === 1 && (await kit.getByTestId('design-proto-present-inspect-button').getAttribute('aria-pressed')) === 'true'))
    at = await centre()
    await page.mouse.move(at.b.x + at.b.width / 2, at.b.y + 12)
    await page.mouse.move(at.b.x + at.b.width / 2 + 2, at.b.y + 14)
    step('player: Inspect outlines the layer under the pointer', await until(async () => (await kit.getByTestId('design-proto-present-inspect-outline').count()) === 1))
    step('player: and lists its properties', await until(async () => (await kit.getByTestId('design-proto-present-inspect-name').count()) === 1 && (await kit.getByTestId('design-proto-present-inspect-section-layout').count()) === 1))
    const layoutText = (await kit.getByTestId('design-proto-present-inspect-section-layout').textContent({ timeout: 2000 }).catch(() => '')) ?? ''
    step('player: the panel shows sizes', /Width/.test(layoutText) && /Height/.test(layoutText), layoutText.slice(0, 60))
    await kit.getByTestId('design-proto-present-inspect-button').click()
    step('player: Inspect again closes the panel', await until(async () => (await kit.getByTestId('design-proto-present-inspect').count()) === 0))

    // Comments: the pins of the frame's comments; a pin opens its thread; Escape closes the thread first.
    const home = (await query('node', { id: HOME })).absoluteBounds
    const added = await run('comment.add', { x: home.x + 100, y: home.y + 200, text: 'Presenter comment', anchor: true })
    await kit.getByTestId('design-proto-present-comments').click()
    const pin = kit.getByTestId(`design-proto-present-pin-${added.id}`)
    step('player: Comments shows the frame\'s pins', await until(async () => (await pin.count()) === 1))
    const pb = await pin.boundingBox()
    at = await centre()
    const scale = at.b.width / home.width
    step('player: the pin sits where the comment is on the frame', Math.abs(pb.x - (at.b.x + 100 * scale)) < 2 && Math.abs(pb.y + pb.height - (at.b.y + 200 * scale)) < 2, `${pb.x},${pb.y + pb.height} vs ${at.b.x + 100 * scale},${at.b.y + 200 * scale}`)
    await pin.click()
    step('player: a pin opens its thread', await until(async () => (await kit.getByTestId('design-comment-thread').textContent({ timeout: 200 })).includes('Presenter comment')))
    await page.keyboard.press('Escape')
    step('player: Escape closes the thread, not the presenter', await until(async () => (await kit.getByTestId('design-comment-thread').count()) === 0 && (await kit.getByTestId('design-proto-present').count()) === 1))
    await page.keyboard.press('Shift+C')
    step('player: Shift+C hides the comments', await until(async () => (await pin.count()) === 0 && (await kit.getByTestId('design-proto-present-comments').getAttribute('aria-pressed')) === 'false'))
    await run('comment.delete', { id: added.id })

    // An overlay from a key, closed by a click outside it.
    await page.keyboard.press('o')
    step('player: a key trigger opens an overlay', await until(async () => (await screen.getAttribute('data-overlays')) === String(CHIP)))
    at = await centre()
    await page.mouse.click(at.b.x + 4, at.b.y + 4)
    step('player: a click outside closes it', await until(async () => (await screen.getAttribute('data-overlays')) === ''))

    // Open link: the notice first.
    await page.keyboard.press('l')
    step('player: Open link asks before leaving the prototype', await until(async () => (await kit.getByTestId('design-proto-link-notice-url').textContent()) === 'https://example.com/trail'))
    await kit.getByTestId('design-proto-link-notice-cancel').click()
    step('player: Cancel stays', await until(async () => (await kit.getByTestId('design-proto-link-notice').count()) === 0))

    // Next frame (the engine's arrow keys), then Restart.
    await kit.getByTestId('design-proto-present-next').click()
    step('player: Next frame steps', await until(async () => (await shown()) === TRAIL))
    await kit.getByTestId('design-proto-present-restart').click()
    step('player: Restart goes back to the start', await until(async () => (await shown()) === HOME))

    // Options through proto.presentOptions: the scale.
    const width = (await screen.boundingBox()).width
    await kit.getByTestId('design-proto-present-options').click()
    await kit.getByTestId('design-proto-present-scale-actual').click()
    step('player: Actual size shows the frame at 100%', await until(async () => Math.abs((await screen.boundingBox()).width - 393) < 1.5), `${width} → ${(await screen.boundingBox()).width}`)
    await kit.getByTestId('design-proto-present-scale').click()
    await kit.getByTestId('design-proto-present-scale-menu').getByText('Fit to window').click()
    step('player: the scale menu fits it again', await until(async () => Math.abs((await screen.boundingBox()).width - width) < 1.5))

    // A device: its body around the screen.
    await kit.getByTestId('design-proto-present-device').click()
    await kit.getByTestId('design-proto-present-device-iPhone 16').click()
    step('player: a device draws its body', await until(async () => (await kit.getByTestId('design-proto-device-body').count()) === 1))
    await run('proto.settings', { device: 'None', responsive: false })
    await kit.getByTestId('design-proto-present-close').click()
    step('player: Close ends presenting', await until(async () => (await kit.getByTestId('design-proto-present').count()) === 0))

    // After delay, and hotspot hints on a click where nothing reacts.
    await run('proto.present', { start: DARK })
    await until(async () => (await shown()) === DARK)
    at = await centre()
    await page.mouse.click(at.x, at.y)
    step('player: a missed click flashes the hotspot hints', await until(async () => (await kit.getByTestId('design-proto-present-player-hints').count()) === 1, 10))
    step('player: After delay navigates by itself', await until(async () => (await shown()) === HOME))
    await page.keyboard.press('Escape')
    step('player: Escape closes the presenter', await until(async () => (await kit.getByTestId('design-proto-present').count()) === 0))
    for (const id of [HOME, TRAIL, DARK]) await clear(id)
  }
  const DARK = 239
  const TRAIL = 178

  // Nothing selected: flows and prototype settings.
  await kit.getByTestId('design-tab-prototype').click()
  step('the Prototype tab shows flows and settings', await until(async () => (await kit.getByTestId('design-proto-flows').count()) === 1 && (await kit.getByTestId('design-proto-settings').count()) === 1))
  await kit.getByTestId('design-proto-flows').getByText('Flow starting points').click()
  step('the empty header adds a starting point', await until(async () => (await proto()).flows.length === 1))
  await kit.getByTestId('design-proto-device').click()
  await kit.getByTestId('design-proto-device-iPhone 16').click()
  step('the device dropdown sets the device', await until(async () => (await proto()).settings.device === 'iPhone 16'))
  await kit.getByTestId('design-proto-rotate').click()
  step('rotate turns it to landscape', await until(async () => (await proto()).settings.rotation === 'landscape'))
  await kit.getByTestId('design-proto-device-color').click()
  await kit.getByTestId('design-proto-device-color-Teal').click()
  step('the colour menu picks the device colour', await until(async () => (await proto()).settings.color === 'Teal'))
  await kit.getByTestId('design-proto-background').locator('input').fill('336699')
  await kit.getByTestId('design-proto-background').locator('input').press('Enter')
  step('the background field sets the background', await until(async () => Math.abs((await proto()).settings.background.b - 0.6) < 0.01))
  const flowName = kit.getByTestId('design-proto-flow-name').first()
  await flowName.fill('Onboarding')
  await flowName.press('Enter')
  step('the flow name renames the flow', await until(async () => (await proto()).flows[0]?.name === 'Onboarding'))

  // A frame: add an interaction, edit it.
  await run('select.set', { ids: [HOME] })
  await until(async () => (await kit.getByTestId('design-proto-interactions').count()) === 1)
  await traceEvents(frame())
  await kit.getByTestId('design-proto-interaction-add').click()
  step('+ adds a click interaction and opens its editor', await until(async () => (await interactions(HOME)).length === 1 && (await kit.getByTestId('design-proto-editor').count()) === 1), `${(await interactions(HOME)).length} on ${HOME}, editor ${await kit.getByTestId('design-proto-editor').count()}, selection ${JSON.stringify(await designKit(() => window.designKit.inspect.selection))}, events ${await traced(frame())}`)
  step('the row summarises it', await until(async () => (await kit.getByTestId('design-proto-interaction-0').textContent()).includes('On click')))
  await kit.getByTestId('design-proto-trigger').click()
  await kit.getByTestId('design-proto-trigger-After delay').click()
  step('the trigger dropdown sets After delay 800ms', await until(async () => (await interactions(HOME))[0]?.trigger.type === 'afterDelay' && (await interactions(HOME))[0].trigger.ms === 800))
  await type(kit.getByTestId('design-proto-delay').locator('input'), '1200')
  step('the delay field sets the delay', await until(async () => (await interactions(HOME))[0]?.trigger.ms === 1200))
  await kit.getByTestId('design-proto-trigger').click()
  await kit.getByTestId('design-proto-trigger-Key/Gamepad').click()
  await until(async () => (await kit.getByTestId('design-proto-key').textContent()).includes('Press a key'))
  await page.keyboard.press('Shift+K')
  step('Key/Gamepad records the next key', await until(async () => (await interactions(HOME))[0]?.trigger.key === 'Shift+K'))
  await kit.getByTestId('design-proto-destination').click()
  await kit.getByTestId(`design-proto-destination-${TRAIL}`).click()
  step('the destination dropdown sets the destination', await until(async () => (await interactions(HOME))[0]?.actions[0]?.dest === TRAIL))
  await kit.getByTestId('design-proto-transition').click()
  await kit.getByTestId('design-proto-transition-push').click()
  step('the animation dropdown sets Push', await until(async () => (await interactions(HOME))[0]?.transition.type === 'push'))
  await kit.getByTestId('design-proto-direction-right').click()
  step('the direction sets right', await until(async () => (await interactions(HOME))[0]?.transition.direction === 'right'))
  await kit.getByTestId('design-proto-easing').click()
  await kit.getByTestId('design-proto-easing-linear').click()
  step('the easing menu sets Linear', await until(async () => (await interactions(HOME))[0]?.easing === 'linear'))
  await type(kit.getByTestId('design-proto-duration').locator('input'), '450')
  step('the duration field sets the duration', await until(async () => (await interactions(HOME))[0]?.duration_ms === 450), `${(await interactions(HOME))[0]?.duration_ms} ${await kit.getByTestId('design-proto-duration').locator('input').inputValue()}`)
  await kit.getByTestId('design-proto-easing').click()
  await kit.getByTestId('design-proto-easing-customBezier').click()
  step('Custom bezier shows its four fields', await until(async () => typeof (await interactions(HOME))[0]?.easing === 'object' && (await kit.getByTestId('design-proto-easing-field-3').count()) === 1))
  await kit.getByTestId('design-proto-add-action').click()
  await kit.getByTestId('design-proto-action-kind-open-link').click()
  step('Add action adds an Open link', await until(async () => (await interactions(HOME))[0]?.actions[1]?.type === 'openUrl'))
  await kit.getByTestId('design-proto-url').locator('input').fill('https://example.com')
  await kit.getByTestId('design-proto-url').locator('input').press('Enter')
  step('the link field sets the URL', await until(async () => (await interactions(HOME))[0]?.actions[1]?.url === 'https://example.com'))
  await kit.getByTestId('design-proto-action-more-1').click()
  await kit.getByTestId('design-proto-action-remove').click()
  step('More > Remove removes the action', await until(async () => (await interactions(HOME))[0]?.actions.length === 1))
  await kit.getByTestId('design-proto-editor-close').click()
  step('Close closes the editor', await until(async () => (await kit.getByTestId('design-proto-editor').count()) === 0))
  await kit.getByTestId('design-proto-overflow').click()
  await kit.getByTestId('design-proto-overflow-vertical').click()
  step('Overflow sets vertical scrolling', await until(async () => (await kit.getByTestId('design-proto-overflow').textContent()).includes('Vertical')))

  // The canvas: the connection, and a new one dragged from the "+" handle.
  await designKit(() => window.designKit.viewTo('view.zoomFit'))
  await page.waitForTimeout(600)
  step('the connection shows on the canvas', await until(async () => (await kit.getByTestId(`design-proto-connection-${HOME}-0`).count()) === 1))
  const handle = kit.getByTestId(`design-proto-handle-${HOME}`)
  await until(async () => (await handle.count()) === 1)
  const hb = await handle.boundingBox()
  const area = (await designKit(() => window.designKit.inspect)).area
  const dark = (await query('node', { id: DARK })).absoluteBounds
  const target = await toPage(dark.x - area.x + dark.width / 2, dark.y - area.y + dark.height / 2)
  await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2)
  await page.mouse.down()
  for (let i = 1; i <= 10; i += 1) await page.mouse.move(hb.x + (target.x - hb.x) * (i / 10), hb.y + (target.y - hb.y) * (i / 10))
  await page.waitForTimeout(400)
  await page.mouse.move(target.x + 2, target.y + 2)
  await page.waitForTimeout(400)
  await page.mouse.up()
  step('dragging the handle onto a frame adds a navigation to it', await until(async () => (await interactions(HOME)).some((it) => it.actions[0]?.dest === DARK)))
  step('and opens its editor', await until(async () => (await kit.getByTestId('design-proto-editor').count()) === 1))
  await kit.getByTestId('design-proto-editor-close').click()
  const count = (await interactions(HOME)).length
  await kit.getByTestId(`design-proto-interaction-open-${count - 1}`).click()
  await until(async () => (await kit.getByTestId('design-proto-editor').count()) === 1)
  await page.keyboard.press('Delete')
  step('Delete removes the open interaction', await until(async () => (await interactions(HOME)).length === count - 1))

  // The preview pane and the presenter.
  await run('proto.interaction.set', { ids: [HOME], index: 0, trigger: { type: 'onClick' }, transition: { type: 'instant' } })
  await kit.getByTestId('design-proto-flow-preview').click()
  step('Preview this flow opens the preview pane', await until(async () => (await kit.getByTestId('design-proto-pane-frame').textContent()).includes('Home — Light')))
  await kit.getByTestId('design-proto-pane-present').click()
  step('Present opens the presenter', await until(async () => (await kit.getByTestId('design-proto-present-frame').textContent()).includes('Home — Light')))
  const screen = kit.getByTestId('design-proto-present-player-screen')
  await until(async () => (await screen.count()) === 1)
  const sb = await screen.boundingBox()
  await page.mouse.click(sb.x + sb.width / 2, sb.y + sb.height / 2)
  step('a click plays the interaction (navigates)', await until(async () => (await kit.getByTestId('design-proto-present-frame').textContent()).includes('Trail — Light')))
  await kit.getByTestId('design-proto-present-restart').click()
  step('Restart goes back to the start', await until(async () => (await kit.getByTestId('design-proto-present-frame').textContent()).includes('Home — Light')))
  // A press the browser cancels (pointercancel, lost capture) ends the engine's press and fires
  // nothing (review #17).
  const pressed = () => designKit(() => window.designKit.invoke('player', 'state', '{}').then((text) => JSON.parse(text).state?.pressed))
  const sbNow = await screen.boundingBox()
  await page.mouse.move(sbNow.x + sbNow.width / 2, sbNow.y + sbNow.height / 2)
  await page.mouse.down()
  const wasPressed = await until(async () => (await pressed()) === true, 30)
  await screen.dispatchEvent('pointercancel', { pointerId: 1, pointerType: 'mouse', isPrimary: true })
  step('a cancelled press is not left down in the player', wasPressed && (await until(async () => (await pressed()) === false, 30)), `${wasPressed} ${await pressed()}`)
  await page.mouse.up()
  await page.waitForTimeout(300)
  step('and its release does not click', (await kit.getByTestId('design-proto-present-frame').textContent()).includes('Home — Light'))
  await kit.getByTestId('design-proto-present-next').click()
  step('Next frame steps to the next frame', await until(async () => !(await kit.getByTestId('design-proto-present-frame').textContent()).includes('Home — Light')))
  await page.keyboard.press('Escape')
  step('Escape closes the presenter', await until(async () => (await kit.getByTestId('design-proto-present').count()) === 0))
  await kit.getByTestId('design-proto-pane-close').click()
  step('Close preview closes the pane', await until(async () => (await kit.getByTestId('design-proto-pane').count()) === 0))

  // Undo keeps working through the package's edits.
  const before = (await interactions(HOME)).length
  await kit.getByTestId('design-proto-interaction-remove-0').click()
  await until(async () => (await interactions(HOME)).length === before - 1)
  await kit.getByTestId('design-undo').click()
  step('undo brings a removed interaction back', await until(async () => (await interactions(HOME)).length === before))

  // The engine's player: hotspots, back, overlays, after delay, links, Present's bars.
  await playerSteps()

  await run('select.set', { ids: [] })
  await devSteps({ page, kit, designKit, step, until, toPage, canvas })
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
