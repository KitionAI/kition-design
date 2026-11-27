/**
 * Keyboard regions (2.1 §6): Toolbar, Layers (rail and left panel), Canvas (the default),
 * Properties (the right panel). F6 and Shift+F6 move between them; Escape outside a field returns
 * to the canvas (from the toolbar it is then the canvas's Escape as well). A region is any element
 * with `data-design-region`; it focuses its `[data-design-prefer]` element first (the active tool,
 * the selected layer), else its first control.
 */
import type { KeyboardEvent } from 'react'

export const REGIONS = ['toolbar', 'layers', 'canvas', 'properties'] as const
export type RegionName = (typeof REGIONS)[number]

/** The props of a region's root element. */
export function regionProps(name: RegionName, label: string) {
  return { role: 'region', 'aria-label': label, 'data-design-region': name } as const
}

const FOCUSABLE = 'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])'

/** A region's elements (Layers is the rail and the left panel), drawn ones only. */
function regionOf(root: HTMLElement, name: RegionName) {
  return [...root.querySelectorAll<HTMLElement>(`[data-design-region='${name}']`)].filter((element) => element.offsetParent !== null)
}

/** Focuses region `name`: its preferred control, else its first; the canvas focuses the editor (its keys go to the tools). */
export function focusRegion(root: HTMLElement, name: RegionName) {
  if (name === 'canvas') {
    const app = root.closest<HTMLElement>('.kui-app') ?? root
    app.focus({ preventScroll: true })
    root.dataset.region = 'canvas'
    return true
  }
  const parts = regionOf(root, name)
  const pick = (selector: string) => parts.map((part) => part.querySelector<HTMLElement>(selector)).find(Boolean) ?? null
  const target = pick('[data-design-prefer]') ?? pick(FOCUSABLE)
  if (!target) return false
  target.focus({ preventScroll: false })
  root.dataset.region = name
  return true
}

/** The region holding `element`, or the canvas. */
export function currentRegion(element: Element | null): RegionName {
  const name = element?.closest<HTMLElement>('[data-design-region]')?.dataset.designRegion
  return (REGIONS as readonly string[]).includes(name ?? '') ? (name as RegionName) : 'canvas'
}

/** The shell's key handler for regions; answers whether it took the key. */
export function regionKeyDown(root: HTMLElement | null, event: KeyboardEvent): boolean {
  if (!root) return false
  if (event.key === 'F6' && !event.metaKey && !event.ctrlKey && !event.altKey) {
    const from = REGIONS.indexOf(currentRegion(document.activeElement))
    for (let step = 1; step <= REGIONS.length; step += 1) {
      const next = REGIONS[(from + (event.shiftKey ? -step : step) + REGIONS.length * 2) % REGIONS.length]
      if (focusRegion(root, next)) break
    }
    return true
  }
  if (event.key === 'Escape' && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
    const active = document.activeElement
    if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) return false
    const region = currentRegion(active)
    if (region === 'canvas') return false
    // From the toolbar the Escape is the canvas's too: after a click on a tool it goes back to Move
    // (C15), as Escape on the canvas does.
    if (region === 'toolbar') {
      focusRegion(root, 'canvas')
      return false
    }
    return focusRegion(root, 'canvas')
  }
  return false
}
