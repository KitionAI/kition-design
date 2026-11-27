// Chinese input (plan 2026-11-26 stage 6): node tests/hand/ime.mjs [chromium|webkit]
// A text made with the Text tool and real keys. The hidden field the input method types into sits
// at the caret, so its candidate window opens there (both browsers). In Chromium the input method
// itself is driven through the DevTools protocol (`Input.imeSetComposition`, `Input.insertText`):
// "n", "ni", "ni hao" show in the text at the caret, underlined, while the document stays as it
// was; Escape, Enter, the arrows and Backspace pressed while composing are the input method's (the
// composition is cancelled, the text stays in editing, nothing moves or goes: Backspace used to
// delete the text layer); the finished "你好" is the text, and one undo takes the whole new text
// away. Walkthrough D9 (nothing showed while composing). Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-ime', which)
const { page, kit, step, until, part, finish, inspect, query, node, toPage, viewAt, settle, undoStack } = hand

try {
  // An empty spot of the document, right of the sample's content, at 100 %.
  await viewAt(1, 6000, 4000)
  const empty = await toPage(5800, 3900)
  await page.mouse.click(empty.x + 300, empty.y + 300)
  await settle()
  /** The caret's top and bottom on the browser page, and the hidden field's box. */
  const caretOnPage = async () => {
    const caret = await query('textCaret')
    if (!caret) return null
    const box = await kit.getByTestId('kit-canvas-surface').boundingBox()
    const [v] = await hand.designKit(() => [window.designKit.canvas.current.view()])
    return { x: box.x + (caret.x0 - v.x) * v.zoom, top: box.y + (caret.y0 - v.y) * v.zoom, bottom: box.y + (caret.y1 - v.y) * v.zoom }
  }
  const field = () => kit.locator('textarea.kui-text-sink').boundingBox()
  const nearCaret = async () => {
    const caret = await caretOnPage()
    const box = await field()
    return { ok: !!caret && !!box && Math.abs(box.x - caret.x) <= 4 && Math.abs(box.y - caret.top) <= 4, caret, box }
  }
  /** T and a click at `point`: the new text's id. */
  const newText = async (point) => {
    await page.keyboard.press('t')
    await until(async () => (await inspect()).tool === 'text', 40)
    await page.mouse.click(point.x, point.y)
    await until(async () => (await inspect()).editing === 'text', 60)
    const id = (await inspect()).textEdit?.id
    if (id == null) throw new Error('no text is being edited after T and a click')
    return id
  }

  await part('the field at the caret', async () => {
    const id = await newText(empty)
    await page.keyboard.type('Hello', { delay: 20 })
    await until(async () => (await node(id)).characters === 'Hello', 60)
    await settle()
    let near = await nearCaret()
    step('the input method’s field sits at the caret', await until(async () => (near = await nearCaret()).ok, 30), JSON.stringify(near))
    await page.keyboard.press('ArrowLeft')
    await page.keyboard.press('ArrowLeft')
    await settle()
    step('and follows it when it moves', await until(async () => (near = await nearCaret()).ok, 30), JSON.stringify(near))
    await page.keyboard.press('Escape')
    await page.keyboard.press('Escape')
    await until(async () => (await inspect()).editing === 'none', 40)
  })

  if (which === 'chromium') {
    const cdp = await page.context().newCDPSession(page)
    const compose = (text) => cdp.send('Input.imeSetComposition', { text, selectionStart: text.length, selectionEnd: text.length })
    /** A key as the browser sends it while an input method composes (key code 229). */
    const composingKey = async (key, code) => {
      await cdp.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key, code, windowsVirtualKeyCode: 229, nativeVirtualKeyCode: 229 })
      await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, windowsVirtualKeyCode: 229, nativeVirtualKeyCode: 229 })
    }
    const preedit = async () => (await query('textSelection').catch(() => null))?.preedit ?? null

    await part('D9', async () => {
      const layers = (await inspect()).layers
      const undo = (await undoStack()).length
      const at = { x: empty.x, y: empty.y + 120 }
      const id = await newText(at)
      await settle()
      const start = await caretOnPage()
      for (const typed of ['n', 'ni', 'ni hao']) {
        await compose(typed)
        const shown = await until(async () => (await preedit())?.text === typed && (await preedit())?.cursor === typed.length, 40)
        step(`D9 composing “${typed}” shows it in the text`, shown, JSON.stringify(await preedit()))
      }
      step('D9 the document does not change while composing', (await node(id)).characters === '' && (await undoStack()).length === undo, JSON.stringify({ characters: (await node(id)).characters, undo: (await undoStack()).length }))
      await page.mouse.move(at.x + 400, at.y + 200)
      await settle(500)
      // Where the composition shows (compared below with the same place without it).
      const clip = { x: Math.round(start.x) + 2, y: Math.round(start.top), width: 80, height: Math.round(start.bottom - start.top) + 4 }
      const composing = await page.screenshot({ clip })
      let near = await nearCaret()
      step('D9 the input method’s field is at the caret while composing', await until(async () => (near = await nearCaret()).ok, 30), JSON.stringify(near))
      // Escape and Enter while composing are the input method's: they never end the editing.
      await composingKey('Escape', 'Escape')
      await compose('')
      await settle()
      step('D9 Escape while composing cancels the composition', (await preedit()) === null && (await node(id)).characters === '', JSON.stringify({ preedit: await preedit(), characters: (await node(id)).characters }))
      step('D9 and the text is still being edited', (await inspect()).editing === 'text', (await inspect()).editing)
      // The composition on the canvas: what differs from the same place without it is letters and
      // a line under them.
      await settle(500)
      const without = await page.screenshot({ clip })
      const ink = await page.evaluate(async ([a, b]) => {
        const pixels = async (data) => {
          const image = new Image()
          image.src = `data:image/png;base64,${data}`
          await image.decode()
          const c = document.createElement('canvas')
          c.width = image.width
          c.height = image.height
          const g = c.getContext('2d')
          g.drawImage(image, 0, 0)
          return { width: c.width, height: c.height, data: g.getImageData(0, 0, c.width, c.height).data }
        }
        const [one, two] = [await pixels(a), await pixels(b)]
        let total = 0
        let longest = 0
        for (let y = 0; y < one.height; y += 1) {
          let run = 0
          for (let x = 0; x < one.width; x += 1) {
            const i = (y * one.width + x) * 4
            const differs = Math.abs(one.data[i] - two.data[i]) + Math.abs(one.data[i + 1] - two.data[i + 1]) + Math.abs(one.data[i + 2] - two.data[i + 2]) > 120
            if (differs) {
              total += 1
              run += 1
              longest = Math.max(longest, run)
            } else run = 0
          }
        }
        return { total, longest }
      }, [composing.toString('base64'), without.toString('base64')])
      step('D9 the composition was on the canvas, underlined', ink.total > 40 && ink.longest >= 20, JSON.stringify(ink))
      await compose('ni hao')
      await until(async () => (await preedit())?.text === 'ni hao', 40)
      await composingKey('Enter', 'Enter')
      await settle()
      step('D9 Enter while composing types no line break', (await node(id)).characters === '' && (await inspect()).editing === 'text', JSON.stringify({ characters: (await node(id)).characters, editing: (await inspect()).editing }))
      const placed = JSON.stringify((await node(id)).absoluteBounds)
      await composingKey('ArrowLeft', 'ArrowLeft')
      await composingKey('Backspace', 'Backspace')
      await settle()
      const after = await node(id).catch(() => null)
      step('D9 the arrows and Backspace while composing neither move nor delete the text', !!after && JSON.stringify(after.absoluteBounds) === placed && (await inspect()).editing === 'text' && (await inspect()).layers === layers + 1, JSON.stringify({ bounds: after?.absoluteBounds, was: placed, editing: (await inspect()).editing, layers: (await inspect()).layers }))
      await cdp.send('Input.insertText', { text: '你好' })
      const typed = await until(async () => (await node(id)).characters === '你好', 40)
      step('D9 the finished text is typed', typed && (await preedit()) === null, JSON.stringify({ characters: (await node(id)).characters, preedit: await preedit() }))
      await page.keyboard.press('Escape')
      await until(async () => (await inspect()).editing === 'none', 40)
      await settle()
      step('D9 the new text is one undo step', (await undoStack()).length === undo + 1, JSON.stringify((await undoStack()).slice(0, 3)))
      await page.keyboard.press(`${hand.mod}+z`)
      step('D9 and one undo takes it away', await until(async () => (await inspect()).layers === layers, 40), `${(await inspect()).layers} layers (was ${layers})`)
    })
  }
  await finish()
} catch (error) {
  await finish(error)
}
