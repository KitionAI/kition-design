// One whole task by hand (plan 2026-11-26 stage 11): node tests/hand/login.mjs [chromium|webkit]
// A 375 x 812 phone login screen, the way a designer makes it with the mouse and keys and nothing
// else: a frame with vertical auto layout (padding 24, gap 16), a bold 28 px title, an Input
// component placed twice with its text overridden (Email, Password), a Button component with
// Default and Pressed variants placed once, everything centred, exported as a PNG. The walkthrough's
// task (its probe 12) counted 131 actions and got stuck 9 times; here each action is the direct
// one, with no way round: an action that does not do what it should counts as stuck and fails.
// Actions are counted as the walkthrough did: a key, a click, a drag, a typed value, the view set
// once; a panel field is two (the click, the value with Enter). The run ends with the screen's
// layers and the exported file, and fails above 110 actions or with any time stuck. With
// HAND_SHOTS=<folder> the finished screen is saved there. Prints PASS or FAIL per step.
import { mkdirSync } from 'node:fs'
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-login', which)
const { page, kit, step, until, finish, mod, inspect, query, node, toPage, viewAt, settle } = hand
const shots = process.env.HAND_SHOTS ?? null
const T = (id) => kit.getByTestId(id)

let actions = 0
const stuck = []
/** An action that did not do what it should: counted, and the run fails. */
const expect = (what, ok, detail = '') => {
  if (!ok) stuck.push(what)
  step(what, ok, detail)
}
const selection = async () => (await inspect()).selection
const selected = async (id) => (await selection()).map(String).includes(String(id))
const box = async (id) => {
  const b = (await node(id)).absoluteBounds
  return { x: b.x, y: b.y, w: b.width, h: b.height }
}
const kids = async (id) => (await query('node', { id, depth: 1 })).children ?? []
const parentOf = (n) => String(n?.parent && typeof n.parent === 'object' ? n.parent.id : n?.parent)

// The actions.
const key = async (k, wait = 250) => {
  actions += 1
  await page.keyboard.press(k)
  await settle(wait)
}
const type = async (text) => {
  actions += 1
  await page.keyboard.type(text, { delay: 15 })
  await settle(200)
}
const clickAt = async (at, held = []) => {
  actions += 1
  for (const k of held) await page.keyboard.down(k)
  await page.mouse.move(at.x - 3, at.y - 2)
  await page.mouse.move(at.x, at.y)
  await page.mouse.click(at.x, at.y)
  for (const k of held) await page.keyboard.up(k)
  await settle(250)
}
const click = async (x, y, held) => clickAt(await toPage(x, y), held)
const dbl = async (x, y) => {
  actions += 1
  const at = await toPage(x, y)
  await page.mouse.move(at.x, at.y)
  await page.mouse.dblclick(at.x, at.y)
  await settle(450)
}
const dragAt = async (a, b, { held = [], steps = 16 } = {}) => {
  actions += 1
  for (const k of held) await page.keyboard.down(k)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(a.x + ((b.x - a.x) * i) / steps, a.y + ((b.y - a.y) * i) / steps)
    await page.waitForTimeout(16)
  }
  await page.waitForTimeout(80)
  await page.mouse.up()
  for (const k of held) await page.keyboard.up(k)
  await settle(400)
}
const drag = async (from, to, options) => dragAt(await toPage(from.x, from.y), await toPage(to.x, to.y), options)
const press = async (locator) => {
  actions += 1
  await locator.click()
  await settle(300)
}
/** A panel field: the click (it selects its text), the value with Enter. */
const field = async (testId, value) => {
  actions += 2
  const el = T(testId)
  await ((await el.locator('input').count()) ? el.locator('input').first() : el).click()
  await page.keyboard.type(String(value), { delay: 10 })
  await page.keyboard.press('Enter')
  await settle(300)
}
/** Where a frame's name label is, on the page (it sits just above the frame's top-left). */
const label = async (id) => {
  const b = await box(id)
  const at = await toPage(b.x + 14, b.y)
  return { x: at.x, y: at.y - 9 }
}
const menuItem = (text) => kit.locator('.design-menu').getByText(text, { exact: true }).first()
/** The text layer in `parent` (its first one). */
const textIn = async (parent) => (await kids(parent)).find((k) => k.type === 'TEXT')?.id

try {
  // 1. A page of its own, the view at 75 %.
  const before = (await inspect()).pages.length
  await press(T('design-pages-add'))
  await key('Enter') // a new page opens its name for typing; Enter keeps "Page N"
  expect('a new page', (await inspect()).pages.length === before + 1)
  actions += 1
  await viewAt(0.75, 420, 420)

  // 2. The screen: F and a drag, then Cmd+R to name it.
  await key('f')
  await drag({ x: 0, y: 0 }, { x: 375, y: 812 })
  const SCREEN = (await selection())[0]
  const screen = SCREEN !== undefined ? await node(SCREEN) : null
  expect('F and a drag make the 375 x 812 screen, selected', screen?.type === 'FRAME' && screen.width === 375 && screen.height === 812, JSON.stringify(screen && [screen.type, screen.width, screen.height]))
  const centre = await toPage(187, 406)
  expect('the screen stays where it was drawn', centre.x > hand.surface.x && centre.x < hand.surface.x + hand.surface.width && centre.y > hand.surface.y && centre.y < hand.surface.y + hand.surface.height)
  await key(`${mod}+r`)
  await type('Login')
  await key('Enter')
  expect('Cmd+R renames it', (await node(SCREEN)).name === 'Login', (await node(SCREEN)).name)

  // 3. Auto layout: Shift+A, vertical, padding 24, gap 16; the screen keeps its size.
  await key('Shift+a')
  expect('Shift+A gives the empty screen auto layout at its size', (await node(SCREEN)).layout?.mode !== undefined && (await node(SCREEN)).width === 375 && (await node(SCREEN)).height === 812, JSON.stringify([(await node(SCREEN)).layout, (await node(SCREEN)).width, (await node(SCREEN)).height]))
  await press(T('design-design-direction-vertical'))
  await field('design-design-paddingHorizontal', 24)
  await field('design-design-paddingVertical', 24)
  await field('design-design-gap', 16)
  const layout = (await node(SCREEN)).layout
  expect('vertical, padding 24, gap 16', layout?.mode === 'vertical' && JSON.stringify(layout.padding) === '[24,24,24,24]' && layout.gap === 16, JSON.stringify(layout))

  // 4. The title: T, a click in the screen, the words, Escape (the text stays selected); 28,
  // Cmd+B.
  await key('t')
  await click(100, 100)
  await type('Welcome back')
  await key('Escape')
  const TITLE = (await selection())[0]
  const title = TITLE !== undefined ? await node(TITLE) : null
  expect('the title is a text in the screen, still selected after Escape', title?.type === 'TEXT' && title.characters === 'Welcome back' && parentOf(title) === String(SCREEN), JSON.stringify(title && [title.type, title.characters, parentOf(title)]))
  expect('the new text is dark on the white screen', title?.fills?.[0]?.color !== '#FFFFFF', JSON.stringify(title?.fills))
  await field('design-font-size', 28)
  expect('the keys are the canvas\'s after the font size', await selected(TITLE))
  await key(`${mod}+b`)
  expect('28 px, bold', (await node(TITLE)).fontSize === 28 && /Bold/.test((await node(TITLE)).fontStyle ?? ''), JSON.stringify([(await node(TITLE)).fontSize, (await node(TITLE)).fontStyle]))

  // 5. The Input component, built beside the screen: a 327 x 48 frame with a stroke, radius 8
  // and a grey placeholder; Shift+A keeps its size; padding 16, centred left; Cmd+R; Alt+Cmd+K.
  await key('Escape')
  await key('f')
  await drag({ x: 480, y: 0 }, { x: 807, y: 48 })
  const INPUT = (await selection())[0]
  expect('the input frame is 327 x 48', (await node(INPUT)).type === 'FRAME' && (await node(INPUT)).width === 327 && (await node(INPUT)).height === 48, JSON.stringify(await box(INPUT)))
  await press(T('design-stroke-add'))
  await field('design-design-radius', 8)
  await key('t')
  await click(500, 20)
  await type('Placeholder')
  await key('Escape')
  const PH = (await selection())[0]
  expect('the placeholder is a text in the input, selected', (await node(PH)).characters === 'Placeholder' && parentOf(await node(PH)) === String(INPUT))
  await field('design-fill-0-hex', '888888')
  await clickAt(await label(INPUT))
  expect('the input\'s label selects it', await selected(INPUT), JSON.stringify(await selection()))
  await key('Shift+a')
  const inn = await node(INPUT)
  expect('Shift+A keeps the stroked input 327 x 48', inn.layout?.mode === 'horizontal' && inn.width === 327 && inn.height === 48, JSON.stringify([inn.layout, inn.width, inn.height]))
  await field('design-design-paddingHorizontal', 16)
  await press(T('design-design-align-cell-0-1'))
  await key(`${mod}+r`)
  await type('Input')
  await key('Enter')
  await key(`${mod}+Alt+k`)
  expect('Alt+Cmd+K makes it the Input component', (await node(INPUT)).type === 'COMPONENT' && (await node(INPUT)).name === 'Input', JSON.stringify([(await node(INPUT)).type, (await node(INPUT)).name]))

  // 6. Two instances into the screen by Alt+drag from the component's label; their texts are
  // overridden (a double-click reaches the text, Cmd+A, the words, Escape); each fills the width.
  const instances = []
  for (const y of [200, 300]) {
    await dragAt(await label(INPUT), await toPage(190, y), { held: ['Alt'], steps: 24 })
    const id = (await selection())[0]
    const n = id !== undefined ? await node(id) : null
    expect(`Alt+drag of the component drops an instance into the screen (${instances.length + 1})`, n?.type === 'INSTANCE' && parentOf(n) === String(SCREEN), JSON.stringify(n && [n.type, parentOf(n)]))
    instances.push(id)
  }
  for (const [i, words] of [[0, 'Email'], [1, 'Password']]) {
    const b = await box(instances[i])
    await key('Escape')
    await dbl(b.x + 60, b.y + b.h / 2)
    if ((await inspect()).editing !== 'text') await dbl(b.x + 60, b.y + b.h / 2)
    expect(`a double-click reaches the text in instance ${i + 1}`, (await inspect()).editing === 'text', (await inspect()).editing)
    await key(`${mod}+a`)
    await type(words)
    await key('Escape')
    const inner = await textIn(instances[i])
    expect(`instance ${i + 1} says ${words}, the component still Placeholder`, inner !== undefined && (await node(inner)).characters === words && (await node(PH)).characters === 'Placeholder', inner ? (await node(inner)).characters : '')
  }
  for (const id of instances) {
    await key('Escape')
    const b = await box(id)
    await click(b.x + b.w - 20, b.y + b.h / 2)
    expect('a click on the instance selects it', await selected(id), JSON.stringify(await selection()))
    await press(T('design-design-sizingH'))
    await press(menuItem('Fill container'))
  }
  expect('both instances fill the width', (await Promise.all(instances.map(node))).every((n) => n.sizing?.horizontal === 'fill'))

  // 7. The Button component: a blue 327 x 48 frame, radius 8, "Sign in"; Shift+A, centred;
  // Cmd+R; Alt+Cmd+K; Add variant; the second variant darker; the property named State, the
  // second value Pressed. One instance into the screen by Alt+drag, filling the width.
  await click(900, 900)
  await key('f')
  await drag({ x: 480, y: 200 }, { x: 807, y: 248 })
  const BTN = (await selection())[0]
  await field('design-fill-0-hex', '2563EB')
  await field('design-design-radius', 8)
  await key('t')
  await click(600, 220)
  await type('Sign in')
  await key('Escape')
  const BL = (await selection())[0]
  expect('the button\'s label is a text in it', (await node(BL)).characters === 'Sign in' && parentOf(await node(BL)) === String(BTN))
  await clickAt(await label(BTN))
  expect('the button\'s label selects it', await selected(BTN), JSON.stringify(await selection()))
  await key('Shift+a')
  expect('Shift+A keeps the button 327 x 48', (await node(BTN)).width === 327 && (await node(BTN)).height === 48, JSON.stringify(await box(BTN)))
  await press(T('design-design-align-cell-1-1'))
  await key(`${mod}+r`)
  await type('LoginButton')
  await key('Enter')
  await key(`${mod}+Alt+k`)
  await press(T('design-design-add-variant'))
  const SET = parentOf(await node(BTN))
  const variants = await kids(SET)
  expect('Add variant makes a set of two', variants.length === 2, JSON.stringify(variants.map((v) => v.name)))
  const PRESSED = variants.find((v) => String(v.id) !== String(BTN))?.id
  expect('the new variant is selected', PRESSED !== undefined && (await selected(PRESSED)), JSON.stringify(await selection()))
  await field('design-fill-0-hex', '1D4ED8')
  await clickAt(await label(SET))
  expect('the set\'s label selects it', await selected(SET), JSON.stringify(await selection()))
  actions += 2
  await T('design-design-properties').locator('input').first().click()
  await page.keyboard.type('State', { delay: 10 })
  await page.keyboard.press('Enter')
  await settle(300)
  const chips = await T('design-design-properties').locator('[data-testid*="-value-"]').evaluateAll((els) => els.map((e) => e.getAttribute('data-testid')))
  actions += 2
  await T(chips[1]).dblclick()
  await settle(300)
  await page.keyboard.press(`${mod}+a`)
  await page.keyboard.type('Pressed', { delay: 10 })
  await page.keyboard.press('Enter')
  await settle(300)
  const names = (await kids(SET)).map((v) => v.name).sort()
  expect('the variants are State=Default and State=Pressed', JSON.stringify(names) === JSON.stringify(['State=Default', 'State=Pressed']), JSON.stringify(names))
  await key('Escape')
  const l = await box(BL)
  await dragAt(await toPage(l.x + l.w / 2, l.y + l.h / 2), await toPage(190, 420), { held: ['Alt'], steps: 24 })
  const BUTTON = (await selection())[0]
  const bn = BUTTON !== undefined ? await node(BUTTON) : null
  expect('Alt+drag of the Default variant drops an instance into the screen', bn?.type === 'INSTANCE' && parentOf(bn) === String(SCREEN), JSON.stringify(bn && [bn.type, bn.name, parentOf(bn)]))
  await press(T('design-design-sizingH'))
  await press(menuItem('Fill container'))

  // 8. Everything centred: the screen by its label, the middle cell of the alignment grid.
  await click(900, 900)
  await clickAt(await label(SCREEN))
  expect('the screen\'s label selects it', await selected(SCREEN), JSON.stringify(await selection()))
  await press(T('design-design-align-cell-1-1'))
  const away = await toPage(900, 900)
  await page.mouse.move(away.x, away.y)
  if (shots) {
    mkdirSync(shots, { recursive: true })
    await page.screenshot({ path: `${shots}/login-${which}.png` })
  }

  // 9. Export: the Export section's + and Export.
  const files = await page.evaluate(() => [...window.devHost.files.keys()])
  await press(T('design-design-export-add'))
  await press(T('design-design-export-run'))
  await until(async () => (await page.evaluate(() => [...window.devHost.files.keys()])).length > files.length, 60)

  // What was made.
  const final = await node(SCREEN)
  const children = await kids(SCREEN)
  const shape = await Promise.all(children.map(async (k) => {
    const n = await node(k.id)
    const text = n.type === 'INSTANCE' ? await textIn(k.id) : null
    return { type: n.type, name: n.name, text: n.type === 'TEXT' ? n.characters : text ? (await node(text)).characters : null, fill: n.sizing?.horizontal ?? null }
  }))
  step('the screen: 375 x 812, vertical, padding 24, gap 16, centred', final.width === 375 && final.height === 812 && final.layout?.mode === 'vertical' && final.layout?.gap === 16 && JSON.stringify(final.layout?.padding) === '[24,24,24,24]' && final.layout?.mainAlign === 'center' && final.layout?.crossAlign === 'center', JSON.stringify([final.width, final.height, final.layout]))
  const wanted = [
    ['TEXT', 'Welcome back', 'hug'],
    ['INSTANCE', 'Email', 'fill'],
    ['INSTANCE', 'Password', 'fill'],
    ['INSTANCE', 'Sign in', 'fill'],
  ]
  // In the auto layout's order, top to bottom (the query lists children bottom-most first or not:
  // either way the order is the flow's).
  const seq = shape.map((s) => [s.type, s.text, s.fill])
  const ordered = JSON.stringify(seq) === JSON.stringify(wanted) || JSON.stringify([...seq].reverse()) === JSON.stringify(wanted)
  step('its layers: the title (its own width), Email, Password, Sign in, the instances filling the width', ordered, JSON.stringify(shape))
  const written = await page.evaluate((files) => {
    const out = []
    for (const [path, file] of window.devHost.files) {
      if (files.includes(path)) continue
      const bytes = new Uint8Array(file.bytes)
      const view = new DataView(file.bytes)
      out.push({ path, png: bytes[0] === 0x89 && bytes[1] === 0x50, w: bytes.length > 24 ? view.getUint32(16) : 0, h: bytes.length > 24 ? view.getUint32(20) : 0 })
    }
    return out
  }, files)
  step('Export writes Login.png, 375 x 812, through the host', written.length === 1 && written[0].path.endsWith('Login.png') && written[0].png && written[0].w === 375 && written[0].h === 812, JSON.stringify(written))
  console.log(`LOGIN ${which}: ${actions} actions, stuck ${stuck.length} times${stuck.length ? `: ${stuck.join('; ')}` : ''}`)
  step('at most 110 actions', actions <= 110, `${actions}`)
  step('stuck no time', stuck.length === 0, stuck.join('; '))
} catch (error) {
  await finish(error)
}
await finish()
