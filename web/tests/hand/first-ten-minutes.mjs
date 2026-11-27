// A designer's first ten minutes (plan 2026-11-26 stage 11): node tests/hand/first-ten-minutes.mjs [chromium|webkit]
// The thirty things a designer does first, and six more, each done the way the acceptance list
// says (walkthrough gap-reference, section N): real pointer and keys, counted. Each thing is one
// part, `N<n>`, whose steps are asserted on what the engine reads back (`inspect`, `query`), and
// whose actions are counted against the list's count. Setup that is not the thing being done (a
// page of its own, layers made by commands, the view put somewhere) is not counted. The new file
// (N1) and saving (N30) are the host's: here dev.html's (Cmd+N reaches the host, a new page is the
// blank canvas; Cmd+S writes through the host). With HAND_SHOTS=<folder> a screenshot of each thing
// is saved there. Prints PASS or FAIL per step and one ITEM line per thing.
import { mkdirSync } from 'node:fs'
import { openHand } from './harness.mjs'

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-first-ten-minutes', which)
const { page, kit, designKit, step, until, finish, mod, inspect, run, query, node, view, toPage, toDoc, viewAt, settle, undoStack, surface } = hand
const mac = mod === 'Meta'
const shots = process.env.HAND_SHOTS ?? null
if (shots) mkdirSync(shots, { recursive: true })

const near = (a, b, within = 1) => Math.abs(a - b) <= within
const ids = (list) => JSON.stringify([...list].map(String).sort())
const selection = async () => (await inspect()).selection
const box = async (id) => {
  const n = await node(id)
  const b = n.absoluteBounds
  return { x: b.x, y: b.y, w: b.width, h: b.height, rotation: n.rotation ?? 0 }
}
const show = (b) => `${Math.round(b.x)},${Math.round(b.y)} ${Math.round(b.w)}x${Math.round(b.h)}${b.rotation ? ` ${Math.round(b.rotation)}deg` : ''}`
const parentOf = (n) => String(n?.parent && typeof n.parent === 'object' ? n.parent.id : n?.parent)
const children = async (id) => ((await query('node', { id, depth: 1 })).children ?? []).map((c) => c.id)
const overlays = async () => {
  const zoom = (await view()).zoom
  const state = await designKit((zoom) => window.designKit.editor.session.tool('toolState', [0, 0, '{}', zoom]), zoom)
  return state?.overlays ?? []
}

// One thing on the list: its actions are counted, its steps decide whether it passed.
const results = []
let current = null
const check = (name, ok, detail = '') => {
  if (current && !ok) current.failed.push(name)
  step(`N${current?.n ?? '?'} ${name}`, ok, detail)
}
/**
 * A step the editor is known not to do as the list says, for a reason written down for the owner
 * (acceptance.md): it marks the thing as not passed, and says so, without failing the run. If it
 * passes one day, the run says that too.
 */
const known = (name, ok, why, detail = '') => {
  if (ok) console.log(`NOTE N${current?.n ?? '?'} ${name}: now as the list says (${why} no longer holds)`)
  else {
    if (current) current.known.push(name)
    console.log(`KNOWN N${current?.n ?? '?'} ${name} - ${why}${detail ? ` (${detail})` : ''}`)
  }
}
const item = async (n, title, reference, body) => {
  current = { n, title, reference, acts: 0, failed: [], known: [] }
  try {
    await body()
  } catch (error) {
    check('ran to the end', false, error instanceof Error ? error.message.split('\n').slice(0, 4).join(' / ') : String(error))
  }
  if (shots) await page.screenshot({ path: `${shots}/n${String(n).padStart(2, '0')}-${which}.png` }).catch(() => undefined)
  results.push(current)
  const missed = [...current.failed, ...current.known]
  console.log(`ITEM N${n} ${current.failed.length ? 'FAIL' : current.known.length ? 'KNOWN' : 'PASS'} actions ${current.acts} (reference ${reference}) ${title}${missed.length ? ` - ${missed.join('; ')}` : ''}`)
  current = null
}
const counted = (by = 1) => {
  if (current) current.acts += by
}

// The actions, each counted once.
const key = async (k, wait = 250) => {
  counted()
  await page.keyboard.press(k)
  await settle(wait)
}
const type = async (text) => {
  counted()
  await page.keyboard.type(text, { delay: 15 })
  await settle(200)
}
/** A real click on document point (`x`, `y`), `held` keys down round it. */
const click = async (x, y, held = []) => {
  counted()
  const at = await toPage(x, y)
  for (const k of held) await page.keyboard.down(k)
  await page.mouse.click(at.x, at.y)
  for (const k of held) await page.keyboard.up(k)
  await settle(250)
}
const dbl = async (x, y, held = []) => {
  counted()
  const at = await toPage(x, y)
  for (const k of held) await page.keyboard.down(k)
  await page.mouse.dblclick(at.x, at.y)
  for (const k of held) await page.keyboard.up(k)
  await settle(400)
}
/** A real drag between page points, in `steps` moves, `held` keys down round it. */
const dragPx = async (a, b, { steps = 12, held = [], before = null } = {}) => {
  counted()
  for (const k of held) await page.keyboard.down(k)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(a.x + ((b.x - a.x) * i) / steps, a.y + ((b.y - a.y) * i) / steps)
    await page.waitForTimeout(16)
  }
  await page.waitForTimeout(80)
  if (before) await before()
  await page.mouse.up()
  for (const k of held) await page.keyboard.up(k)
  await settle(300)
}
const drag = async (from, to, options) => dragPx(await toPage(from.x, from.y), await toPage(to.x, to.y), options)
const press = async (locator) => {
  counted()
  await locator.click()
  await settle(300)
}
/** A panel field: a click (it selects its text), the value typed, Enter. */
const field = async (testId, value) => {
  const el = kit.getByTestId(testId)
  // A field, a box holding one, or a value that turns into a field when clicked (a fill's hex).
  const inner = await el.locator('input').count()
  await press(inner ? el.locator('input').first() : el)
  await type(String(value))
  await key('Enter', 300)
}

// Setup, not counted.
const quiet = async (fn) => {
  const saved = current
  current = null
  try {
    return await fn()
  } finally {
    current = saved
  }
}
const newPage = async (name) => {
  const added = await run('page.add', { name })
  const id = added?.id ?? (await inspect()).page
  await run('page.select', { id })
  await settle(400)
  return id
}
const made = async (answer) => answer?.id ?? (await selection())[0]
const rect = async (x, y, width, height, parent = undefined) => made(await run('shape.rectangle', { x, y, width, height, ...(parent !== undefined ? { parent } : {}) }))
const frame = async (x, y, width, height, name) => {
  const id = await made(await run('frame.add', { x, y, width, height, name }))
  await run('node.set', { id, props: { fill: '#ffffff' } })
  return id
}
const select = async (list) => {
  await run('select.set', { ids: list })
  await settle(250)
}
const deselect = async () => {
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  await select([])
}
const undo = async (times = 1) => {
  for (let i = 0; i < times; i += 1) await page.keyboard.press(`${mod}+z`)
  await settle(300)
}
/** The `inspect` bounds (`selectionBounds`, `contentBounds`; document-area coordinates) as a document box. */
const docBox = async (b) => {
  if (!b) return null
  const area = (await inspect()).area
  return { x: b.x + area.x, y: b.y + area.y, w: b.width, h: b.height }
}
/** Whether document box `b` is inside the view. */
const inView = async (b) => {
  const [v, area] = await designKit(() => [window.designKit.canvas.current.view(), window.designKit.inspect.area])
  const left = area.x + v.x
  const top = area.y + v.y
  return b.x >= left - 1 && b.y >= top - 1 && b.x + b.w <= left + v.width / v.zoom + 1 && b.y + b.h <= top + v.height / v.zoom + 1
}
const focused = () => designKit(() => {
  const el = document.activeElement
  return `${el?.tagName ?? ''} ${el?.closest('[data-testid]')?.getAttribute('data-testid') ?? ''}`
})

try {
  let A = null
  let screenFrame = null

  // 1. A new file: Cmd+N is the host's (dev.html shows the shortcut it was given); the blank canvas
  // is a new page here, with the Layers panel's hint.
  await item(1, 'New file', 1, async () => {
    await key(`${mod}+n`)
    const status = await page.locator('#status').innerText()
    check('Cmd+N goes to the host', /host shortcut/i.test(status) && /N$/i.test(status.trim()), status)
    const before = (await inspect()).pages.length
    await press(kit.getByTestId('design-pages-add'))
    await until(async () => (await inspect()).pages.length === before + 1, 40)
    // A new page starts with its name being edited; Enter keeps the name it was given.
    await key('Enter')
    const now = await inspect()
    const here = now.pages.find((p) => String(p.id) === String(now.page))
    check('a blank page, nothing on it', now.pages.length === before + 1 && here?.layers === 0, JSON.stringify(here))
    const hint = await kit.locator('.design-layers-empty').innerText().catch(() => '')
    check('the Layers panel says to press F or R', /\(F\)/.test(hint) && /\(R\)/.test(hint), hint)
    await quiet(() => viewAt(1, 300, 250))
  })

  // 3. F and a drag: a white frame named "Frame N", selected, back to Move.
  await item(3, 'Drag a custom frame', 2, async () => {
    await key('f')
    check('F picks the Frame tool', (await inspect()).tool === 'frame', (await inspect()).tool)
    await drag({ x: 0, y: 0 }, { x: 400, y: 300 })
    const [id] = await selection()
    const n = id !== undefined ? await node(id) : null
    A = id
    check('a 400 x 300 frame', n?.type === 'FRAME' && near(n.width, 400) && near(n.height, 300), JSON.stringify(n && [n.type, n.width, n.height]))
    check('named Frame N, white', /^Frame \d+$/.test(n?.name ?? '') && n?.fills?.[0]?.color === '#FFFFFF', JSON.stringify(n && [n.name, n.fills]))
    check('selected, back to Move', (await selection()).length === 1 && (await inspect()).tool === 'move', JSON.stringify([await selection(), (await inspect()).tool]))
  })

  // 2. F and a preset: the frame beside the one there, the view on it, Move, selected.
  await item(2, 'A phone frame from a preset', 2, async () => {
    await key('f')
    const preset = kit.getByTestId('design-design-frame-preset-iPhone 16')
    check('the Frame tool lists the presets', (await preset.count()) === 1)
    await press(preset)
    const [id] = await selection()
    const n = id !== undefined ? await node(id) : null
    screenFrame = id
    check('a 393 x 852 frame, selected', n?.type === 'FRAME' && near(n.width, 393) && near(n.height, 852) && String(id) !== String(A), JSON.stringify(n && [n.type, n.width, n.height]))
    const b = n ? await box(id) : null
    check('beside the frame already there', b !== null && b.x >= 400, b ? show(b) : '')
    check('the view shows it', b !== null && (await inView(b)), JSON.stringify(await view()))
    check('back to Move', (await inspect()).tool === 'move', (await inspect()).tool)
    await quiet(async () => {
      await deselect()
      await viewAt(1, 300, 250)
    })
  })

  // 4. R and a drag on the frame: its child, #D9D9D9, back to Move; a click gives 100 x 100.
  let R1 = null
  await item(4, 'Draw a rectangle', 2, async () => {
    await key('r')
    await drag({ x: 20, y: 20 }, { x: 120, y: 100 })
    const [id] = await selection()
    R1 = id
    const n = id !== undefined ? await node(id) : null
    check('a 100 x 80 rectangle in the frame', n?.type === 'RECTANGLE' && near(n.width, 100) && near(n.height, 80) && parentOf(n) === String(A), JSON.stringify(n && [n.type, n.width, n.height, parentOf(n)]))
    check('filled #D9D9D9', n?.fills?.[0]?.color === '#D9D9D9', JSON.stringify(n?.fills))
    check('back to Move', (await inspect()).tool === 'move', (await inspect()).tool)
    await quiet(async () => {
      await page.keyboard.press('r')
      await click(160, 20)
    })
    const [one] = await selection()
    const o = one !== undefined ? await node(one) : null
    check('a click with R gives 100 x 100', o?.type === 'RECTANGLE' && near(o.width, 100) && near(o.height, 100) && String(one) !== String(id), JSON.stringify(o && [o.type, o.width, o.height]))
    await quiet(() => run('edit.delete', {}))
  })

  // 5. O and a Shift+drag: a circle.
  await item(5, 'Draw a circle', 2, async () => {
    await key('o')
    await drag({ x: 160, y: 20 }, { x: 230, y: 60 }, { held: ['Shift'] })
    const [id] = await selection()
    const n = id !== undefined ? await node(id) : null
    check('an ellipse as wide as it is high', n?.type === 'ELLIPSE' && near(n.width, n.height, 0.5) && n.width > 30, JSON.stringify(n && [n.type, n.width, n.height]))
  })

  // 6. T, a click, typing, Escape: an auto-width text, still selected; T, a click and Escape with
  // nothing typed leave no layer.
  let TEXT = null
  await item(6, 'Write a line of text', 4, async () => {
    await key('t')
    await click(20, 200)
    await type('Hello')
    await key('Escape')
    const [id] = await selection()
    TEXT = id
    const n = id !== undefined ? await node(id) : null
    check('a text with what was typed, in the frame', n?.type === 'TEXT' && n.characters === 'Hello' && parentOf(n) === String(A), JSON.stringify(n && [n.type, n.characters, parentOf(n)]))
    check('Inter 12, auto width', /Inter/.test(JSON.stringify(n?.fontName ?? n?.fontFamily ?? '')) && n?.fontSize === 12 && /WIDTH_AND_HEIGHT|auto|width/i.test(String(n?.textAutoResize ?? n?.autoResize ?? n?.sizing?.text ?? '')), JSON.stringify(n && [n.fontName ?? n.fontFamily, n.fontSize, n.textAutoResize ?? n.autoResize]))
    check('Escape leaves it selected, the Move tool', (await inspect()).editing === 'none' && ids(await selection()) === ids([id]) && (await inspect()).tool === 'move', JSON.stringify([(await inspect()).editing, await selection(), (await inspect()).tool]))
    const layers = (await inspect()).layers
    await quiet(async () => {
      await page.keyboard.press('t')
      await click(250, 250)
      await page.keyboard.press('Escape')
      await settle(300)
    })
    check('T, a click and Escape with nothing typed leave no layer', (await inspect()).layers === layers, `${(await inspect()).layers - layers} more`)
  })

  // 7. A double-click on a text edits it with all of it selected; typing replaces it.
  await item(7, 'Change a text', 3, async () => {
    await quiet(deselect)
    const b = await box(TEXT)
    await dbl(b.x + b.w / 2, b.y + b.h / 2)
    const edit = (await inspect()).textEdit
    check('a double-click edits it, all selected', (await inspect()).editing === 'text' && edit !== null && Math.abs(edit.caret - edit.anchor) === 5, JSON.stringify([(await inspect()).editing, edit]))
    await type('World')
    await key('Escape')
    check('typing replaced the text', (await node(TEXT)).characters === 'World', (await node(TEXT)).characters)
  })

  // Moving and selecting: two rectangles on a page and a frame with a group in it.
  let M1 = null
  let M2 = null
  let F = null
  let G1 = null
  await quiet(async () => {
    await newPage('Move')
    M1 = await rect(100, 250, 100, 80)
    M2 = await rect(300, 100, 100, 80)
    F = await frame(500, 50, 300, 250, 'Card')
    // A child's x and y are its parent's.
    G1 = await rect(20, 20, 60, 40, F)
    const g2 = await rect(100, 20, 60, 40, F)
    await select([G1, g2])
    await run('arrange.group', {})
    await select([])
    await viewAt(1, 400, 250)
  })

  // 8. A press on a layer that is not selected drags it; near another one's edge it snaps, with
  // the red line.
  await item(8, 'Select and move', 1, async () => {
    let guides = 0
    await drag({ x: 150, y: 290 }, { x: 150, y: 143 }, {
      before: async () => {
        guides = (await overlays()).filter((o) => o.kind === 'line' && o.color?.[0] === 0xf2 && o.color?.[1] === 0x48).length
      },
    })
    const b = await box(M1)
    check('the press dragged the layer that was not selected', ids(await selection()) === ids([M1]) && near(b.x, 100), `${show(b)} ${JSON.stringify(await selection())}`)
    check('it snapped to the other one\'s top (3 px off)', near(b.y, 100, 0.01), show(b))
    check('the red line showed', guides > 0, `${guides} lines`)
    check('one undo step', (await undoStack())[0] !== undefined)
    await quiet(() => undo())
  })

  // 9. A marquee from the empty canvas selects what it touches; Shift+click adds one.
  await item(9, 'Select several', '1–n', async () => {
    await quiet(deselect)
    await drag({ x: 240, y: 60 }, { x: 320, y: 120 })
    check('a marquee that touches a rectangle\'s corner selects it', ids(await selection()) === ids([M2]), JSON.stringify(await selection()))
    await click(150, 290, ['Shift'])
    check('Shift+click adds one', ids(await selection()) === ids([M1, M2]), JSON.stringify(await selection()))
    const bounds = await docBox((await inspect()).selectionBounds)
    check('one box round both', bounds && near(bounds.x, 100) && near(bounds.y, 100) && near(bounds.w, 300) && near(bounds.h, 230), JSON.stringify(bounds))
    await quiet(deselect)
    await drag({ x: 450, y: 20 }, { x: 510, y: 60 })
    known('a marquee that touches a top-level frame\'s corner selects the frame', ids(await selection()) === ids([F]), 'the engine\'s selection suite pins a marquee that selects a top-level frame only when it holds it whole; changing the rule is the owner\'s (walkthrough A9)', JSON.stringify(await selection()))
  })

  // 10. Cmd+click picks the deepest layer, in a group in a frame.
  await item(10, 'Select a layer in a group', 1, async () => {
    await quiet(deselect)
    await click(550, 90, [mod])
    check('Cmd+click selects the rectangle in the group', ids(await selection()) === ids([G1]), JSON.stringify(await selection()))
  })

  // 11. A corner handle resizes; Shift keeps the proportions, Alt resizes about the centre; the
  // Design panel's W and H follow.
  await item(11, 'Resize', 1, async () => {
    await quiet(async () => {
      await select([M1])
    })
    let label = false
    await drag({ x: 200, y: 330 }, { x: 220, y: 350 }, {
      before: async () => {
        // The size pill under the selection comes with the canvas chrome.
        const zoom = (await view()).zoom
        const chrome = await designKit((zoom) => window.designKit.invoke('chrome', zoom).then((text) => JSON.parse(text)), zoom).catch(() => null)
        label = /120\D+100/.test(chrome?.sizeLabel?.text ?? '')
      },
    })
    const b = await box(M1)
    check('the corner handle resizes', near(b.x, 100) && near(b.y, 250) && near(b.w, 120) && near(b.h, 100), show(b))
    check('the size label under it follows during the drag', label)
    const w = await kit.getByTestId('design-design-width').locator('input').inputValue()
    const h = await kit.getByTestId('design-design-height').locator('input').inputValue()
    check('the panel\'s W and H follow', w === '120' && h === '100', `${w} x ${h}`)
    await quiet(() => undo())
    await quiet(() => drag({ x: 200, y: 330 }, { x: 240, y: 335 }, { held: ['Shift'] }))
    const s = await box(M1)
    check('Shift keeps the proportions', near(s.w / s.h, 100 / 80, 0.02) && s.w > 110, show(s))
    await quiet(() => undo())
    await quiet(() => drag({ x: 200, y: 330 }, { x: 210, y: 340 }, { held: ['Alt'] }))
    const c = await box(M1)
    check('Alt resizes about the centre', near(c.x + c.w / 2, 150) && near(c.y + c.h / 2, 290) && near(c.w, 120), show(c))
    await quiet(() => undo())
  })

  // 12. W in the panel: 200, +20, *2, each a click, the value and Enter; Tab goes to H.
  await item(12, 'Exact size', 3, async () => {
    await quiet(() => select([M1]))
    await field('design-design-width', '200')
    check('200 sets the width', near((await box(M1)).w, 200, 0.01), show(await box(M1)))
    await quiet(() => field('design-design-width', '+20'))
    check('+20 adds 20', near((await box(M1)).w, 220, 0.01), show(await box(M1)))
    await quiet(() => field('design-design-width', '*2'))
    check('*2 doubles it', near((await box(M1)).w, 440, 0.01), show(await box(M1)))
    await quiet(async () => {
      await kit.getByTestId('design-design-width').locator('input').first().click()
      await page.keyboard.press('Tab')
      await settle(200)
    })
    check('Tab goes to H', (await focused()).includes('design-design-height'), await focused())
    await quiet(async () => {
      await page.keyboard.press('Escape')
      await undo(3)
    })
  })

  // 13. Dragging just outside a corner rotates; with Shift in steps of 15 degrees.
  await item(13, 'Rotate', 1, async () => {
    await quiet(() => select([M1]))
    await drag({ x: 210, y: 242 }, { x: 260, y: 300 }, { held: ['Shift'] })
    const r = (await box(M1)).rotation
    check('a drag outside the corner rotates', Math.abs(r) > 5, `${r}`)
    check('Shift snaps to 15 degrees', near(Math.abs(r) % 15, 0, 0.01) || near(Math.abs(r) % 15, 15, 0.01), `${r}`)
    await quiet(() => undo())
  })

  // 14. Alt+drag drags a copy; Cmd+D makes another at the same distance.
  await item(14, 'Duplicate', '1 + 1', async () => {
    await quiet(() => select([M1]))
    const layers = (await inspect()).layers
    await drag({ x: 150, y: 290 }, { x: 150, y: 440 }, { held: ['Alt'] })
    const [copy] = await selection()
    check('Alt+drag drags a copy, the original stays', (await inspect()).layers === layers + 1 && String(copy) !== String(M1) && near((await box(copy)).y, 400, 1) && near((await box(M1)).y, 250, 0.01), `${(await inspect()).layers - layers} more, ${copy !== undefined ? show(await box(copy)) : ''}`)
    await key(`${mod}+d`)
    const [third] = await selection()
    check('Cmd+D makes another at the same distance', (await inspect()).layers === layers + 2 && third !== undefined && near((await box(third)).y, 550, 1) && near((await box(third)).x, 100, 1), third !== undefined ? show(await box(third)) : '')
    await quiet(() => undo(2))
  })

  // 15. The arrows nudge 1, with Shift 10.
  await item(15, 'Nudge', 1, async () => {
    await quiet(() => select([M1]))
    await key('ArrowRight')
    check('an arrow moves 1', near((await box(M1)).x, 101, 0.01), show(await box(M1)))
    await key('Shift+ArrowDown')
    check('Shift+arrow moves 10', near((await box(M1)).y, 260, 0.01), show(await box(M1)))
    await quiet(() => undo(2))
  })

  // 29. Undo and redo: one drag, one field, one text edit, each one step.
  await item(29, 'Undo and redo', 1, async () => {
    await quiet(() => select([M1]))
    await quiet(() => drag({ x: 150, y: 290 }, { x: 180, y: 290 }))
    await key(`${mod}+z`)
    check('Cmd+Z takes a whole drag back', near((await box(M1)).x, 100, 0.01), show(await box(M1)))
    await key(`Shift+${mod}+z`)
    check('Shift+Cmd+Z does it again', near((await box(M1)).x, 130, 0.01), show(await box(M1)))
    await quiet(() => undo())
    await quiet(() => field('design-design-width', '150'))
    await quiet(() => page.keyboard.press('Escape'))
    await key(`${mod}+z`)
    check('one step takes a field back', near((await box(M1)).w, 100, 0.01), show(await box(M1)))
    const t = await quiet(async () => {
      const id = await made(await run('text.add', { text: 'Undo me', x: 100, y: 450 }))
      await deselect()
      const b = await box(id)
      await dbl(b.x + b.w / 2, b.y + b.h / 2)
      await page.keyboard.type('Gone', { delay: 15 })
      await page.keyboard.press('Escape')
      await settle(300)
      return id
    })
    check('the text edit took', (await node(t)).characters === 'Gone', (await node(t)).characters)
    await key(`${mod}+z`)
    check('one step takes a whole text edit back', (await node(t)).characters === 'Undo me', (await node(t)).characters)
  })

  // 31. With one layer selected, Alt over another shows the distance.
  await item(31, 'Measure the distance', 1, async () => {
    await quiet(() => select([M1]))
    const over = await toPage(350, 140)
    await page.mouse.move(over.x - 20, over.y)
    counted()
    await page.keyboard.down('Alt')
    await page.mouse.move(over.x, over.y, { steps: 4 })
    await until(async () => (await overlays()).some((o) => o.kind === 'measure'), 20)
    const measured = (await overlays()).filter((o) => o.kind === 'measure').length
    await page.keyboard.up('Alt')
    check('Alt over the other layer shows the distances', measured > 0, `${measured} measurements`)
  })

  // 16–19 on one rectangle.
  let P = null
  await quiet(async () => {
    await newPage('Paint')
    P = await rect(100, 100, 160, 120)
    await select([P])
    await viewAt(1, 300, 200)
  })

  // 16. The fill: the hex typed and Enter; or a drag in the picker, one undo step.
  await item(16, 'Change the fill', '2–3', async () => {
    const undoDepth = (await undoStack()).length
    await field('design-fill-0-hex', 'FF0000')
    check('the hex sets the fill', (await node(P)).fills?.[0]?.color === '#FF0000', JSON.stringify((await node(P)).fills))
    check('one undo step', (await undoStack()).length === undoDepth + 1, JSON.stringify((await undoStack()).slice(0, 2)))
    await quiet(async () => {
      await select([P])
      await press(kit.getByTestId('design-fill-0-swatch'))
    })
    const sv = kit.getByTestId('design-color-picker-sv')
    await until(async () => (await sv.count()) === 1, 30)
    const area = await sv.boundingBox()
    const depth = (await undoStack()).length
    let live = null
    await quiet(() => dragPx({ x: area.x + area.width * 0.3, y: area.y + area.height * 0.3 }, { x: area.x + area.width * 0.7, y: area.y + area.height * 0.6 }, {
      before: async () => {
        live = (await node(P)).fills?.[0]?.color
      },
    }))
    const after = (await node(P)).fills?.[0]?.color
    check('a drag in the picker changes the fill as it goes', live !== '#FF0000' && after !== '#FF0000', `${live} -> ${after}`)
    check('the whole picker drag is one undo step', (await undoStack()).length === depth + 1, JSON.stringify((await undoStack()).slice(0, 3)))
    await quiet(async () => {
      await page.keyboard.press('Escape')
      await settle(200)
      await select([P])
    })
  })

  // 17. The Stroke section's +: a stroke, with its weight and its position.
  await item(17, 'Add a stroke', 1, async () => {
    await press(kit.getByTestId('design-stroke-add'))
    const n = await node(P)
    check('a stroke', (n.strokes?.length ?? 0) === 1, JSON.stringify(n.strokes))
    check('its weight and position can be set', (await kit.getByTestId('design-stroke-weight').count()) > 0 && (await kit.getByTestId('design-stroke-align').count()) > 0)
  })

  // 18. The radius handle inside a corner, or the Corner radius field.
  await item(18, 'Round the corners', '1–3', async () => {
    await quiet(() => select([P]))
    // The handle sits 10 px in along the corner's bisector at radius 0 (100 % zoom).
    await drag({ x: 107, y: 107 }, { x: 121, y: 121 }, { steps: 10 })
    const r = (await node(P)).cornerRadius ?? 0
    check('dragging the handle inside a corner rounds every corner', r > 5, JSON.stringify([r, (await node(P)).topLeftRadius]))
    await quiet(() => field('design-design-radius', '8'))
    check('the Corner radius field sets it', (await node(P)).cornerRadius === 8, JSON.stringify((await node(P)).cornerRadius))
    await quiet(() => page.keyboard.press('Escape'))
  })

  // 19. The Effects section's +: a drop shadow, X 0, Y 4, blur 4, black at 25 %.
  await item(19, 'Add a drop shadow', 1, async () => {
    await quiet(() => select([P]))
    await press(kit.getByTestId('design-effect-add'))
    const e = (await node(P)).effects?.[0]
    check('a drop shadow X 0 Y 4 blur 4 black 25 %', e?.type === 'dropShadow' && JSON.stringify(e.offset) === '[0,4]' && e.radius === 4 && e.color?.a === 0.25 && e.color?.r === 0 && e.color?.g === 0 && e.color?.b === 0, JSON.stringify(e))
  })

  // 20–23 on three rectangles in a frame.
  let FR = null
  let K = []
  await quiet(async () => {
    await newPage('Arrange')
    FR = await frame(0, 0, 600, 400, 'Board')
    K = [await rect(40, 40, 60, 40, FR), await rect(200, 90, 60, 60, FR), await rect(420, 60, 80, 30, FR)]
    await select([])
    await viewAt(1, 300, 200)
  })
  const kb = async () => Promise.all(K.map(box))

  // 20. Alt+A / H / D / W / V / S align, Ctrl+Alt+H / V distribute; one layer aligns to its frame.
  await item(20, 'Align and distribute', 1, async () => {
    await quiet(async () => {
      await click(70, 60)
      await click(230, 120, ['Shift'])
      await click(460, 75, ['Shift'])
    })
    check('three selected by Shift+click', ids(await selection()) === ids(K), JSON.stringify(await selection()))
    const keys = [
      ['Alt+a', (b) => b.every((r) => near(r.x, 40, 0.01)), 'left'],
      ['Alt+h', (b) => b.every((r) => near(r.x + r.w / 2, b[0].x + b[0].w / 2, 0.01)), 'centre'],
      ['Alt+d', (b) => b.every((r) => near(r.x + r.w, 500, 0.01)), 'right'],
      ['Alt+w', (b) => b.every((r) => near(r.y, 40, 0.01)), 'top'],
      ['Alt+v', (b) => b.every((r) => near(r.y + r.h / 2, b[0].y + b[0].h / 2, 0.01)), 'middle'],
      ['Alt+s', (b) => b.every((r) => near(r.y + r.h, 150, 0.01)), 'bottom'],
    ]
    for (const [k, ok, name] of keys) {
      if (k === 'Alt+a') await key(k)
      else await quiet(() => key(k))
      const b = await kb()
      check(`${k} aligns ${name}`, ok(b), b.map(show).join(' / '))
      await quiet(() => undo())
    }
    const distribute = mac ? 'Control+Alt+h' : 'Alt+Shift+h'
    await quiet(() => key(distribute))
    const d = await kb()
    const gaps = [d[1].x - (d[0].x + d[0].w), d[2].x - (d[1].x + d[1].w)]
    check(`${distribute} spaces them evenly`, near(gaps[0], gaps[1], 0.01), JSON.stringify(gaps))
    await quiet(() => undo())
    await quiet(async () => {
      await deselect()
      await click(70, 60)
    })
    await quiet(() => key('Alt+d'))
    check('one layer aligns to its frame', near((await box(K[0])).x + 60, 600, 0.01), show(await box(K[0])))
    await quiet(() => undo())
  })

  // 21. Cmd+G groups, Alt+Cmd+G frames; the new one selected.
  await item(21, 'Group and frame', 1, async () => {
    await quiet(() => select([K[0], K[1]]))
    await key(`${mod}+g`)
    const [g] = await selection()
    const gn = g !== undefined ? await node(g) : null
    check('Cmd+G: a new group, selected, round both', gn?.type === 'GROUP' && ids(await children(g)) === ids([K[0], K[1]]), JSON.stringify(gn && [gn.type, await children(g)]))
    await quiet(() => undo())
    await quiet(() => select([K[0], K[1]]))
    await quiet(() => key(`${mod}+Alt+g`))
    const [f] = await selection()
    const fn = f !== undefined ? await node(f) : null
    check('Alt+Cmd+G: a new frame, selected, round both', fn?.type === 'FRAME' && String(f) !== String(FR) && ids(await children(f)) === ids([K[0], K[1]]), JSON.stringify(fn && [fn.type, await children(f)]))
    await quiet(() => undo())
  })

  // 22. A double-click on a row's name, the name, Enter; Tab goes on to the next row.
  await item(22, 'Rename a layer', 3, async () => {
    await quiet(() => select([]))
    await quiet(() => run('layer.expandAll', {}).catch(() => undefined))
    const row = kit.locator(`[data-row-id='${K[2]}'] .design-layer-name`)
    await until(async () => (await row.count()) === 1, 30)
    counted()
    await row.dblclick()
    await until(async () => (await kit.getByTestId('design-layer-rename').count()) === 1, 30)
    await type('Third')
    await key('Enter')
    check('the row\'s name is the layer\'s new name', (await node(K[2])).name === 'Third', (await node(K[2])).name)
    await quiet(async () => {
      await row.dblclick()
      await until(async () => (await kit.getByTestId('design-layer-rename').count()) === 1, 30)
      await page.keyboard.type('Third again')
      await page.keyboard.press('Tab')
      await settle(300)
    })
    const next = await kit.getByTestId('design-layer-rename').locator('input').inputValue().catch(() => kit.getByTestId('design-layer-rename').inputValue().catch(() => null))
    check('Tab saves it and renames the next row', (await node(K[2])).name === 'Third again' && next === (await node(K[1])).name, `${(await node(K[2])).name}, field ${next}`)
    await quiet(async () => {
      await page.keyboard.press('Escape')
      await settle(200)
    })
  })

  // 23. A row dragged in the Layers panel, with the drop line; Cmd+] and Cmd+[.
  await item(23, 'Layer order', 1, async () => {
    const was = await children(FR)
    const lowest = was[0]
    const highest = was[was.length - 1]
    const from = await kit.locator(`[data-row-id='${lowest}']`).boundingBox()
    const to = await kit.locator(`[data-row-id='${highest}']`).boundingBox()
    let line = 0
    await dragPx({ x: from.x + 120, y: from.y + from.height / 2 }, { x: to.x + 120, y: to.y + 4 }, {
      steps: 12,
      before: async () => {
        line = await kit.locator('.design-insertion-line').count()
      },
    })
    check('a drop line shows', line === 1, `${line}`)
    check('the row dropped above the top row is on top', (await children(FR)).at(-1) === lowest, JSON.stringify(await children(FR)))
    await quiet(() => undo())
    await quiet(() => select([was[0]]))
    await quiet(() => key(`${mod}+]`))
    check('Cmd+] brings it forward one', (await children(FR))[1] === was[0], JSON.stringify(await children(FR)))
    await quiet(() => key(`${mod}+[`))
    check('Cmd+[ sends it back', (await children(FR))[0] === was[0], JSON.stringify(await children(FR)))
  })

  // 24–26: auto layout.
  let L = []
  await quiet(async () => {
    await newPage('Layout')
    L = [await rect(0, 0, 60, 40), await rect(80, 0, 60, 40), await rect(160, 0, 60, 40)]
    await select([])
    await viewAt(1, 200, 100)
  })
  let AL = null
  // 24. Shift+A on three in a row: a frame round them, horizontal, their gap, nothing moves.
  await item(24, 'Add auto layout', 1, async () => {
    await quiet(() => drag({ x: -20, y: -20 }, { x: 240, y: 60 }))
    check('the three selected', ids(await selection()) === ids(L), JSON.stringify(await selection()))
    const before = await Promise.all(L.map(box))
    await key('Shift+a')
    const [id] = await selection()
    AL = id
    const n = id !== undefined ? await node(id) : null
    check('a frame round them, horizontal, gap 20', n?.type === 'FRAME' && n.layout?.mode === 'horizontal' && n.layout?.gap === 20 && ids(await children(id)) === ids(L), JSON.stringify(n && [n.type, n.layout]))
    const after = await Promise.all(L.map(box))
    check('nothing moved', after.every((b, i) => near(b.x, before[i].x, 0.01) && near(b.y, before[i].y, 0.01)), after.map(show).join(' / '))
  })

  // 25. The padding band dragged on the canvas, or the panel's field.
  await item(25, 'Gap and padding', '1–3', async () => {
    const zone = kit.getByTestId('design-al-padding-3')
    await until(async () => (await zone.count()) === 1, 40)
    const z = await zone.boundingBox()
    const at = { x: z.x + z.width / 2, y: z.y + z.height / 2 }
    const before = (await node(AL)).layout?.padding?.[3] ?? 0
    await dragPx(at, { x: at.x + 12, y: at.y }, { steps: 8 })
    const after = (await node(AL)).layout?.padding?.[3] ?? 0
    check('dragging the left padding band sets it', near(after, before + 12, 0.5), `${before} -> ${after}`)
    await quiet(() => field('design-design-gap', '24'))
    check('the gap field sets the gap', (await node(AL)).layout?.gap === 24, JSON.stringify((await node(AL)).layout))
    await quiet(() => page.keyboard.press('Escape'))
  })

  // 26. Hug from the W menu, or a double-click on the frame's edge; Alt+double-click on a child's
  // edge fills.
  await item(26, 'Fill and hug', '2 / 1', async () => {
    await quiet(async () => {
      await select([AL])
      await run('node.set', { id: AL, props: { sizingH: 'fixed', width: 400 } })
      await settle(200)
    })
    await press(kit.getByTestId('design-design-sizingH'))
    await press(kit.locator('.design-menu').getByText('Hug contents').first())
    check('Hug contents from the W menu', (await node(AL)).sizing?.horizontal === 'hug', JSON.stringify((await node(AL)).sizing))
    await quiet(async () => {
      await run('node.set', { id: AL, props: { sizingH: 'fixed', width: 400 } })
      await select([AL])
      await settle(200)
    })
    const f = await box(AL)
    const edge = await toPage(f.x + f.w, f.y + f.h / 2)
    await page.mouse.move(edge.x - 20, edge.y)
    await page.mouse.move(edge.x, edge.y, { steps: 4 })
    await settle(200)
    await quiet(async () => {
      counted()
      await page.mouse.dblclick(edge.x, edge.y)
      await settle(300)
    })
    check('a double-click on the frame\'s right edge hugs', (await node(AL)).sizing?.horizontal === 'hug', JSON.stringify((await node(AL)).sizing))
    await quiet(async () => {
      await run('node.set', { id: AL, props: { sizingH: 'fixed', width: 400 } })
      await select([L[2]])
      await settle(200)
    })
    const k = await box(L[2])
    const kidEdge = await toPage(k.x + k.w, k.y + k.h / 2)
    await page.mouse.move(kidEdge.x - 10, kidEdge.y)
    await page.mouse.move(kidEdge.x, kidEdge.y, { steps: 3 })
    await settle(200)
    await page.keyboard.down('Alt')
    await page.mouse.dblclick(kidEdge.x, kidEdge.y)
    await page.keyboard.up('Alt')
    await settle(300)
    check('Alt+double-click on a child\'s edge fills', (await node(L[2])).sizing?.horizontal === 'fill', JSON.stringify((await node(L[2])).sizing))
  })

  // 27. Alt+Cmd+K, then Alt+drag of the main component: an instance, whose text is an override.
  await item(27, 'A component and an instance', '1 + 1', async () => {
    let card = null
    let label = null
    await quiet(async () => {
      await newPage('Components')
      card = await frame(0, 0, 200, 80, 'Card')
      label = await made(await run('text.add', { text: 'Title', x: 20, y: 20, parent: card }))
      await select([card])
      await viewAt(1, 250, 150)
    })
    await key(`${mod}+Alt+k`)
    const main = (await selection())[0]
    check('Alt+Cmd+K makes a component', (await node(main)).type === 'COMPONENT', (await node(main)).type)
    await drag({ x: 150, y: 60 }, { x: 150, y: 260 }, { held: ['Alt'], steps: 16 })
    const [copy] = await selection()
    const cn = copy !== undefined ? await node(copy) : null
    check('Alt+drag of it gives an instance', cn?.type === 'INSTANCE' && near((await box(copy)).y, 200, 1), JSON.stringify(cn && [cn.type, await box(copy)]))
    const b = await box(copy)
    await quiet(async () => {
      await deselect()
      for (let i = 0; i < 3 && (await inspect()).editing !== 'text'; i += 1) await dbl(b.x + 40, b.y + 30)
    })
    check('a double-click reaches the instance\'s text', (await inspect()).editing === 'text', (await inspect()).editing)
    await quiet(async () => {
      await page.keyboard.type('Changed', { delay: 15 })
      await page.keyboard.press('Escape')
      await settle(300)
    })
    const inner = (await query('node', { id: copy, depth: 1 })).children?.[0]?.id
    check('the instance\'s text is an override, the main one keeps its own', inner !== undefined && (await node(inner)).characters === 'Changed' && (await node(label)).characters === 'Title', JSON.stringify([inner && (await node(inner)).characters, (await node(label)).characters]))
  })

  // 28. Zoom and pan: Cmd+wheel about the pointer, a two-finger scroll pans, Space+drag pans,
  // Shift+1 fits, Shift+2 the selection, Shift+0 100 %.
  await item(28, 'Zoom and pan', 1, async () => {
    await quiet(() => viewAt(1, 100, 100))
    const at = { x: surface.x + surface.width * 0.6, y: surface.y + surface.height * 0.4 }
    await page.mouse.move(at.x, at.y)
    const under = await toDoc(at.x, at.y)
    counted()
    await page.keyboard.down(mod)
    await page.mouse.wheel(0, -100)
    await page.keyboard.up(mod)
    await settle(400)
    const z = (await view()).zoom
    const still = await toDoc(at.x, at.y)
    check('Cmd+wheel zooms in about the pointer', z > 1.05 && near(still.x, under.x, 1) && near(still.y, under.y, 1), `zoom ${z}, ${JSON.stringify([under, still])}`)
    const v0 = await view()
    await quiet(async () => {
      await page.mouse.wheel(0, 40)
      await settle(300)
    })
    const v1 = await view()
    check('a two-finger scroll pans', near(v1.zoom, v0.zoom, 0.001) && v1.y > v0.y, JSON.stringify([v0.y, v1.y]))
    const before = await view()
    await quiet(async () => {
      await page.keyboard.down('Space')
      await dragPx(at, { x: at.x - 120, y: at.y - 60 }, { steps: 8 })
      await page.keyboard.up('Space')
    })
    const after = await view()
    check('Space+drag pans', near((after.x - before.x) * after.zoom, 120, 3) && near((after.y - before.y) * after.zoom, 60, 3), JSON.stringify([before, after]))
    await quiet(() => key('Shift+1', 500))
    const content = await docBox((await inspect()).contentBounds)
    check('Shift+1 fits everything', content && (await inView(content)), JSON.stringify([content, await view()]))
    const [sel] = await selection()
    if (sel === undefined) await quiet(async () => select([(await inspect()).tree?.[0]?.id]))
    await quiet(() => key('Shift+2', 500))
    const sb = await docBox((await inspect()).selectionBounds)
    check('Shift+2 zooms to the selection', sb && (await inView(sb)) && (sb.w * (await view()).zoom > surface.width * 0.3 || sb.h * (await view()).zoom > surface.height * 0.3), JSON.stringify([sb, (await view()).zoom]))
    await quiet(() => key('Shift+0', 500))
    check('Shift+0 is 100 %', near((await view()).zoom, 1, 0.001), `${(await view()).zoom}`)
  })

  // 30. Cmd+C and Cmd+V paste in place, or in the view's middle when that is out of view; Export's
  // + then Export writes a 1x PNG through the host; Cmd+S saves through the host.
  await item(30, 'Copy and paste, export, save', '2; 2–3; 1–2', async () => {
    let C = null
    await quiet(async () => {
      await newPage('Copy')
      C = await rect(100, 100, 120, 90)
      await select([C])
      await viewAt(1, 250, 200)
    })
    const layers = (await inspect()).layers
    await key(`${mod}+c`)
    await key(`${mod}+v`, 400)
    await until(async () => (await inspect()).layers === layers + 1, 30)
    const [pasted] = await selection()
    check('Cmd+C, Cmd+V pastes in place', (await inspect()).layers === layers + 1 && String(pasted) !== String(C) && near((await box(pasted)).x, 100, 0.5) && near((await box(pasted)).y, 100, 0.5), pasted !== undefined ? show(await box(pasted)) : `${(await inspect()).layers - layers} more`)
    await quiet(() => viewAt(1, 2000, 1500))
    await quiet(async () => {
      await page.keyboard.press(`${mod}+v`)
      await until(async () => (await inspect()).layers === layers + 2, 30)
      await settle(300)
    })
    const [far] = await selection()
    const fb = far !== undefined ? await box(far) : null
    check('out of view it pastes in the view\'s middle', (await inspect()).layers === layers + 2 && fb !== null && near(fb.x + fb.w / 2, 2000, 2) && near(fb.y + fb.h / 2, 1500, 2), fb ? show(fb) : '')
    await quiet(async () => {
      await viewAt(1, 250, 200)
      await select([C])
    })
    const before = await page.evaluate(() => [...window.devHost.files.keys()])
    await press(kit.getByTestId('design-design-export-add'))
    await press(kit.getByTestId('design-design-export-run'))
    await until(async () => (await page.evaluate(() => [...window.devHost.files.keys()])).length > before.length, 60)
    const written = await page.evaluate((before) => {
      const out = []
      for (const [path, file] of window.devHost.files) {
        if (before.includes(path)) continue
        const bytes = new Uint8Array(file.bytes)
        const view = new DataView(file.bytes)
        out.push({ path, png: bytes[0] === 0x89 && bytes[1] === 0x50, w: bytes.length > 24 ? view.getUint32(16) : 0, h: bytes.length > 24 ? view.getUint32(20) : 0 })
      }
      return out
    }, before)
    check('Export writes a 1x PNG through the host', written.length === 1 && written[0].png && /\.png$/i.test(written[0].path) && written[0].w === 120 && written[0].h === 90, JSON.stringify(written))
    const saved = await page.evaluate(() => window.devHost.files.get('Sample.kdesign')?.version ?? 0)
    await quiet(() => page.locator('#kit').click({ position: { x: 700, y: 400 } }).catch(() => undefined))
    await quiet(() => select([C]))
    await key(`${mod}+s`, 600)
    await until(async () => (await page.evaluate(() => window.devHost.files.get('Sample.kdesign')?.version ?? 0)) > saved, 60)
    check('Cmd+S saves through the host', (await page.evaluate(() => window.devHost.files.get('Sample.kdesign')?.version ?? 0)) > saved)
  })

  // 32. The pen: clicks add points, a press dragged out a curve, a click on the first closes it.
  await item(32, 'A curve with the pen', 'n', async () => {
    await quiet(async () => {
      await newPage('Pen')
      await viewAt(1, 250, 200)
    })
    const layers = (await inspect()).layers
    await key('p')
    await click(100, 100)
    await drag({ x: 260, y: 120 }, { x: 300, y: 180 }, { steps: 8 })
    await click(160, 260)
    await click(100, 100)
    await until(async () => (await inspect()).layers === layers + 1, 30)
    const [id] = await selection()
    const n = id !== undefined ? await node(id) : null
    check('a closed, filled vector', n?.type === 'VECTOR' && (n.fills?.length ?? 0) > 0, JSON.stringify(n && [n.type, n.fills]))
    await quiet(async () => {
      await page.keyboard.press('Escape')
      await settle(200)
      await page.keyboard.press('Enter')
      await until(async () => (await inspect()).editing === 'vector', 30)
    })
    const net = (await query('vectorEdit'))?.state?.network
    const curved = (net?.segments ?? []).some((s) => Math.hypot(...s.ta) + Math.hypot(...s.tb) > 1)
    check('three points, three segments, one of them curved', net?.vertices.length === 3 && net?.segments.length === 3 && curved, JSON.stringify(net && [net.vertices.length, net.segments.length, curved]))
    await quiet(async () => {
      await page.keyboard.press('Escape')
      await page.keyboard.press('Escape')
      await settle(200)
      await page.keyboard.press('p')
      await click(400, 100)
      await click(500, 140)
      await page.keyboard.press('Enter')
      await settle(300)
    })
    const [open] = await selection()
    check('Enter ends an open path', open !== undefined && String(open) !== String(id) && (await node(open)).type === 'VECTOR', JSON.stringify([open, (await inspect()).editing]))
    await quiet(async () => {
      await page.keyboard.press('Enter')
      await page.keyboard.press('Escape')
      await settle(200)
    })
  })

  // 33. Cmd+K, a name, Enter.
  await item(33, 'Find a command', 3, async () => {
    let D = null
    await quiet(async () => {
      D = await rect(100, 300, 80, 60)
      await select([D])
    })
    const layers = (await inspect()).layers
    await key(`${mod}+k`)
    check('Cmd+K opens the Actions palette', (await kit.getByTestId('design-actions-palette').count()) === 1)
    await type('Duplicate')
    await key('Enter', 400)
    check('Enter runs the command', (await inspect()).layers === layers + 1 && (await kit.getByTestId('design-actions-palette').count()) === 0, `${(await inspect()).layers - layers} more`)
    await quiet(async () => {
      await page.keyboard.press('Escape')
      await page.keyboard.press(`${mod}+/`)
      await settle(300)
    })
    check('Cmd+/ opens it too', (await kit.getByTestId('design-actions-palette').count()) === 1)
    await quiet(async () => {
      await page.keyboard.press('Escape')
      await settle(200)
    })
  })

  // 34. Prototype: the + handle dragged to a frame connects them; Alt+Cmd+Enter presents.
  await item(34, 'A prototype link and a preview', 'n', async () => {
    let HOME = null
    let NEXT = null
    await quiet(async () => {
      await newPage('Prototype')
      HOME = await frame(0, 0, 200, 300, 'Home')
      NEXT = await frame(300, 0, 200, 300, 'Next')
      await select([])
      await viewAt(1, 250, 150)
    })
    await press(kit.getByTestId('design-tab-prototype'))
    await click(100, 150)
    check('the frame selected in Prototype', ids(await selection()) === ids([HOME]), JSON.stringify(await selection()))
    const handle = kit.getByTestId(`design-proto-handle-${HOME}`)
    await until(async () => (await handle.count()) === 1, 40)
    const hb = await handle.boundingBox()
    const target = await toPage(400, 150)
    counted()
    await quiet(async () => {
      await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2)
      await page.mouse.down()
      for (let i = 1; i <= 10; i += 1) await page.mouse.move(hb.x + (target.x - hb.x) * (i / 10), hb.y + (target.y - hb.y) * (i / 10))
      await page.waitForTimeout(300)
      await page.mouse.move(target.x + 2, target.y + 2)
      await page.waitForTimeout(300)
      await page.mouse.up()
      await settle(400)
    })
    const links = (await node(HOME)).interactions ?? []
    check('dragging the + handle to the other frame links them', links.some((it) => String(it.actions?.[0]?.dest) === String(NEXT)), JSON.stringify(links))
    await quiet(async () => {
      if ((await kit.getByTestId('design-proto-editor').count()) === 1) await kit.getByTestId('design-proto-editor-close').click()
      await settle(200)
    })
    await key(`${mod}+Alt+Enter`, 600)
    check('Alt+Cmd+Enter presents', await until(async () => (await kit.getByTestId('design-proto-present').count()) === 1, 40))
    await quiet(async () => {
      await page.keyboard.press('Escape')
      await until(async () => (await kit.getByTestId('design-proto-present').count()) === 0, 30)
      await kit.getByTestId('design-tab-design').click().catch(() => undefined)
      await settle(200)
    })
  })

  // 35. Shift+Cmd+K places an image chosen through the host (dropping and pasting one are in
  // clipboard.mjs).
  await item(35, 'Place an image', 2, async () => {
    await quiet(async () => {
      await newPage('Image')
      await viewAt(1, 250, 200)
      await page.evaluate(async () => {
        const canvas = new OffscreenCanvas(40, 30)
        const c = canvas.getContext('2d')
        c.fillStyle = '#dc285a'
        c.fillRect(0, 0, 40, 30)
        const blob = await canvas.convertToBlob({ type: 'image/png' })
        window.devHost.files.set('photo.png', { bytes: await blob.arrayBuffer(), version: 1 })
        window.devHost.scriptHostPick('photo.png')
      })
    })
    const layers = (await inspect()).layers
    await key(`Shift+${mod}+k`, 600)
    await until(async () => (await inspect()).layers > layers, 40)
    if ((await inspect()).layers === layers) await click(200, 200)
    await until(async () => (await inspect()).layers > layers, 40)
    const [id] = await selection()
    const n = id !== undefined ? await node(id) : null
    check('the chosen image is on the page', (await inspect()).layers === layers + 1 && n !== null && JSON.stringify(n.fills ?? []).includes('image'), JSON.stringify(n && [n.type, n.fills]))
  })

  // 36. Shift+Cmd+H hides, Shift+Cmd+L locks; the eye and the padlock in the row; a sweep down the
  // eyes hides every row it passes.
  await item(36, 'Hide and lock', 1, async () => {
    let H = []
    await quiet(async () => {
      await newPage('Hide')
      H = [await rect(0, 0, 50, 50), await rect(80, 0, 50, 50), await rect(160, 0, 50, 50)]
      await select([H[0]])
      await viewAt(1, 150, 100)
    })
    await key(`Shift+${mod}+h`)
    check('Shift+Cmd+H hides it', (await node(H[0])).visible === false, JSON.stringify((await node(H[0])).visible))
    await quiet(() => page.keyboard.press(`Shift+${mod}+h`))
    await settle(200)
    await quiet(() => key(`Shift+${mod}+l`))
    check('Shift+Cmd+L locks it', (await node(H[0])).locked === true, JSON.stringify((await node(H[0])).locked))
    await quiet(() => key(`Shift+${mod}+l`))
    await quiet(() => select([]))
    const row = kit.locator(`[data-row-id='${H[1]}']`)
    await row.hover()
    await quiet(() => press(kit.getByTestId(`design-layer-eye-${H[1]}`)))
    check('the row\'s eye hides it', (await node(H[1])).visible === false)
    await quiet(() => press(kit.getByTestId(`design-layer-eye-${H[1]}`)))
    const rows = await Promise.all([2, 1, 0].map((i) => kit.locator(`[data-row-id='${H[i]}']`).boundingBox()))
    await kit.locator(`[data-row-id='${H[2]}']`).hover()
    const eye = await kit.getByTestId(`design-layer-eye-${H[2]}`).boundingBox()
    await quiet(async () => {
      await page.mouse.move(eye.x + eye.width / 2, eye.y + eye.height / 2)
      await page.mouse.down()
      await page.mouse.move(eye.x + eye.width / 2, eye.y + eye.height / 2 + 10, { steps: 3 })
      await page.mouse.move(eye.x + eye.width / 2, rows[2].y + rows[2].height / 2, { steps: 8 })
      await page.mouse.up()
      await settle(300)
    })
    const hidden = await Promise.all(H.map(async (id) => (await node(id)).visible === false))
    check('a sweep down the eyes hides every row it passes', hidden.every(Boolean), JSON.stringify(hidden))
    await quiet(() => undo())
    const shown = await Promise.all(H.map(async (id) => (await node(id)).visible !== false))
    check('one undo shows them all again', shown.every(Boolean), JSON.stringify(shown))
  })
} catch (error) {
  await finish(error)
}
const passed = (list) => list.filter((r) => !r.failed.length && !r.known.length).length
const main = results.filter((r) => r.n <= 30)
const extra = results.filter((r) => r.n > 30)
console.log(`SUMMARY ${which}: ${passed(main)}/${main.length} of the thirty, ${passed(extra)}/${extra.length} of the six`)
await finish()
