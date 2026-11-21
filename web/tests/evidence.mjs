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

// What the pointer, click, focus and scroll events in `frame` reached (and its console errors and
// toast), for a step whose click seemed to do nothing: `traceEvents(frame)` starts a record,
// `traced(frame)` reads it.
export async function traceEvents(frame) {
  await frame.evaluate(() => {
    window.__trace = []
    if (window.__tracing) return
    window.__tracing = true
    const describe = (t) => {
      if (!(t instanceof Element)) return String(t?.nodeName)
      const id = t.getAttribute('data-testid')
      const cls = typeof t.className === 'string' && t.className ? `.${t.className.split(' ')[0]}` : ''
      return `${t.tagName.toLowerCase()}${id ? `[${id}]` : ''}${cls}"${(t.textContent ?? '').trim().slice(0, 16)}"`
    }
    for (const type of ['pointerdown', 'pointerup', 'click', 'contextmenu', 'focusin']) {
      document.addEventListener(type, (e) => window.__trace.push(`${type} ${describe(e.target)}${'clientX' in e ? ` ${Math.round(e.clientX)},${Math.round(e.clientY)}` : ''}`), true)
    }
    document.addEventListener('scroll', (e) => window.__trace.push(`scroll ${describe(e.target)}`), true)
    window.addEventListener('unhandledrejection', (e) => window.__trace.push(`unhandledrejection ${String(e.reason?.stack ?? e.reason).slice(0, 300)}`))
    window.addEventListener('error', (e) => window.__trace.push(`error ${String(e.message).slice(0, 200)}`))
    const error = console.error.bind(console)
    console.error = (...args) => {
      window.__trace.push(`console.error ${args.map(String).join(' ').slice(0, 200)}`)
      error(...args)
    }
  })
}

export const traced = (frame) =>
  frame.evaluate(() => `${(window.__trace ?? []).slice(-30).join(' | ')} | toast ${document.querySelector('[data-testid=design-toast]')?.textContent ?? 'none'} | viewport ${innerWidth}x${innerHeight}`)
