// Dev Mode's steps of the prototype package's test (tests/prototype.mjs calls `devSteps`): the
// Inspect panel, measurements, annotations, dev resources, code, the focus view, compare and
// version history, against the engine through the development host.

export async function devSteps({ page, kit, designKit, step, until, toPage, canvas }) {
  const query = (name, params = {}) => designKit(([n, p]) => window.designKit.query(n, p), [name, params])
  const run = (command, params = {}) => designKit(([c, p]) => window.designKit.run(c, p), [command, params])
  const frame = 68

  await kit.getByTestId('design-mode-inspect').click()
  step('dev: the toolbar enters Inspect', await until(async () => (await kit.getByTestId('design-dev-panel').count()) === 1))
  step('dev: Inspect turns the engine Dev Mode on', await until(async () => (await query('toolbar')).devMode === true))
  await run('select.set', { ids: [] })
  step('dev: the ready list shows the ready-for-dev frame', await until(async () => (await kit.getByTestId(`design-dev-ready-${frame}`).count()) === 1))
  await kit.getByTestId('design-dev-ready-filter').click()
  await kit.getByTestId('design-dev-ready-filter-completed').click()
  step('dev: the status filter lists completed frames only', await until(async () => (await kit.getByTestId(`design-dev-ready-${frame}`).count()) === 0))
  await kit.getByTestId('design-dev-ready-filter').click()
  await kit.getByTestId('design-dev-ready-filter-readyForDev').click()

  // A ready row selects its frame.
  await kit.getByTestId(`design-dev-ready-${frame}`).click()
  step('dev: a ready row selects its frame', await until(async () => (await designKit(() => window.designKit.inspect.selection)).includes(frame)))
  step('dev: the header names the layer', await until(async () => (await kit.getByTestId('design-dev-layer-header').textContent()).includes('Home')))

  // Status.
  await kit.getByTestId('design-dev-status').click()
  await kit.getByTestId('design-dev-status-completed').click()
  step('dev: the status dropdown sets devStatus', await until(async () => (await query('node', { id: frame })).devStatus === 'completed'))
  await kit.getByTestId('design-dev-status').click()
  await kit.getByTestId('design-dev-status-readyForDev').click()
  await until(async () => (await query('node', { id: frame })).devStatus === 'readyForDev')

  // Annotations: add one, pin a property, remove it.
  const notes = async () => (await query('annotations', { id: frame })).annotations.filter((a) => a.id === frame)
  const before = (await notes()).length
  await kit.getByTestId('design-dev-annotation-new').fill('Use 16px padding on phones')
  await kit.getByTestId('design-dev-annotation-new').press('Control+Enter')
  step('dev: adds an annotation', await until(async () => (await notes()).length === before + 1))
  const index = before
  await kit.getByTestId(`design-dev-annotation-pin-${index}`).click()
  await kit.getByTestId('design-dev-pin-width').click()
  step('dev: pins a property to it', await until(async () => (await notes())[index]?.properties.some((p) => p.property === 'width')))
  step('dev: the canvas shows its callout', await until(async () => (await kit.getByTestId(`design-dev-callout-${frame}-${index}`).count()) === 1))
  await kit.getByTestId(`design-dev-annotation-remove-${index}`).click()
  step('dev: removes it', await until(async () => (await notes()).length === before))

  // Dev resources.
  await kit.getByTestId('design-dev-add-resource').click()
  await kit.getByTestId('design-dev-resource-new').locator('input').fill('https://example.com/docs/home')
  await kit.getByTestId('design-dev-resource-new').locator('input').press('Enter')
  step('dev: adds a dev resource', await until(async () => (await query('devResources', { id: frame })).length === 1))
  await kit.getByTestId('design-dev-resource-remove-0').click()
  step('dev: removes it', await until(async () => (await query('devResources', { id: frame })).length === 0))

  // Kept measurements.
  const kept = async () => (await query('redlines', {})).length
  const measured = await kept()
  await kit.getByTestId('design-dev-add-measurement').click()
  await kit.getByTestId('design-dev-add-measurement-top').click()
  step('dev: keeps a measurement along the top', await until(async () => (await kept()) === measured + 1))
  step('dev: draws it on the canvas', await until(async () => (await kit.getByTestId('design-dev-measure-label').count()) > 0))
  await kit.getByTestId(`design-dev-measurement-remove-${frame}-0`).click()
  step('dev: removes it', await until(async () => (await kept()) === measured))

  // Hover another layer: the distance to it.
  await designKit(() => window.designKit.viewTo('view.zoomFit'))
  await page.waitForTimeout(300)
  const area = await designKit(() => window.designKit.inspect.area)
  const dark = (await query('node', { id: 239 })).absoluteBounds
  const at = await toPage(dark.x - area.x + dark.width / 2, dark.y - area.y + 200)
  await page.mouse.move(at.x - 12, at.y)
  await page.mouse.move(at.x, at.y, { steps: 4 })
  step('dev: hovering a layer measures the distance to it', await until(async () => (await kit.getByTestId('design-dev-measure-line').count()) > 0))
  await page.mouse.move(canvas.x + canvas.width - 20, canvas.y + 20)

  // Code: another language.
  const css = await kit.getByTestId('design-dev-code').textContent()
  await kit.getByTestId('design-dev-language').click()
  await kit.getByTestId('design-dev-language-react').click()
  step('dev: the code follows the language', await until(async () => {
    const text = await kit.getByTestId('design-dev-code').textContent()
    return text !== css && /return|export|=>/.test(text)
  }))
  await kit.getByTestId('design-dev-language').click()
  await kit.getByTestId('design-dev-language-css').click()
  // egui wraps code at spaces first: a word with a dash stays whole on its row.
  await until(async () => (await kit.getByTestId('design-dev-code').textContent()) === css)
  const dashed = await kit.getByTestId('design-dev-code').locator('.design-egui-word').evaluateAll((words) => words.map((w) => w.getClientRects().length))
  step('dev: code keeps dashed words whole (egui wraps at spaces first)', dashed.length > 0 && dashed.every((n) => n === 1), JSON.stringify(dashed))

  // Focus view.
  await kit.getByTestId('design-dev-focus-view').click()
  step('dev: Focus view shows the focus bar', await until(async () => (await kit.getByTestId('design-focus-bar').count()) === 1))
  step('dev: the focus bar reads dev.focusState', await until(async () => (await query('dev.focusState'))?.id === frame && (await kit.getByTestId('design-focus-bar').textContent()).includes('Home')))
  await kit.getByTestId('design-focus-exit').click()
  step('dev: and leaves it', await until(async () => (await kit.getByTestId('design-focus-bar').count()) === 0))

  // Compare changes after a change since it was marked ready.
  await run('node.set', { ids: [100], props: { opacity: 0.4 } })
  await until(async () => (await query('node', { id: frame })).devChanged === true)
  await kit.getByTestId('design-dev-compare').click()
  step('dev: Compare changes lists what changed', await until(async () => /changed/i.test((await kit.getByTestId('design-compare-summary').textContent()) ?? '')))
  const loaded = (testId) => kit.getByTestId(testId).evaluate((img) => img.complete && img.naturalWidth > 0)
  step('dev: Compare draws the before picture', await until(async () => await loaded('design-compare-before-image')))
  step('dev: and the picture now', await until(async () => await loaded('design-compare-now-image')))
  step('dev: and outlines the changed layer', await until(async () => (await kit.getByTestId('design-compare-now').locator('.design-dev-compare-box').count()) > 0))
  await kit.getByTestId('design-compare-overlay').click()
  step('dev: Overlay shows both pictures', await until(async () => (await loaded('design-compare-overlay-picture-under')) && (await loaded('design-compare-overlay-picture-image'))))
  await kit.getByTestId('design-compare-resolve').click()
  step('dev: Resolve change closes it and resolves', await until(async () => (await kit.getByTestId('design-compare').count()) === 0 && (await query('node', { id: frame })).devChanged !== true))

  // Version history (saving, previewing and restoring with a host that keeps versions is
  // tests/versions.mjs): the panel opens and, with this host, says it keeps no versions.
  await run('file.versionHistory', {})
  step('dev: the version history panel opens', await until(async () => (await kit.getByTestId('design-version-history').count()) === 1))
  step('dev: without the host keeping versions it says why', (await kit.getByTestId('design-version-unavailable').count()) === 1)
  await kit.getByTestId('design-version-close').click()
  step('dev: the panel closes', await until(async () => (await kit.getByTestId('design-version-history').count()) === 0))

  await run('select.set', { ids: [] })
  await kit.getByTestId('design-mode-design').click()
  step('dev: leaving Inspect turns Dev Mode off', await until(async () => (await query('toolbar')).devMode === false))
}
