/**
 * The editor's text. Labels are the upstream editor's English text (the key of its catalogs in
 * `i18n/upstream/*.toml`); `t('Design')` translates one through the catalog of the host's locale.
 * Keys this editor adds itself (and kit-editor's frame keys) live in `CATALOGS`; English labels
 * the upstream catalogs lack are translated by `PHRASES`.
 */
import { KIT_UI_CATALOGS, catalogNames, type Catalog, type Phrases } from '@kition/kit-editor'

/** The editor's own strings (`en-US` is required; others fall back to it). */
const enUS: Catalog = {
  'kit.opening': 'Opening…',
  'stub.left': 'Left panel',
  'stub.design': 'Design panel',
  'stub.prototype': 'Prototype panel',
  'stub.dev': 'Dev Mode panel',
  'view.only': 'View only',
  'view.onlyTip': 'This file is read-only here',
  'share.disabled': 'Sharing happens in Kition: there is no online share here',
  'interaction.padding': 'Padding',
  'interaction.gap': 'Gap',
  'region.toolbar': 'Toolbar',
  'region.layers': 'Layers',
  'region.canvas': 'Canvas',
  'region.properties': 'Properties',
  'panel.resize': 'Resize panel',
}

const zhCN: Catalog = {
  'kit.opening': '正在打开…',
  'stub.left': '左侧面板',
  'stub.design': '设计面板',
  'stub.prototype': '原型面板',
  'stub.dev': '开发模式面板',
  'view.only': '仅查看',
  'view.onlyTip': '此文件在这里是只读的',
  'share.disabled': '分享在 Kition 中进行：这里没有在线分享',
  'interaction.padding': '内边距',
  'interaction.gap': '间距',
  'region.toolbar': '工具栏',
  'region.layers': '图层',
  'region.canvas': '画布',
  'region.properties': '属性',
  'panel.resize': '调整面板大小',
}

export const CATALOGS: Record<string, Catalog> = { 'en-US': enUS, 'zh-CN': zhCN }

/** Translations of English labels the upstream catalogs lack, by locale. */
export const PHRASES: Record<string, Record<string, string>> = {
  'zh-CN': {
    'Help and resources': '帮助和资源',
    'Present or preview': '演示或预览',
    'People in this file': '此文件中的人员',
    Draw: '绘制',
    Inspect: '检查',
  },
}

/** A catalog entry: a string, or plural forms (`{ one, other }`). */
type Entry = string | Record<string, string>

/** One TOML catalog: `section` → key → entry. Only what the catalogs use is read. */
export type TomlCatalog = Map<string, Map<string, Entry>>

const unescape = (text: string) =>
  text.replace(/\\(u[0-9a-fA-F]{4}|U[0-9a-fA-F]{8}|.)/g, (_, code: string) => {
    if (code[0] === 'u' || code[0] === 'U') return String.fromCodePoint(parseInt(code.slice(1), 16))
    return code === 'n' ? '\n' : code === 't' ? '\t' : code
  })

const QUOTED = /"((?:[^"\\]|\\.)*)"/y

/** Parses the subset of TOML the upstream catalogs are written in: sections, quoted keys, strings, and `{ one = "…", other = "…" }`. */
export function parseCatalog(text: string): TomlCatalog {
  const out: TomlCatalog = new Map()
  let section = new Map<string, Entry>()
  out.set('', section)
  for (const raw of text.split('\n')) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    const header = /^\[([^\]]+)\]$/.exec(line)
    if (header) {
      section = out.get(header[1]) ?? new Map()
      out.set(header[1], section)
      continue
    }
    QUOTED.lastIndex = 0
    const key = QUOTED.exec(line)
    if (!key) continue
    const rest = line.slice(QUOTED.lastIndex).replace(/^\s*=\s*/, '')
    if (rest.startsWith('"')) {
      QUOTED.lastIndex = 0
      const value = QUOTED.exec(rest)
      if (value) section.set(unescape(key[1]), unescape(value[1]))
    } else if (rest.startsWith('{')) {
      const forms: Record<string, string> = {}
      for (const match of rest.matchAll(/(\w+)\s*=\s*"((?:[^"\\]|\\.)*)"/g)) forms[match[1]] = unescape(match[2])
      section.set(unescape(key[1]), forms)
    }
  }
  return out
}

/** The upstream catalogs, by file name (`zh-hans`), as `import.meta.glob` loaders. */
const UPSTREAM = import.meta.glob('./i18n/upstream/*.toml', { query: '?raw', import: 'default' }) as Record<string, () => Promise<string>>

async function loadCatalog(name: string): Promise<TomlCatalog | null> {
  const entry = Object.entries(UPSTREAM).find(([path]) => path.split('/').pop()!.replace(/\.toml$/, '').toLowerCase() === name)
  return entry ? parseCatalog(String(await entry[1]())) : null
}

/** Values for `{name}` placeholders; `n` also picks a plural form. */
export type Values = Record<string, string | number>

/**
 * The editor's text function: `t(key)` for this editor's and kit-editor's own keys, and
 * `t(english)` for the upstream editor's labels, with `{name}` placeholders filled from `values`.
 */
export type DesignStrings = {
  locale: string
  t: (key: string, values?: Values) => string
  /** An engine command's label by id (the upstream `[commands]` catalog), else `english`. */
  command: (id: string, english: string) => string
  /** A main-menu item's label by command id (the upstream `[menu]` catalog), else the command's. */
  menuLabel: (id: string, english: string) => string
  /** English → translation, for kit-editor's menus and dialogs (command titles, menu names). */
  phrase: Phrases
}

function fill(text: string, values?: Values) {
  return values ? text.replace(/\{(\w+)\}/g, (match, name: string) => (name in values ? String(values[name]) : match)) : text
}

/** The own catalogs for `locale`, best first, with kit-editor's under each. */
function ownChain(locale: string): Catalog[] {
  const merged: Record<string, Catalog> = { ...KIT_UI_CATALOGS }
  for (const [tag, catalog] of Object.entries(CATALOGS)) merged[tag] = { ...merged[tag], ...catalog }
  const pick = (tag: string) => merged[tag] ?? Object.entries(merged).find(([key]) => key.split('-')[0] === tag)?.[1]
  return [locale, locale.split('-')[0], 'en-US'].map(pick).filter((catalog): catalog is Catalog => Boolean(catalog))
}

function phrasesFor(locale: string): Record<string, string> | null {
  const names = catalogNames(locale).concat(locale.toLowerCase())
  for (const name of names) {
    const found = Object.entries(PHRASES).find(([key]) => key.toLowerCase() === name)
    if (found) return found[1]
  }
  return null
}

/** Builds the strings for `locale` from the catalogs already parsed (English when `upstream` is null). */
export function makeStrings(locale: string, english: TomlCatalog | null, upstream: TomlCatalog | null): DesignStrings {
  const own = ownChain(locale)
  const isEnglish = locale.toLowerCase().startsWith('en')
  const ui = isEnglish ? null : (upstream?.get('ui') ?? null)
  const extra = isEnglish ? null : phrasesFor(locale)
  const commands = isEnglish ? null : (upstream?.get('commands') ?? null)
  const menu = isEnglish ? null : (upstream?.get('menu') ?? null)
  const plural = new Intl.PluralRules(locale)
  // English command titles → their translation, through the command ids both catalogs share.
  const titles = new Map<string, string>()
  if (commands && english) {
    for (const [id, title] of english.get('commands') ?? []) {
      const translated = commands.get(id)
      if (typeof title === 'string' && typeof translated === 'string') titles.set(title, translated)
    }
  }
  const form = (entry: Entry, values?: Values) => {
    if (typeof entry === 'string') return entry
    const n = values && typeof values.n === 'number' ? values.n : 1
    return entry[plural.select(n)] ?? entry.other ?? Object.values(entry)[0] ?? ''
  }
  // The English catalog last: it holds the plural forms of English text.
  const upstreamText = (key: string, values?: Values): string | null => {
    const entry = ui?.get(key) ?? extra?.[key] ?? titles.get(key) ?? english?.get('ui')?.get(key)
    return entry === undefined ? null : form(entry, values)
  }
  const t = (key: string, values?: Values) => {
    const mine = own.find((catalog) => key in catalog)?.[key]
    if (mine !== undefined) return fill(mine, values)
    return fill(upstreamText(key, values) ?? key, values)
  }
  const command = (id: string, fallback: string) => {
    const entry = commands?.get(id)
    return typeof entry === 'string' ? entry : t(fallback)
  }
  const menuLabel = (id: string, fallback: string) => {
    const entry = menu?.get(id)
    return typeof entry === 'string' ? entry : command(id, fallback)
  }
  const phrase: Phrases = (text) => {
    if (isEnglish) return text
    const entry = ui?.get(text) ?? extra?.[text] ?? titles.get(text)
    return entry === undefined ? text : form(entry)
  }
  return { locale, t, command, menuLabel, phrase }
}

/** Loads the upstream catalogs for `locale` (English always, for command ids and plurals). */
export async function loadStrings(locale: string): Promise<DesignStrings> {
  const english = await loadCatalog('en')
  const name = catalogNames(locale).find((candidate) => Object.keys(UPSTREAM).some((path) => path.toLowerCase().endsWith(`/${candidate}.toml`)))
  const upstream = name && name !== 'en' ? await loadCatalog(name) : null
  return makeStrings(locale, english, upstream)
}
