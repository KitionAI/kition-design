// Text that types right (plan 2026-11-26 stage 2): node tests/hand/text.mjs [chromium|webkit]
// A new text made with the Text tool and real keys: typing, the arrows, typing over a selection,
// Cmd+A and Cmd+B, Cmd+C and Cmd+V, a triple-click and Cmd+Z, each read back from the engine.
// Walkthrough D6 (Cmd+A selected every layer), D7 (a triple-click selected nothing), D8 (Cmd+C /
// Cmd+V pasted a second text layer), D10 (Cmd+Z left an empty text layer), D12 (the layer stayed
// unnamed), and the keyboard left in the hidden text field after editing (T, R did nothing).
// Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-text', which)
const { page, step, until, part, finish, mod, inspect, node, toPage, viewAt, settle, undoStack } = hand

try {
  // An empty spot of the document, right of the sample's content, at 100 %.
  await viewAt(1, 6000, 4000)
  const at = await toPage(5800, 3900)
  await page.mouse.click(at.x + 300, at.y + 300)
  await settle()
  const characters = async (id) => (await node(id)).characters
  const range = async () => {
    const te = (await inspect()).textEdit
    return te ? [Math.min(te.anchor, te.caret), Math.max(te.anchor, te.caret)] : null
  }
  /** T, a click at `point`, then `typed`: the new text's id. */
  const newText = async (point, typed) => {
    await page.keyboard.press('t')
    await until(async () => (await inspect()).tool === 'text', 40)
    await page.mouse.click(point.x, point.y)
    await until(async () => (await inspect()).editing === 'text', 60)
    const id = (await inspect()).textEdit?.id
    if (id == null) throw new Error(`no text is being edited after T and a click: ${JSON.stringify(await inspect().then((i) => ({ tool: i.tool, editing: i.editing, selection: i.selection })))}`)
    await page.keyboard.type(typed, { delay: 20 })
    await until(async () => (await characters(id)) === typed, 60)
    return id
  }

  let text = null
  const layersBefore = (await inspect()).layers
  await part('typing', async () => {
    text = await newText(at, 'Hello world')
    step('typing on the canvas writes the text', (await characters(text)) === 'Hello world', await characters(text))
    step('D12 the new text is named after what it says', (await node(text)).name === 'Hello world', JSON.stringify((await node(text)).name))
    for (let i = 0; i < 5; i += 1) await page.keyboard.press('ArrowLeft')
    step('the arrows move the caret', await until(async () => JSON.stringify(await range()) === '[6,6]', 40), JSON.stringify(await range()))
    await page.keyboard.press('Shift+End')
    await until(async () => JSON.stringify(await range()) === '[6,11]', 40)
    await page.keyboard.type('there')
    step('typing replaces the selected characters', await until(async () => (await characters(text)) === 'Hello there', 40), await characters(text))
  })

  await part('D6', async () => {
    await page.keyboard.press(`${mod}+a`)
    step('D6 Cmd+A while typing a new text selects its characters', await until(async () => JSON.stringify(await range()) === '[0,11]', 40), JSON.stringify(await range()))
    const selection = (await inspect()).selection
    step('D6 Cmd+A keeps the layer selection to the text', JSON.stringify(selection) === JSON.stringify([text]), JSON.stringify(selection))
    await page.keyboard.press(`${mod}+b`)
    await page.waitForTimeout(400)
    await settle()
    const weight = async () => (await node(text)).fontStyle
    step('D6 Cmd+B then bolds the text', await until(async () => /bold/i.test(String(await weight())), 40), String(await weight()))
    step('D6 Cmd+B keeps the layer selection to the text', JSON.stringify((await inspect()).selection) === JSON.stringify([text]) && (await inspect()).editing === 'text', JSON.stringify((await inspect()).selection))
  })

  await part('D8', async () => {
    const layers = (await inspect()).layers
    await page.keyboard.press('End')
    for (let i = 0; i < 5; i += 1) await page.keyboard.press('Shift+ArrowLeft')
    await until(async () => JSON.stringify(await range()) === '[6,11]', 40)
    await page.keyboard.press(`${mod}+c`)
    await page.keyboard.press('End')
    await page.keyboard.press(`${mod}+v`)
    step('D8 Cmd+C, End, Cmd+V types the copied word', await until(async () => (await characters(text)) === 'Hello therethere', 40), await characters(text))
    await settle()
    step('D8 the paste adds no layer', (await inspect()).layers === layers, `${(await inspect()).layers} vs ${layers}`)
    for (let i = 0; i < 5; i += 1) await page.keyboard.press('Shift+ArrowLeft')
    await until(async () => JSON.stringify(await range()) === '[11,16]', 40)
    await page.keyboard.press(`${mod}+x`)
    step('D8 Cmd+X cuts the selected characters', await until(async () => (await characters(text)) === 'Hello there', 40), await characters(text))
    step('D8 the cut keeps the layer', (await inspect()).layers === layers && (await inspect()).editing === 'text', `${(await inspect()).layers} vs ${layers}`)
  })

  await part('D7', async () => {
    await page.keyboard.press('End')
    await page.keyboard.press('Enter')
    await page.keyboard.type('second line')
    await until(async () => (await characters(text)) === 'Hello there\nsecond line', 40)
    const b = (await node(text)).absoluteBounds
    const first = await toPage(b.x + 12, b.y + b.height / 4)
    await page.mouse.click(first.x, first.y, { clickCount: 3 })
    step('D7 a triple-click selects the paragraph under it', await until(async () => JSON.stringify(await range()) === '[0,11]', 40), JSON.stringify(await range()))
    const second = await toPage(b.x + 12, b.y + (b.height * 3) / 4)
    await page.mouse.click(second.x, second.y, { clickCount: 3 })
    step('D7 a triple-click on the second line selects that one', await until(async () => JSON.stringify(await range()) === '[12,23]', 40), JSON.stringify(await range()))
  })

  await part('D10', async () => {
    await page.keyboard.press('Escape')
    await until(async () => (await inspect()).editing === 'none', 40)
    // The keyboard is the editor's again: a tool's letter picks the tool.
    await page.keyboard.press('r')
    step('after Escape ends the editing, R picks the Rectangle tool', await until(async () => (await inspect()).tool === 'rectangle', 40), (await inspect()).tool)
    await page.keyboard.press('Escape')
    await until(async () => (await inspect()).tool === 'move', 40)
    const undo = (await undoStack()).length
    await page.keyboard.press(`${mod}+z`)
    step('D10 one Cmd+Z takes the new text away', await until(async () => (await inspect()).layers === layersBefore, 60), `${(await inspect()).layers} vs ${layersBefore}`)
    step('D10 and its one undo step', (await undoStack()).length === undo - 1, `${(await undoStack()).length} vs ${undo - 1}`)
    // A second new text, typed and left, then undone: nothing of it stays.
    const spot = await toPage(5800, 4100)
    const again = await newText(spot, 'Hello')
    await page.keyboard.press('Escape')
    await until(async () => (await inspect()).editing === 'none', 40)
    step('D10 the text is there after Escape', (await characters(again)) === 'Hello')
    await page.keyboard.press(`${mod}+z`)
    step('D10 T, a click, Hello, Escape, Cmd+Z leaves no layer', await until(async () => (await inspect()).layers === layersBefore, 60), `${(await inspect()).layers} vs ${layersBefore}`)
    step('D10 the undo stack is back where it was', (await undoStack()).length === undo - 1, `${(await undoStack()).length} vs ${undo - 1}`)
    await page.keyboard.press('t')
    step('after the undo, T picks the Text tool again', await until(async () => (await inspect()).tool === 'text', 40), (await inspect()).tool)
    await page.keyboard.press('Escape')
  })
  await finish()
} catch (error) {
  await finish(error)
}
