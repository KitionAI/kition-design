// The keys of Windows and Linux (plan 2026-11-26 stage 11, regressions/issue-hand-other-platform-keys):
// node tests/hand/other-platform-keys.mjs [chromium|webkit]
// The page reports a Linux platform, as Windows and Linux browsers do, on any machine. Every
// binding whose first key there is not the Mac's (kit-editor's frame binds the Mac's, from the
// engine's catalog) is pressed with real keys on three selected rectangles, and the command must
// reach the engine exactly once: Alt+Shift+H and Alt+Shift+V distribute (Ctrl+Alt+H on a Mac),
// Ctrl+Alt+Shift+T tidies up, Ctrl+Alt+P pixel preview, Ctrl+Alt+M mask, Ctrl+Shift+] and [ to
// the front and back, Ctrl+Alt+Shift+A suggest auto layout; Ctrl+D and Ctrl+G, which the frame
// binds, still run once. Before the fix none of the first eight did anything off the Mac.
// Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

const linux = () => {
  Object.defineProperty(Navigator.prototype, 'platform', { get: () => 'Linux x86_64' })
}
// In the kit's frame: the commands the page sends the engine's worker.
const sniff = () => {
  if (window === window.top) return
  const sent = (window.__commands = [])
  const post = Worker.prototype.postMessage
  Worker.prototype.postMessage = function (data, transfer) {
    if (data?.request?.command) sent.push(data.request.command)
    return post.call(this, data, transfer)
  }
}
const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-other-platform-keys', which, { init: `(${linux})();(${sniff})()` })
const { page, designKit, step, part, finish, inspect, run, settle, surface } = hand

try {
  await part('keys', async () => {
    step('the page reports Linux', (await designKit(() => navigator.platform)) === 'Linux x86_64')
    const made = async (answer) => answer?.id ?? (await inspect()).selection[0]
    const ids = []
    for (const [x, width] of [[40, 60], [200, 60], [420, 80]]) ids.push(await made(await run('shape.rectangle', { x, y: 1000, width, height: 40 })))
    const keys = [
      ['distribute.horizontal', 'Alt+Shift+h'],
      ['distribute.vertical', 'Alt+Shift+v'],
      ['arrange.tidy', 'Control+Alt+Shift+t'],
      ['view.pixelPreview', 'Control+Alt+p'],
      ['object.mask', 'Control+Alt+m'],
      ['arrange.bringToFront', 'Control+Shift+BracketRight'],
      ['arrange.sendToBack', 'Control+Shift+BracketLeft'],
      ['layout.suggest', 'Control+Alt+Shift+a'],
      ['edit.duplicate', 'Control+d'],
      ['arrange.group', 'Control+g'],
    ]
    for (const [id, key] of keys) {
      // The keyboard on the canvas, the three rectangles selected.
      await page.mouse.click(surface.x + surface.width - 30, surface.y + 30)
      await run('select.set', { ids })
      await settle(300)
      await designKit(() => (window.__commands.length = 0))
      await page.keyboard.press(key)
      await settle(500)
      const sent = await designKit(() => window.__commands.slice())
      step(`${key} runs ${id} once`, sent.filter((c) => c === id).length === 1, JSON.stringify(sent))
      if (sent.length && (await inspect()).canUndo && sent.includes(id) && id !== 'view.pixelPreview') await run('edit.undo', {})
      if (id === 'view.pixelPreview' && sent.includes(id)) await run('view.pixelPreview', {})
      await settle(200)
    }
  })
} catch (error) {
  await finish(error)
}
await finish()
