// Saved versions through the host's checkpoints, in a real browser: node tests/versions.mjs [chromium|webkit]
// The development host keeps the replica and its checkpoints (`?checkpoints=1`, Kit UI protocol
// section 7m). Saves two versions, reloads the document (both are still listed: nothing about them
// is in the file or the replica), renames, deletes by identity (two versions with one title),
// restores one (the content is what it was) and undoes the restore. A file opened without its
// history lists its versions as unavailable.
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
const shots = evidence(page, 'versions', which)
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
const until = async (check, tries = 100) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(100)
  }
  return false
}

const PATH = 'Sample.kdesign'

try {
  await page.goto(`${origin}/dev.html?locale=en-US&checkpoints=1`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = () => page.frames().find((f) => f !== page.mainFrame())
  const designKit = (body, arg) => frame().evaluate(body, arg)
  const ready = () => until(async () => (await designKit(() => Boolean(window.designKit?.canvas.current?.view()))) === true, 300)
  await ready()
  const run = (command, params = {}) => designKit(([c, p]) => window.designKit.run(c, p), [command, params])
  /** The names of the first page's top-level layers. */
  const layers = async () => (await designKit(() => window.designKit.query('document', { depth: 1 }))).pages[0].children.map((child) => child.name)
  const kept = () => page.evaluate((path) => window.devHost.history.checkpoints(path), PATH)
  const labelled = async () => (await kept()).filter((c) => c.label)
  const openPanel = async () => {
    await run('file.versionHistory', {})
    return until(async () => (await kit.getByTestId('design-version-history').count()) === 1)
  }
  const save = async (title) => {
    await kit.getByTestId('design-version-save-toggle').click()
    await kit.getByTestId('design-version-save-form-title').fill(title)
    const before = (await labelled()).length
    await kit.getByTestId('design-version-save-form-submit').click()
    return until(async () => (await labelled()).length === before + 1)
  }
  const rowName = (index) => kit.getByTestId(`design-version-${index}`).locator('.design-version-name').textContent()
  const menu = async (index, item) => {
    await kit.getByTestId(`design-version-${index}`).hover()
    await kit.getByTestId(`design-version-more-${index}`).click()
    await kit.getByTestId(item).click()
  }

  step('the host keeps versions: the panel can save them', (await openPanel()) && (await kit.getByTestId('design-version-unavailable').count()) === 0)

  // Two versions, each of the design as it is then.
  await run('shape.rectangle', { x: -3000, y: -3000, width: 40, height: 40, name: 'Version A' })
  const first = await layers()
  step('saves a first version (the design saved, the host records it)', await save('First'))
  await run('shape.rectangle', { x: -2900, y: -3000, width: 40, height: 40, name: 'Version B' })
  const second = await layers()
  step('saves a second version', await save('Second'))
  const recorded = await labelled()
  step('both are the kit\'s, newest first, at the replica versions saved', recorded.length === 2 && recorded[0].label === 'Second' && recorded[1].label === 'First' && recorded.every((c) => c.origin === 'kit' && c.available && c.version), JSON.stringify(recorded))
  const names = await page.evaluate((path) => {
    // The file's members (a zip's central directory): nothing about versions.
    const data = new Uint8Array(window.devHost.files.get(path).bytes)
    const out = []
    for (let i = 0; i + 46 <= data.length; i += 1) {
      if (data[i] === 0x50 && data[i + 1] === 0x4b && data[i + 2] === 0x01 && data[i + 3] === 0x02) {
        const n = data[i + 28] | (data[i + 29] << 8)
        out.push(new TextDecoder().decode(data.slice(i + 46, i + 46 + n)))
      }
    }
    return out
  }, PATH)
  step('the file holds the content and its images, nothing about versions or a thumbnail', names.length > 0 && names.every((n) => n.startsWith('kdm/') || n.startsWith('images/')), names.join(', '))

  // Reloaded: the versions are the host's, so both are listed again.
  await page.evaluate(() => window.devHost.reload())
  await ready()
  await until(async () => JSON.stringify(await layers()) === JSON.stringify(second))
  await openPanel()
  step('after a reload both versions are listed', await until(async () => (await rowName(0)) === 'Second' && (await rowName(1)) === 'First'))

  // Rename: the title and the description go to the host.
  await menu(1, 'design-version-menu-rename')
  await kit.getByTestId('design-version-rename-form-title').fill('Kickoff')
  await kit.getByTestId('design-version-rename-form-description').fill('one box')
  await kit.getByTestId('design-version-rename-form-submit').click()
  step('renames a version with the host', await until(async () => (await labelled()).some((c) => c.id === recorded[1].id && c.label === 'Kickoff' && c.description === 'one box')))
  step('and the row shows it', await until(async () => (await rowName(1)) === 'Kickoff'))

  // Delete reads the version's identity, not its title: with both titled the same, deleting the
  // newer row deletes that one only.
  await menu(1, 'design-version-menu-rename')
  await kit.getByTestId('design-version-rename-form-title').fill('Same')
  await kit.getByTestId('design-version-rename-form-submit').click()
  await menu(0, 'design-version-menu-rename')
  await kit.getByTestId('design-version-rename-form-title').fill('Same')
  await kit.getByTestId('design-version-rename-form-submit').click()
  await until(async () => (await labelled()).every((c) => c.label === 'Same'))
  await menu(0, 'design-version-menu-delete')
  await kit.getByTestId('design-version-delete-confirm').click()
  await until(async () => (await labelled()).length === 1)
  const left = await labelled()
  step('deleting a row deletes that version by its id, not by its title', left.length === 1 && left[0].id === recorded[1].id, JSON.stringify(left))
  step('the panel lists the one left', await until(async () => (await kit.getByTestId('design-version-1').count()) === 0 && (await rowName(0)) === 'Same'))

  // Preview, then restore: the content is the version's; undo takes the restore back.
  await kit.getByTestId('design-version-0').click()
  step('a row previews its version', await until(async () => (await kit.getByTestId('design-version-banner').count()) === 1 && JSON.stringify(await layers()) === JSON.stringify(first)))
  await kit.getByTestId('design-version-back').click()
  await until(async () => (await kit.getByTestId('design-version-banner').count()) === 0)
  step('back to the current version', JSON.stringify(await layers()) === JSON.stringify(second))
  await menu(0, 'design-version-menu-restore')
  step('restoring gives the design the content it had at that version', await until(async () => JSON.stringify(await layers()) === JSON.stringify(first)), JSON.stringify(await layers()))
  await kit.getByTestId('design-undo').click()
  step('undo takes the restore back', await until(async () => JSON.stringify(await layers()) === JSON.stringify(second)), JSON.stringify(await layers()))
  await kit.getByTestId('design-redo').click()
  step('and redo restores it again', await until(async () => JSON.stringify(await layers()) === JSON.stringify(first)))

  // The file opened without its history: the version stays listed, unavailable, with the reason.
  await page.evaluate((path) => window.devHost.open(path, window.devHost.files.get(path).bytes.slice(0)), PATH)
  await ready()
  await openPanel()
  step('a version the history no longer reaches is listed as unavailable', await until(async () => (await kit.getByTestId('design-version-unavailable-0').count()) === 1))
  await kit.getByTestId('design-version-0').click()
  await page.waitForTimeout(300)
  step('and is not shown', (await kit.getByTestId('design-version-banner').count()) === 0)
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
