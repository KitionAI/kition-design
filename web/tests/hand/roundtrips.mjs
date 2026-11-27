// What one pointer event costs (plan 2026-11-26 stage 3, R2): node tests/hand/roundtrips.mjs [chromium|webkit]
// Real pointer events; counts, never milliseconds. Each message the page sends the engine's worker
// is counted by kind, and React's commits by which of the editor's components rendered in them.
// A drag of a rectangle, 30 samples: at most 4 worker messages per sample (the pointer, kit-editor's
// tool state and disabled commands, a render), no panel query, chrome or inspect while it runs, the
// Layers and Design panels not rendered, the Design panel's X and Y following it, and after the
// release one round of queries (no query asked twice). 30 hovers over the same layer: only the
// pointer events, no `canvas.*` answer, no panel or canvas component rendered. Stage 4 (M1, M2):
// while the 100 × 80 rectangle is dragged, the engine draws only the area each sample changed
// (its answer's `damage`), never more than a quarter of the view's pixels a render; after the
// drag the canvas shows what a whole render of the page shows there. Prints PASS or FAIL per step.
import { openHand } from './harness.mjs'

// Before the editor's scripts, in the kit's frame: the worker's messages and React's commits.
const counters = () => {
  if (window === window.top) return
  const m = (window.__roundtrips = { sent: [], back: new Set(), rendered: {}, commits: 0, live: [], on: false })
  const post = Worker.prototype.postMessage
  const seen = new WeakSet()
  Worker.prototype.postMessage = function (data, transfer) {
    if (!seen.has(this)) {
      seen.add(this)
      this.addEventListener('message', (event) => {
        if (event.data?.id != null) m.back.add(event.data.id)
      })
    }
    const r = data?.request
    if (r && m.on) {
      let detail = ''
      if (r.method === 'invoke' && r.name === 'query') detail = `${r.args?.[0] ?? ''} ${r.args?.[1] ?? ''}`
      else if (r.method === 'tool' && r.name === 'pointer') detail = (() => { try { return JSON.parse(r.args[0]).action } catch { return '' } })()
      // The pixels a render asks the engine to draw.
      const pixels = r.method === 'render' || r.method === 'renderArea' ? Number(r.width) * Number(r.height) : 0
      m.sent.push({ id: data.id, kind: `${r.method}${r.name ? `:${r.name}` : ''}`, detail, pixels })
    }
    return post.call(this, data, transfer)
  }
  const WATCH = new Set(['Shell', 'Layers', 'DesignPanel', 'PositionSection', 'LayoutSection', 'DesignCanvas', 'Toolbar'])
  let previous = new WeakSet()
  window.__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
    supportsFiber: true,
    isDisabled: false,
    renderers: new Map(),
    inject(renderer) {
      this.renderers.set(this.renderers.size + 1, renderer)
      return this.renderers.size
    },
    checkDCE() {},
    onScheduleFiberRoot() {},
    onCommitFiberUnmount() {},
    onPostCommitFiberRoot() {},
    onCommitFiberRoot(_id, root) {
      // Every commit's tree is kept (also while not counting), to tell what the next one rendered.
      if (m.on) m.commits += 1
      // A component rendered in this commit: a fiber not in the last commit's tree that did work.
      const next = new WeakSet()
      const stack = [root.current]
      while (stack.length) {
        const fiber = stack.pop()
        next.add(fiber)
        const name = typeof fiber.type === 'function' ? fiber.type.displayName || fiber.type.name : null
        if (m.on && name && WATCH.has(name) && !previous.has(fiber) && (fiber.flags & 1) === 1) m.rendered[name] = (m.rendered[name] ?? 0) + 1
        if (fiber.sibling) stack.push(fiber.sibling)
        if (fiber.child) stack.push(fiber.child)
      }
      previous = next
    },
  }
}

const which = process.argv[2] ?? 'chromium'
const hand = await openHand('hand-roundtrips', which, { init: counters })
const { page, kit, frame, designKit, step, until, part, finish, inspect, node, toPage, viewAt, settle } = hand

/** Starts counting (and listens to the engine's `canvas.*` answers). */
const count = () =>
  designKit(async () => {
    const m = window.__roundtrips
    m.sent.length = 0
    m.rendered = {}
    m.commits = 0
    m.live.length = 0
    if (!m.listening) {
      m.listening = true
      const { onUiRequest } = await import('/src/kit/requests.ts')
      onUiRequest('*', (request) => {
        if (m.on && String(request.kind).startsWith('canvas.')) m.live.push(request.kind)
      })
    }
    m.on = true
  })
/** Stops counting: the messages by kind, the queries, the components rendered. */
const counted = () =>
  designKit(() => {
    const m = window.__roundtrips
    m.on = false
    const kinds = {}
    for (const s of m.sent) {
      const key = s.kind === 'tool:pointer' ? `tool:pointer:${s.detail}` : s.kind
      kinds[key] = (kinds[key] ?? 0) + 1
    }
    const queries = m.sent.filter((s) => s.kind === 'invoke:query').map((s) => s.detail)
    const answers = {}
    for (const kind of m.live) answers[kind] = (answers[kind] ?? 0) + 1
    const renders = m.sent.filter((s) => s.pixels > 0).map((s) => s.pixels)
    return { total: m.sent.length, kinds, queries, renders, rendered: { ...m.rendered }, commits: m.commits, live: [...m.live], answers, unanswered: m.sent.filter((s) => !m.back.has(s.id)).length }
  })
/** Waits until every counted message came back. */
const answered = () => until(async () => (await designKit(() => window.__roundtrips.sent.every((s) => window.__roundtrips.back.has(s.id)))) === true, 100)
const field = (testId) => kit.getByTestId(testId).locator('input').first().inputValue()

try {
  let rect = null
  await part('setup', async () => {
    // The sample's empty canvas right of its screens, at 100 %.
    await viewAt(1, 1600, 700)
    const empty = await toPage(1700, 900)
    await page.mouse.click(empty.x, empty.y)
    await page.keyboard.press('r')
    await until(async () => (await inspect()).tool === 'rectangle', 40)
    const a = await toPage(1500, 600)
    const b = await toPage(1600, 680)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(b.x, b.y, { steps: 6 })
    await page.mouse.up()
    await until(async () => (await inspect()).selection.length === 1 && (await inspect()).tool === 'move', 60)
    await settle(400)
    rect = (await inspect()).selection[0]
    step('setup a rectangle drawn and selected', typeof rect === 'number' && Math.round((await node(rect)).absoluteBounds.width) === 100)
    await until(async () => (await field('design-design-x')) !== '', 40)
  })

  await part('drag', async () => {
    const from = await toPage(1550, 640)
    await page.mouse.move(from.x, from.y)
    await settle(300)
    await page.mouse.down()
    await page.waitForTimeout(100)
    await answered()
    await count()
    const xs = []
    const samples = 30
    for (let i = 1; i <= samples; i += 1) {
      await page.mouse.move(from.x + i * 4, from.y + i * 2)
      // One sample at a time: the next move goes once this one's answer came.
      await until(async () => (await designKit(() => window.__roundtrips.sent.filter((s) => s.kind === 'tool:pointer').length)) >= i, 50)
      await answered()
      if (i === 10 || i === samples) xs.push(await field('design-design-x'))
    }
    await answered()
    const during = await counted()
    const drags = during.kinds['tool:pointer:drag'] ?? 0
    console.log(`  drag: ${JSON.stringify({ ...during, live: undefined })}`)
    step('drag every sample reached the engine', drags === samples, `${drags} of ${samples}`)
    step('drag at most 4 worker messages per sample', during.total <= 4 * drags, `${during.total} messages for ${drags} samples: ${JSON.stringify(during.kinds)}`)
    step('drag no panel query while it runs', during.queries.length === 0, during.queries.join(', '))
    step('drag no chrome or inspect asked while it runs', !during.kinds['invoke:chrome'] && !during.kinds['invoke:inspect'], JSON.stringify(during.kinds))
    step('drag the Layers and Design panels do not render', !during.rendered.Layers && !during.rendered.DesignPanel && !during.rendered.Shell, JSON.stringify(during.rendered))
    step('drag the answers carry the geometry and the chrome', during.live.filter((k) => k === 'canvas.gesture').length === drags && during.live.filter((k) => k === 'canvas.chrome').length === drags, `${during.live.length} canvas answers`)
    step('drag the X field follows the drag', xs.length === 2 && xs[0] !== '' && xs[1] !== '' && Number(xs[1]) > Number(xs[0]), JSON.stringify(xs))
    // Stage 4: each render draws the area the sample changed, not the view (and its margin).
    const shown = await designKit(() => {
      const surface = document.querySelector('[data-testid=kit-canvas-surface]')
      return surface.width * surface.height
    })
    const biggest = Math.max(0, ...during.renders)
    step('drag every render draws at most a quarter of the view', during.renders.length > 0 && biggest <= shown / 4, `${during.renders.length} renders, the largest ${biggest} pixels of ${shown}: ${during.renders.join(' ')}`)
    await count()
    await page.mouse.up()
    await until(async () => (await inspect()).gesture === false, 40)
    await settle(600)
    await answered()
    const after = await counted()
    console.log(`  release: ${JSON.stringify(after)}`)
    const asked = new Map()
    for (const q of after.queries) asked.set(q, (asked.get(q) ?? 0) + 1)
    const twice = [...asked].filter(([, n]) => n > 1)
    step('release the panels ask again, one round', after.queries.some((q) => q.startsWith('selection ')) && twice.length === 0, twice.map(([q, n]) => `${q} x${n}`).join(', '))
    const b = (await node(rect)).absoluteBounds
    // Within a snap's reach of where the pointer left it.
    step('release the rectangle is where the drag left it', Math.abs(b.x - (1500 + samples * 4)) <= 8 && Math.abs(b.y - (600 + samples * 2)) <= 8, JSON.stringify(b))
    step('release the X field shows where it is', Number(await field('design-design-x')) === Number(xs[1]), `${await field('design-design-x')} vs ${xs[1]}`)
    // What the patched canvas shows is what the engine draws there as a whole (no selection drawn
    // over it, the pointer away): the drag's old and new places and round them.
    await page.keyboard.press('Escape')
    const away = await toPage(1500, 950)
    await page.mouse.move(away.x, away.y, { steps: 4 })
    await until(async () => (await inspect()).selection.length === 0 && (await inspect()).hover == null, 40)
    await settle(600)
    const compare = await designKit(async ([x0, y0, x1, y1]) => {
      const k = window.designKit
      const area = k.inspect.area
      const v = k.canvas.current.view()
      const dpr = window.devicePixelRatio || 1
      const s = v.zoom * dpr
      const cx = Math.round((x0 - area.x - v.x) * s)
      const cy = Math.round((y0 - area.y - v.y) * s)
      const w = Math.round((x1 - x0) * s)
      const h = Math.round((y1 - y0) * s)
      const surface = document.querySelector('[data-testid=kit-canvas-surface]')
      const shown = surface.getContext('2d').getImageData(cx, cy, w, h).data
      const whole = await k.doc.render(cx / s + v.x, cy / s + v.y, s, w, h)
      let slight = 0
      let big = 0
      let box = null
      for (let i = 0; i < shown.length; i += 1) {
        const d = Math.abs(shown[i] - whole[i])
        if (d > 2) slight += 1
        if (d > 96) {
          big += 1
          const p = Math.floor(i / 4)
          const [x, y] = [p % w, Math.floor(p / w)]
          box = box ? [Math.min(box[0], x), Math.min(box[1], y), Math.max(box[2], x), Math.max(box[3], y), box[4]] : [x, y, x, y, `${x},${y}: ${shown.slice(i - (i % 4), i - (i % 4) + 4)} vs ${whole.slice(i - (i % 4), i - (i % 4) + 4)}`]
        }
      }
      return { zoom: v.zoom, w, h, slight, big, box }
    }, [1480, 580, 1660, 720])
    step('release the canvas shows what a whole render shows', compare.w > 100 && compare.slight <= 200 && compare.big === 0, JSON.stringify(compare))
  })

  await part('hover', async () => {
    const b = (await node(rect)).absoluteBounds
    const inside = await toPage(b.x + 30, b.y + 30)
    await page.mouse.move(inside.x, inside.y)
    await settle(400)
    await answered()
    await count()
    for (let i = 0; i < 30; i += 1) {
      await page.mouse.move(inside.x + (i % 10) * 2, inside.y + (i % 6))
      await page.waitForTimeout(16)
    }
    await settle(400)
    await answered()
    const hovered = await counted()
    console.log(`  hover: ${JSON.stringify(hovered)}`)
    const pointers = hovered.kinds['tool:pointer:move'] ?? 0
    step('hover only the pointer events go to the worker', pointers > 0 && hovered.total === pointers, JSON.stringify(hovered.kinds))
    step('hover the same layer: no canvas answer for the editor', hovered.live.length === 0, hovered.live.join(', '))
    step('hover the same layer: no panel or canvas component renders', Object.keys(hovered.rendered).length === 0, JSON.stringify(hovered.rendered))
    // Onto the empty canvas: the hover changes once.
    await count()
    const out = await toPage(b.x + 230, b.y + 220)
    await page.mouse.move(out.x, out.y, { steps: 3 })
    await settle(400)
    await answered()
    const left = await counted()
    step('hover leaving the layer tells it once', left.live.filter((k) => k === 'canvas.hover').length === 1, left.live.join(', '))
  })
} catch (error) {
  await finish(error)
}
await finish()
