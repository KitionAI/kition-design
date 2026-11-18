// web/scripts/readme-shots.mjs: the README pictures of Kition Design (assets/readme/), taken with
// kition-sdk's shared runner (tools/readme-shots). Every shot opens the showcase sample "Sales
// Analytics Dashboard" in the development host, in Classic display mode, and changes it only
// through the editor and the engine's own commands.
//
//   node scripts/readme-shots.mjs [name ...]     # from web/, with the engine built in public/
//
// The sample comes from a kition-showcase checkout next to this repository (or the one
// KITION_SHOWCASE names); kition-sdk likewise (or KITION_SDK). Run it with no HTTP proxy set.
// When pngquant is installed, each picture is then shrunk with --quality 70-90.
import { execFileSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const workspace = path.resolve(web, '..', '..')
const sdk = path.resolve(process.env.KITION_SDK ?? path.join(workspace, 'kition-sdk'))
const showcase = path.resolve(process.env.KITION_SHOWCASE ?? path.join(workspace, 'kition-showcase'))
const { takeShots, sizeLines, sizeReport } = await import(pathToFileURL(path.join(sdk, 'tools/readme-shots/shots.mjs')).href)

const sample = path.join(showcase, 'design', 'sales-analytics-dashboard', 'sales-analytics-dashboard.kdesign')

/** Waits until `check` answers true (every 100 ms, `tries` times). */
const until = async (page, check, tries = 80) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(100)
  }
  return false
}

/** The editor as development builds expose it (`window.designKit`, in the kit's frame). */
function editor(page, frame) {
  const run = (command, params = {}) => frame.evaluate(([c, p]) => window.designKit.run(c, p), [command, params])
  const query = (name, params = {}) => frame.evaluate(([n, p]) => window.designKit.query(n, p), [name, params])
  const selection = () => frame.evaluate(() => window.designKit.inspect.selection)
  /** The first layer named `name` (depth first) in the document query's tree, or in `within`'s. */
  const find = async (name, within = null) => {
    const walk = (nodes) => {
      for (const node of nodes ?? []) {
        if (node.name === name) return node
        const found = walk(node.children)
        if (found) return found
      }
      return null
    }
    const found = walk(within ? [within] : (await query('document', { depth: 20 })).pages[0].children)
    if (!found) throw new Error(`no layer named ${name}`)
    return found
  }
  const select = async (ids) => {
    await run('select.set', { ids })
    await until(page, async () => (await selection()).join() === ids.join())
    await page.waitForTimeout(300)
  }
  /** Fits the canvas to the layers' bounds, with `margin` document pixels around them. */
  const show = async (nodes, margin = 24) => {
    const boxes = nodes.map((node) => node.absoluteBounds)
    const left = Math.min(...boxes.map((b) => b.x))
    const top = Math.min(...boxes.map((b) => b.y))
    const right = Math.max(...boxes.map((b) => b.x + b.width))
    const bottom = Math.max(...boxes.map((b) => b.y + b.height))
    await frame.evaluate((area) => {
      const at = window.designKit.inspect.area
      window.designKit.canvas.current.show({ x0: area.x - at.x, y0: area.y - at.y, x1: area.x - at.x + area.width, y1: area.y - at.y + area.height })
    }, { x: left - margin, y: top - margin, width: right - left + 2 * margin, height: bottom - top + 2 * margin })
    await page.waitForTimeout(400)
  }
  return { run, query, find, select, selection, show }
}

const report = await takeShots({
  web,
  sample,
  query: { modes: 'classic,line,menus,focus' },
  ready: async (page, { frame }) => {
    await until(page, async () => (await frame.evaluate(() => Boolean(window.designKit?.canvas.current?.view()))) === true, 600)
    await frame.evaluate(() => window.designKit.viewTo('view.zoomFit'))
  },
  shots: [
    {
      // The whole editor: the dashboard on the canvas, its layers, and the chart card selected.
      name: 'hero',
      steps: async (page, { frame }) => {
        const kit = editor(page, frame)
        await kit.select([(await kit.find('Revenue Chart')).id])
      },
    },
    {
      // Auto layout: the KPI row selected, its gap band under the pointer, and the Auto layout section.
      name: 'see-1',
      steps: async (page, { frame, kit: ui }) => {
        const kit = editor(page, frame)
        const top = await kit.find('Top Bar')
        const charts = await kit.find('Charts')
        await kit.show([top, charts])
        await kit.select([(await kit.find('KPIs')).id])
        const gap = ui.getByTestId('design-al-gap-1')
        await gap.waitFor()
        await gap.hover()
      },
    },
    {
      // Components: the first order's status pill made a component and named; another "Paid"
      // order's pill replaced by an instance of it, selected; the component listed in Assets.
      name: 'see-2',
      steps: async (page, { frame, kit: ui }) => {
        const kit = editor(page, frame)
        await kit.show([await kit.find('Recent Orders')], 40)
        const pill = await kit.find('Status', await kit.find('Order 24310'))
        await kit.select([pill.id])
        await kit.run('component.create', { ids: [pill.id] })
        await until(page, async () => (await kit.selection()).length === 1)
        const [component] = await kit.selection()
        await kit.run('node.set', { ids: [component], props: { name: 'Status pill' } })
        const row = await kit.find('Order 24307')
        await kit.run('edit.delete', { ids: [(await kit.find('Status', row)).id] })
        const { id: instance } = await kit.run('instance.create', { component, parent: row.id })
        await kit.select([instance])
        await ui.getByTestId('design-rail-assets').click()
        await page.waitForTimeout(800)
      },
    },
    {
      // Fills: the colour picker on the highlighted bar of the chart.
      name: 'see-3',
      steps: async (page, { frame, kit: ui }) => {
        const kit = editor(page, frame)
        await kit.select([(await kit.find('M12')).id])
        await ui.getByTestId('design-fill-0-swatch').click()
        await ui.getByTestId('design-color-picker').waitFor()
      },
    },
    {
      // Variables: a Theme collection with Light and Dark modes, some of it bound to the dashboard,
      // in the Local variables window.
      name: 'see-4',
      steps: async (page, { frame, kit: ui }) => {
        const kit = editor(page, frame)
        const { id: collection } = await kit.run('variable.createCollection', { name: 'Theme', modes: ['Light', 'Dark'] })
        const variable = async (name, type, light, dark) => (await kit.run('variable.create', { collection, name, type, values: { Light: light, Dark: dark } })).id
        const surface = await variable('Surface', 'color', '#FFFFFF', '#1E293B')
        await variable('Canvas', 'color', '#F4F6FA', '#0F172A')
        await variable('Text', 'color', '#0F172A', '#F8FAFC')
        const accent = await variable('Accent', 'color', '#6366F1', '#818CF8')
        await variable('Positive', 'color', '#DCFCE7', '#14532D')
        const radius = await variable('Card radius', 'number', 14, 14)
        const gap = await variable('Card gap', 'number', 20, 16)
        const kpis = await kit.find('KPIs')
        const cards = kpis.children.map((card) => card.id)
        await kit.run('variable.bind', { ids: cards, field: 'fill', variable: surface })
        await kit.run('variable.bind', { ids: cards, field: 'cornerRadius', variable: radius })
        await kit.run('variable.bind', { ids: [kpis.id], field: 'gap', variable: gap })
        await kit.run('variable.bind', { ids: [(await kit.find('M12')).id], field: 'fill', variable: accent })
        await ui.getByTestId('design-rail-variables').click()
        await ui.getByTestId('design-variables').waitFor()
        await page.waitForTimeout(500)
      },
    },
  ],
})

// Shrink each picture when pngquant is there (the README's are at most 400 KB each).
let shrunk = false
for (const { file } of report) {
  try {
    execFileSync('pngquant', ['--quality', '70-90', '--force', '--skip-if-larger', '--ext', '.png', path.resolve(web, '..', file)], { stdio: 'ignore' })
    shrunk = true
  } catch (error) {
    if (error.code === 'ENOENT') break
  }
}
if (shrunk) {
  const fs = await import('node:fs')
  console.log('after pngquant --quality 70-90:')
  for (const line of sizeLines(sizeReport(report.map(({ file }) => ({ file, bytes: fs.statSync(path.resolve(web, '..', file)).size }))))) console.log(line)
}
