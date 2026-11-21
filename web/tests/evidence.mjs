// Evidence for a failed step: a screenshot of the page in web/test-results/ (gitignored; Kition's
// kit CI uploads that folder from a failed job). `failed(name)` starts one; `settled()` waits for
// them before the browser closes.
import { mkdirSync } from 'node:fs'

const dir = new URL('../test-results/', import.meta.url).pathname

export function evidence(page, script, which) {
  const shots = []
  let count = 0
  return {
    failed(name) {
      count += 1
      const slug = name.replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '').slice(0, 60)
      const path = `${dir}${script}-${which}-${String(count).padStart(2, '0')}-${slug}.png`
      mkdirSync(dir, { recursive: true })
      shots.push(page.screenshot({ path }).catch(() => undefined))
    },
    settled: () => Promise.all(shots),
  }
}
