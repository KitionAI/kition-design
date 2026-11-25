/**
 * Dev Mode's UI state (upstream `DevUi` and the version panel's state): the ready list's filter,
 * which annotations the canvas shows, the colour format and code language, the compare window, and the version history panel. Kept outside React so the panel, the canvas
 * layer and the hosts share it.
 */
import { useSyncExternalStore } from 'react'

export type ReadyFilter = 'readyForDev' | 'changed' | 'completed' | 'any'
export type ColorFormat = 'hex' | 'rgb' | 'css' | 'hsl' | 'hsb'
export type CodeLanguage = 'css' | 'html' | 'react' | 'leptos' | 'vue' | 'tailwind' | 'swiftui' | 'compose' | 'arkui'

/** `query('dev.focusState')`: the focused frame, its status, its place among the ready-for-dev ones. */
export type FocusState = { id: number; name: string; status: string; changed: boolean; index: number | null; count: number; canStep: boolean; canCompare: boolean }

export type DevState = {
  readyFilter: ReadyFilter
  /** `all`, `none` (uncategorised) or a category id. */
  categories: string
  colorFormat: ColorFormat
  language: CodeLanguage
  rem: boolean
  /** The layer compared and the saved version's snapshot (base64) it is compared with; null: with when it was marked ready. */
  compare: { id: number; snapshot: string | null } | null
  versions: { open: boolean; saving: boolean }
}

let state: DevState = {
  readyFilter: 'readyForDev',
  categories: 'all',
  colorFormat: 'hex',
  language: 'css',
  rem: false,
  compare: null,
  versions: { open: false, saving: false },
}

const listeners = new Set<() => void>()

export function getDev(): DevState {
  return state
}

export function setDev(patch: Partial<DevState> | ((now: DevState) => Partial<DevState>)) {
  const next = typeof patch === 'function' ? patch(state) : patch
  state = { ...state, ...next }
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useDev(): DevState {
  return useSyncExternalStore(subscribe, getDev, getDev)
}

/** Opens the version history panel (with the save form when `saving`). */
export function openVersionHistory(saving = false) {
  setDev((now) => ({ versions: { ...now.versions, open: true, saving: saving || now.versions.saving } }))
}

export const LANGUAGES: Array<{ value: CodeLanguage; label: string; web: boolean }> = [
  { value: 'css', label: 'CSS', web: true },
  { value: 'html', label: 'HTML + CSS', web: true },
  { value: 'react', label: 'React', web: true },
  { value: 'leptos', label: 'Leptos', web: true },
  { value: 'vue', label: 'Vue', web: true },
  { value: 'tailwind', label: 'React + Tailwind', web: true },
  { value: 'swiftui', label: 'SwiftUI', web: false },
  { value: 'compose', label: 'Compose', web: false },
  { value: 'arkui', label: 'ArkUI', web: false },
]

export const COLOR_FORMATS: Array<{ value: ColorFormat; label: string }> = [
  { value: 'hex', label: 'Hex' },
  { value: 'rgb', label: 'RGB' },
  { value: 'css', label: 'CSS' },
  { value: 'hsl', label: 'HSL' },
  { value: 'hsb', label: 'HSB' },
]

/** The status badge colours (`DEV_READY`, `DEV_CHANGED`, `DEV_COMPLETED`). */
export const STATUS_COLORS = { readyForDev: '#087d4f', changed: '#a75b00', completed: '#6e6e73' } as const

/** Upstream's `relative_time` buckets for a Unix time in seconds. */
export function relativeTime(t: (key: string, values?: Record<string, string | number>) => string, seconds: number, now = Date.now() / 1000): string {
  const ago = Math.max(0, now - seconds)
  const minute = 60
  const hour = 60 * minute
  const day = 24 * hour
  if (ago < minute) return t('Just now')
  if (ago < hour) return t('{n} minutes ago', { n: Math.floor(ago / minute) })
  if (ago < day) return t('{n} hours ago', { n: Math.floor(ago / hour) })
  if (ago < 2 * day) return t('Yesterday')
  if (ago < 7 * day) return t('{n} days ago', { n: Math.floor(ago / day) })
  if (ago < 30 * day) return t('{n} weeks ago', { n: Math.floor(ago / (7 * day)) })
  if (ago < 365 * day) return t('{n} months ago', { n: Math.floor(ago / (30 * day)) })
  return t('{n} years ago', { n: Math.floor(ago / (365 * day)) })
}

/** Copies `text` to the clipboard (the iframe may refuse; a hidden textarea is the fallback). */
export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    return
  } catch {
    // Fall back below.
  }
  const area = document.createElement('textarea')
  area.value = text
  area.style.position = 'fixed'
  area.style.opacity = '0'
  document.body.appendChild(area)
  area.select()
  try {
    document.execCommand('copy')
  } finally {
    area.remove()
  }
}
