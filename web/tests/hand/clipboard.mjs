// The system clipboard and files dropped on the canvas (plan 2026-11-26 stage 7):
// node tests/hand/clipboard.mjs [chromium|webkit]
// Pastes are the browser's paste events, with what another app put on the clipboard: an image, SVG
// text, plain text (walkthrough L5: Cmd+V pasted the layers copied in the editor instead). A copy
// of layers puts the editor's own mark on the clipboard, and its paste is the layers again, in
// place or in the middle of the view when the original is out of it (L1, reference R-L5, R-L6).
// While a text is edited, Copy puts the selected characters on the clipboard and Paste types the
// clipboard's text (D8). Files dropped on the canvas are imported where they land (L4); a dropped
// document is not opened here. Local variables' Import asks the host for the file. In Chromium,
// with the clipboard allowed, real Cmd+C and Cmd+V go through the system clipboard: the copy is
// the layers' SVG for other apps, text another app copied pastes as a text layer, or into the
// text being edited. Prints PASS or FAIL per step.
import { deflateSync } from 'node:zlib'
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-clipboard', which)
const { page, kit, frame, designKit, step, until, part, finish, mod, surface, inspect, run, node, toPage, viewAt, settle } = hand

/** A `width` x `height` PNG of one colour. */
function png(width, height) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    return c >>> 0
  })
  const crc = (bytes) => {
    let c = 0xffffffff
    for (const byte of bytes) c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8)
    return (c ^ 0xffffffff) >>> 0
  }
  const chunk = (type, data) => {
    const head = Buffer.alloc(8)
    head.writeUInt32BE(data.length, 0)
    head.write(type, 4, 'latin1')
    const tail = Buffer.alloc(4)
    tail.writeUInt32BE(crc(Buffer.concat([head.subarray(4), data])), 0)
    return Buffer.concat([head, data, tail])
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0)
  header.writeUInt32BE(height, 4)
  header.set([8, 2, 0, 0, 0], 8)
  const rows = Buffer.alloc((width * 3 + 1) * height)
  for (let y = 0; y < height; y += 1) for (let x = 0; x < width; x += 1) rows.set([220, 40, 90], y * (width * 3 + 1) + 1 + x * 3)
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', header), chunk('IDAT', deflateSync(rows)), chunk('IEND', Buffer.alloc(0))])
}
const PNG = png(40, 30).toString('base64')
const SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40"><rect width="60" height="40" fill="#3366ff"/></svg>'

/**
 * Sends the focused element a paste event carrying `text`, `html` and a PNG, as the browser does
 * for Cmd+V with that on the system clipboard: whether the editor took it.
 */
const paste = ({ text = null, html = null, image = null } = {}) =>
  designKit(
    ({ text, html, image }) => {
      const data = new DataTransfer()
      if (text != null) data.setData('text/plain', text)
      if (html != null) data.setData('text/html', html)
      if (image) data.items.add(new File([Uint8Array.from(atob(image), (c) => c.charCodeAt(0))], 'image.png', { type: 'image/png' }))
      const event = new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true })
      ;(document.activeElement ?? document.body).dispatchEvent(event)
      return event.defaultPrevented
    },
    { text, html, image },
  )
/** Sends the focused element a copy (or cut) event: what the editor put on the clipboard. */
const copyEvent = (type = 'copy') =>
  designKit((type) => {
    const data = new DataTransfer()
    const event = new ClipboardEvent(type, { clipboardData: data, bubbles: true, cancelable: true })
    ;(document.activeElement ?? document.body).dispatchEvent(event)
    return { prevented: event.defaultPrevented, text: data.getData('text/plain') }
  }, type)

const frameBox = await page.locator('#kit').boundingBox()
/** Sends drag event `type` carrying `files` at page point `at` to what is there: whether it was taken. */
const dragEvent = (type, files, at) =>
  designKit(
    ({ type, files, x, y }) => {
      const data = new DataTransfer()
      for (const file of files) data.items.add(new File([Uint8Array.from(atob(file.data), (c) => c.charCodeAt(0))], file.name, { type: file.type }))
      const event = new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: data, clientX: x, clientY: y, relatedTarget: null })
      document.elementFromPoint(x, y).dispatchEvent(event)
      return event.defaultPrevented
    },
    { type, files, x: at.x - frameBox.x, y: at.y - frameBox.y },
  )
/** Drags files over the canvas and drops them at page point `at`: whether the drag and the drop were taken. */
const drop = async (files, at) => {
  await dragEvent('dragenter', files, at)
  const over = await dragEvent('dragover', files, at)
  const dropped = await dragEvent('drop', files, at)
  return { over, dropped }
}

try {
  const near = (a, b, tolerance = 2) => Math.abs(a - b) <= tolerance
  const box = async (id) => (await node(id)).absoluteBounds
  const selected = async () => (await inspect()).selection
  /** The view's centre as a document point. */
  const centre = async () => {
    const [v, area] = await designKit(() => [window.designKit.canvas.current.view(), window.designKit.inspect.area])
    return { x: area.x + v.x + v.width / v.zoom / 2, y: area.y + v.y + v.height / v.zoom / 2 }
  }
  /** A real click on document point (`x`, `y`). */
  const click = async (x, y) => {
    const at = await toPage(x, y)
    await page.mouse.click(at.x, at.y)
    await settle(250)
  }
  /** Waits for the layer count to reach `count`: the layer selected then. */
  const added = async (count) => {
    await until(async () => (await inspect()).layers === count, 60)
    await settle()
    return (await selected())[0] ?? null
  }

  // A page of its own, at 100 %, with one rectangle drawn on it.
  let rect = null
  await part('setup', async () => {
    const pages = (await inspect()).pages.length
    await kit.getByTestId('design-pages-add').click()
    await until(async () => (await inspect()).pages.length === pages + 1 && (await inspect()).layers === 0, 60)
    await settle()
    await viewAt(1, 300, 200)
    await click(500, 380)
    await page.keyboard.press('r')
    await until(async () => (await inspect()).tool === 'rectangle', 40)
    const from = await toPage(100, 100)
    const to = await toPage(200, 180)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(to.x, to.y, { steps: 8 })
    await page.mouse.up()
    await until(async () => (await inspect()).tool === 'move' && (await selected()).length === 1, 40)
    rect = (await selected())[0]
    step('setup a rectangle drawn', near((await box(rect)).width, 100), JSON.stringify(await box(rect)))
  })

  await part('L5 paste from another app', async () => {
    await click(500, 380)
    // An image: a new image layer in the middle of the view.
    let layers = (await inspect()).layers
    let middle = await centre()
    step('L5 the editor takes a paste event', await paste({ image: PNG, html: '<img src="x">' }))
    let id = await added(layers + 1)
    let b = id == null ? null : await box(id)
    const image = id == null ? null : await node(id)
    step('L5 a pasted image is a new image layer', image?.fills?.[0]?.type === 'image' && (await inspect()).layers === layers + 1, JSON.stringify({ layers: (await inspect()).layers, fills: image?.fills?.map((f) => f.type) }))
    step('L5 the image keeps its size and sits in the middle of the view', !!b && near(b.width, 40) && near(b.height, 30) && near(b.x + b.width / 2, middle.x) && near(b.y + b.height / 2, middle.y), JSON.stringify({ b, middle }))
    step('L5 the pasted image is named so', image?.name === 'Pasted image', JSON.stringify(image?.name))
    // SVG text: an editable frame, in the middle of the view.
    layers = (await inspect()).layers
    await viewAt(1, 700, 200)
    middle = await centre()
    await paste({ text: SVG })
    id = await added(layers + 1)
    const svg = id == null ? null : await node(id)
    b = svg?.absoluteBounds
    step('L5 pasted SVG text becomes an editable frame', svg?.type === 'FRAME' && (await inspect()).layers >= layers + 1, JSON.stringify({ type: svg?.type, layers: (await inspect()).layers }))
    step('L5 the SVG sits in the middle of the view', !!b && near(b.width, 60) && near(b.x + b.width / 2, middle.x, 3) && near(b.y + b.height / 2, middle.y, 3), JSON.stringify({ b, middle }))
    // Plain text: a text layer saying it.
    await click(900, 380)
    layers = (await inspect()).layers
    const words = 'Copied in another app'
    await paste({ text: words })
    id = await added(layers + 1)
    const text = id == null ? null : await node(id)
    step('L5 pasted text becomes a text layer', text?.type === 'TEXT' && text.characters === words, JSON.stringify({ type: text?.type, characters: text?.characters }))
    // Each paste is one undo step.
    await page.keyboard.press(`${mod}+z`)
    step('L5 one undo takes a paste away', await until(async () => (await inspect()).layers === layers, 40), `${(await inspect()).layers} vs ${layers}`)
  })

  let mark = null
  await part('L1 the editor\'s own copy', async () => {
    await viewAt(1, 300, 200)
    const original = await box(rect)
    await click(150, 140)
    step('L1 the rectangle is selected', JSON.stringify(await selected()) === JSON.stringify([rect]), JSON.stringify(await selected()))
    const copied = await copyEvent('copy')
    mark = copied.text
    step('L1 a copy of layers puts the editor\'s mark on the clipboard', copied.prevented && copied.text.startsWith('kition-design://clipboard/'), JSON.stringify(copied))
    let layers = (await inspect()).layers
    await paste({ text: mark })
    let id = await added(layers + 1)
    let b = id == null ? null : await box(id)
    step('L1 its paste is the layers again, in place', id !== rect && !!b && near(b.x, original.x) && near(b.y, original.y) && near(b.width, original.width), JSON.stringify({ b, original }))
    // The original out of the view: the copy lands in its middle.
    await viewAt(1, 3000, 2000)
    const middle = await centre()
    layers = (await inspect()).layers
    await click(3000, 2000)
    await paste({ text: mark })
    id = await added(layers + 1)
    b = id == null ? null : await box(id)
    step('L1 with the original out of view it lands in the middle of the view', !!b && near(b.x + b.width / 2, middle.x) && near(b.y + b.height / 2, middle.y), JSON.stringify({ b, middle }))
    // A cut: the layer goes, its paste brings it back.
    layers = (await inspect()).layers
    const cut = await copyEvent('cut')
    step('L1 a cut puts the mark on the clipboard and removes the layer', cut.text.startsWith('kition-design://clipboard/') && (await until(async () => (await inspect()).layers === layers - 1, 40)), JSON.stringify({ cut, layers: (await inspect()).layers }))
    await paste({ text: cut.text })
    step('L1 the cut layer pastes back', await until(async () => (await inspect()).layers === layers, 40), `${(await inspect()).layers} vs ${layers}`)
    // Nothing selected: a copy leaves the clipboard as it is.
    await click(3200, 2200)
    await until(async () => (await selected()).length === 0, 40)
    const none = await copyEvent('copy')
    step('L1 a copy with nothing selected leaves the clipboard alone', !none.prevented && none.text === '', JSON.stringify(none))
  })

  await part('D8 the clipboard while a text is edited', async () => {
    await viewAt(1, 300, 600)
    const at = await toPage(100, 600)
    await page.keyboard.press('t')
    await until(async () => (await inspect()).tool === 'text', 40)
    await page.mouse.click(at.x, at.y)
    await until(async () => (await inspect()).editing === 'text', 60)
    const id = (await inspect()).textEdit?.id
    await page.keyboard.type('Hello world', { delay: 20 })
    await until(async () => (await node(id)).characters === 'Hello world', 60)
    for (let i = 0; i < 5; i += 1) await page.keyboard.press('Shift+ArrowLeft')
    await until(async () => {
      const te = (await inspect()).textEdit
      return te && Math.min(te.anchor, te.caret) === 6 && Math.max(te.anchor, te.caret) === 11
    }, 40)
    await settle(400)
    const layers = (await inspect()).layers
    const copied = await copyEvent('copy')
    step('D8 Copy in a text puts the selected characters on the clipboard', copied.prevented && copied.text === 'world', JSON.stringify(copied))
    await page.keyboard.press('End')
    await settle()
    await paste({ text: copied.text })
    step('D8 its paste types them at the caret', await until(async () => (await node(id)).characters === 'Hello worldworld', 40), (await node(id)).characters)
    await paste({ text: 'XYZ' })
    step('D8 text another app copied types into the text', await until(async () => (await node(id)).characters === 'Hello worldworldXYZ', 40), (await node(id)).characters)
    await paste({ text: mark })
    await settle(400)
    step('D8 copied layers paste nothing into a text, and no layer', (await node(id)).characters === 'Hello worldworldXYZ' && (await inspect()).layers === layers && (await inspect()).editing === 'text', JSON.stringify({ characters: (await node(id)).characters, layers: (await inspect()).layers }))
    await page.keyboard.press('Escape')
    await until(async () => (await inspect()).editing === 'none', 40)
  })

  await part('L4 files dropped on the canvas', async () => {
    await viewAt(1, 300, 200)
    await click(500, 380)
    let layers = (await inspect()).layers
    const point = await toPage(420, 120)
    const files = [{ name: 'dots.png', type: 'image/png', data: PNG }]
    await dragEvent('dragenter', files, point)
    const over = await dragEvent('dragover', files, point)
    const hint = kit.getByTestId('design-drop-hint')
    const shown = await until(async () => ((await hint.textContent().catch(() => '')) ?? '') === 'Drop to import', 30)
    step('L4 a file dragged over the canvas is taken, with a hint over the canvas', over && shown, JSON.stringify({ over, hint: await hint.textContent().catch(() => null) }))
    await dragEvent('dragleave', files, point)
    step('L4 the hint goes when the drag leaves', await until(async () => (await hint.count()) === 0, 30))
    const dropped = await drop(files, point)
    step('L4 the drop is taken', dropped.over && dropped.dropped, JSON.stringify(dropped))
    let id = await added(layers + 1)
    const where = await hand.toDoc(point.x, point.y)
    let b = id == null ? null : await box(id)
    step('L4 a dropped image is a new layer whose top-left is where it landed', !!b && near(b.x, where.x) && near(b.y, where.y) && near(b.width, 40) && (await node(id)).fills?.[0]?.type === 'image', JSON.stringify({ b, where }))
    step('L4 the hint goes after the drop', await until(async () => (await kit.getByTestId('design-drop-hint').count()) === 0, 30))
    layers = (await inspect()).layers
    const svgPoint = await toPage(420, 300)
    await drop([{ name: 'shape.svg', type: 'image/svg+xml', data: Buffer.from(SVG).toString('base64') }], svgPoint)
    id = await added(layers + 1)
    const svgWhere = await hand.toDoc(svgPoint.x, svgPoint.y)
    b = id == null ? null : await box(id)
    step('L4 a dropped SVG is a frame whose top-left is where it landed', !!b && (await node(id)).type === 'FRAME' && near(b.x, svgWhere.x) && near(b.y, svgWhere.y), JSON.stringify({ b, svgWhere }))
    layers = (await inspect()).layers
    await drop([{ name: 'Other.kdesign', type: '', data: Buffer.from('not opened here').toString('base64') }], await toPage(600, 300))
    await settle(400)
    const toast = await kit.getByTestId('design-toast').textContent().catch(() => '')
    step('L4 a dropped document is not imported; the editor says where it opens', (await inspect()).layers === layers && /Other\.kdesign/.test(toast ?? ''), JSON.stringify({ layers: (await inspect()).layers, toast }))
    // A file dropped beside the canvas does not take the editor's page away.
    const panel = await kit.getByTestId('design-pages-add').boundingBox()
    const aside = await dragEvent('drop', [{ name: 'note.txt', type: 'text/plain', data: Buffer.from('x').toString('base64') }], { x: panel.x + 2, y: panel.y + 2 })
    step('L4 a file dropped beside the canvas is not opened by the browser', aside)
  })

  await part('files through the host', async () => {
    // Local variables' Import asks the host for the file (no file field of the editor's own).
    const exported = await run('variable.export', { format: 'json' })
    const text = typeof exported === 'object' && exported ? exported.text : JSON.parse(exported).text
    await page.evaluate((text) => {
      window.devHost.files.set('tokens.json', { bytes: new TextEncoder().encode(text).buffer, version: 1 })
      window.devHost.scriptHostPick('tokens.json')
    }, text)
    const picks = await page.evaluate(() => window.devHost.hostPicks.length)
    await kit.getByTestId('design-rail-variables').click()
    await until(async () => (await kit.getByTestId('design-variables').count()) === 1)
    await kit.getByTestId('design-variables-import').click()
    await until(async () => /Imported/.test((await kit.getByTestId('design-toast').textContent().catch(() => '')) ?? ''), 40)
    const asked = await page.evaluate(() => window.devHost.hostPicks.slice(-1)[0] ?? null)
    step('Local variables\' Import asks the host for a JSON file', (await page.evaluate(() => window.devHost.hostPicks.length)) === picks + 1 && JSON.stringify(asked?.accept ?? []).includes('json'), JSON.stringify(asked))
    step('the file the host gave is imported', /Imported/.test((await kit.getByTestId('design-toast').textContent().catch(() => '')) ?? ''))
    await kit.getByTestId('design-variables-close').click()
    await until(async () => (await kit.getByTestId('design-variables').count()) === 0)
    step('the editor has no file field of its own', (await designKit(() => document.querySelectorAll('input[type="file"]').length)) === 0)
  })

  await part('real keys', async () => {
    await viewAt(1, 300, 200)
    await click(150, 140)
    await until(async () => JSON.stringify(await selected()) === JSON.stringify([rect]), 40)
    let layers = (await inspect()).layers
    await page.keyboard.press(`${mod}+c`)
    await settle()
    await page.keyboard.press(`${mod}+v`)
    let id = await added(layers + 1)
    const original = await box(rect)
    const b = id == null ? null : await box(id)
    step('L1 real Cmd+C, Cmd+V pastes a copy in place', id !== rect && !!b && near(b.x, original.x) && near(b.y, original.y), JSON.stringify({ b, original, layers: (await inspect()).layers }))
    if (which !== 'chromium') return
    // The system clipboard itself: Chromium, with the clipboard allowed.
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write'])
    await click(150, 140)
    await page.keyboard.press(`${mod}+c`)
    const read = () => frame().evaluate(() => navigator.clipboard.readText()).catch((error) => `error ${error}`)
    step('L2 Cmd+C puts the layers\' SVG on the system clipboard', await until(async () => (await read()).includes('<svg'), 40), (await read()).slice(0, 80))
    layers = (await inspect()).layers
    await page.keyboard.press(`${mod}+v`)
    id = await added(layers + 1)
    step('L1 and Cmd+V pastes the layers again (the SVG is the editor\'s own)', (await inspect()).layers === layers + 1 && (await node(id)).type === 'RECTANGLE', JSON.stringify({ layers: (await inspect()).layers, type: id == null ? null : (await node(id)).type }))
    await frame().evaluate(() => navigator.clipboard.writeText('Written elsewhere'))
    await click(700, 380)
    layers = (await inspect()).layers
    await page.keyboard.press(`${mod}+v`)
    id = await added(layers + 1)
    const text = id == null ? null : await node(id)
    step('L5 Cmd+V pastes text another app copied as a text layer', text?.type === 'TEXT' && text.characters === 'Written elsewhere', JSON.stringify({ type: text?.type, characters: text?.characters }))
    // While a text is edited: Cmd+C puts the characters on the system clipboard, Cmd+V types its text.
    await page.keyboard.press('Enter')
    await until(async () => (await inspect()).editing === 'text', 40)
    await page.keyboard.press('End')
    for (let i = 0; i < 9; i += 1) await page.keyboard.press('Shift+ArrowLeft')
    await settle(400)
    await page.keyboard.press(`${mod}+c`)
    step('D8 Cmd+C in a text puts the characters on the system clipboard', await until(async () => (await read()) === 'elsewhere', 40), await read())
    await frame().evaluate(() => navigator.clipboard.writeText(' and pasted'))
    await page.keyboard.press('End')
    await page.keyboard.press(`${mod}+v`)
    step('D8 Cmd+V in a text types the system clipboard\'s text', await until(async () => (await node(id)).characters === 'Written elsewhere and pasted', 40), (await node(id)).characters)
    await page.keyboard.press('Escape')
  })
  await finish()
} catch (error) {
  await finish(error)
}
