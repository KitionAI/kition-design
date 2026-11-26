// An agent's draft and a restored version in the editor (Kit UI protocol sections 7d and 7m),
// through the development host with the replica and checkpoints (`?replica=1&checkpoints=1`):
// node tests/drafts.mjs [chromium|webkit]
// a. The person adds a layer; the agent's draft adds another; the comparison lists the draft's
//    layer and changes nothing in the document; the person adds a layer meanwhile; the person
//    accepts the draft: the Layers panel shows all three.
// b. One Undo (the editor's button) takes back the draft's layer only; Redo brings it back.
// d. A version saved with a label in the Version history panel; a layer added after it; the host
//    restores the version: the design is the version's; one Undo brings the later layer back.
import assert from 'node:assert/strict'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { evidence } from './evidence.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
const shots = evidence(page, 'drafts', which)
const kit = page.frameLocator('#kit')
const errors = []
page.on('pageerror', (error) => errors.push(error.message))
const PATH = 'Sample.kdesign'
const frame = () => page.frames().find((f) => f !== page.mainFrame())
const designKit = (body, arg) => frame().evaluate(body, arg)
const until = async (check, message) => {
  for (let n = 0; n < 200; n += 1) {
    if (await check().catch(() => false)) return
    await page.waitForTimeout(100)
  }
  assert.fail(message)
}
/** An agent's step through the host (`ai/run`), on the document or (`draft`) on the agent's draft. */
const agent = (command, params, draft) =>
  page.evaluate(async ({ command, params, draft }) => (await window.devHost.connection().ai.run([{ command, params }], false, draft ? { draft: true } : {})).completed[0].result, { command, params, draft })
/** The names of the first page's top-level layers, as the editor reads them. */
const layers = async () => (await designKit(() => window.designKit.query('document', { depth: 1 }))).pages[0].children.map((child) => child.name)
/** The text of the Layers panel. */
const panel = () => kit.getByTestId('design-layers').textContent()
/** Whether the editor shows exactly `want` among `names` (the Layers panel and its document). */
const shows = async (names, want) => {
  const [now, text] = [await layers(), await panel()]
  return names.every((name) => now.includes(name) === want.includes(name) && text.includes(name) === want.includes(name))
}
/** The person draws a rectangle named `name` (the editor's own command, as its tool runs it). */
const draw = async (name, x) => {
  await designKit(([name, x]) => window.designKit.run('shape.rectangle', { x, y: -3000, width: 40, height: 40, props: { name } }), [name, x])
  await until(async () => (await panel()).includes(name), `the person's ${name} shows in the Layers panel`)
}
const button = async (id) => {
  await until(() => kit.getByTestId(id).isEnabled(), `${id} is enabled`)
  await kit.getByTestId(id).click()
}
const NAMES = ['Person A', 'Person B', 'From the draft', 'After the version']
let failures = 0

try {
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US&replica=1&checkpoints=1`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  await until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true, 'the canvas is up')

  // a. The person's edit, then the agent's draft of another.
  await draw('Person A', -3000)
  await agent('shape.rectangle', { x: -2900, y: -3000, width: 40, height: 40, props: { name: 'From the draft' } }, true)
  const drafted = JSON.stringify(await page.evaluate(() => window.devHost.connection().ai.inspect({ draft: true })))
  assert.ok((await page.evaluate(() => window.devHost.connection().draft.state())).state !== 'none', 'the draft is open')
  const before = await layers()
  const compared = await page.evaluate(() => window.devHost.connection().draft.compare({ maxSize: 256 }))
  assert.ok(compared.summary.created.length >= 1, `the preview lists the draft's layer: ${JSON.stringify(compared.summary).slice(0, 400)} (draft ${drafted.slice(0, 200)})`)
  assert.deepEqual(await layers(), before, 'comparing changes nothing in the document')
  assert.ok(await shows(NAMES, ['Person A']), 'the editor does not show the draft\'s layer')
  await draw('Person B', -2800)
  const merged = await page.evaluate(() => window.devHost.connection().draft.merge({ kind: 'agent', principal: 'agent:test' }))
  assert.equal(merged.merged, true, 'the draft merged')
  await until(() => shows(NAMES, ['Person A', 'Person B', 'From the draft']), `the editor shows both the person's layers and the draft's (${await layers()})`)
  console.log(`PASS ${which}: a draft accepted after the person's edits keeps them and adds its own`)

  // b. One Undo takes the draft back, and only the draft; Redo brings it back.
  await button('design-undo')
  await until(() => shows(NAMES, ['Person A', 'Person B']), `one Undo takes back the draft's layer only (${await layers()})`)
  await button('design-redo')
  await until(() => shows(NAMES, ['Person A', 'Person B', 'From the draft']), `Redo brings the draft's layer back (${await layers()})`)
  console.log(`PASS ${which}: one Undo takes back the merged draft only, Redo brings it back`)

  // d. A version saved with a label; a later edit; the host restores the version; Undo.
  const labelled = async () => (await page.evaluate((path) => window.devHost.history.checkpoints(path), PATH)).filter((c) => c.label)
  await designKit(() => window.designKit.run('file.versionHistory', {}))
  await until(async () => (await kit.getByTestId('design-version-history').count()) === 1, 'the Version history panel opens')
  await kit.getByTestId('design-version-save-toggle').click()
  await kit.getByTestId('design-version-save-form-title').fill('With the draft')
  await kit.getByTestId('design-version-save-form-submit').click()
  await until(async () => (await labelled()).some((c) => c.label === 'With the draft'), 'the labelled version is recorded by the host')
  const version = (await labelled()).find((c) => c.label === 'With the draft')
  await draw('After the version', -2700)
  await page.evaluate((id) => window.devHost.restoreCheckpoint(id), version.id)
  await until(() => shows(NAMES, ['Person A', 'Person B', 'From the draft']), `the restore shows the version's content (${await layers()})`)
  await button('design-undo')
  await until(() => shows(NAMES, ['Person A', 'Person B', 'From the draft', 'After the version']), `one Undo brings the later edit back (${await layers()})`)
  console.log(`PASS ${which}: a restored version is one step of the person's Undo`)
  assert.deepEqual(errors, [], 'no page errors')
} catch (error) {
  failures += 1
  shots.failed('error')
  console.log(`FAIL ${which}: ${error instanceof Error ? error.message.split('\n').slice(0, 6).join(' / ') : String(error)}`)
} finally {
  if (errors.length) console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
  await shots.settled()
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
