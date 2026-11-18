/**
 * The menus package's own strings: text that is not upstream's (upstream labels translate through
 * `t()` and the upstream catalogs). Kept here, per package, with `en-US` required.
 */
import type { Values } from '../strings'

const enUS: Record<string, string> = {
  'import.noPicker': 'This host cannot pick files to import',
  'about.blurb': 'Interface design and prototyping, inside Kition.',
  'about.license': 'Licensed under Apache-2.0. Third-party notices are listed in the kit.',
  'import.notes': 'Kition Design doesn’t support every feature of these files yet. The notes list what was left out or changed in the imported copy; the original file is unchanged.',
  'missing.fonts': 'This file uses fonts this editor doesn’t have. Their text is drawn in a fallback font.',
  'preferences.host': 'Theme, language, interface size, motion and your name follow Kition’s settings.',
}

const zhCN: Record<string, string> = {
  'import.noPicker': '此宿主无法选择要导入的文件',
  'about.blurb': '界面设计与原型，在 Kition 中。',
  'about.license': '依据 Apache-2.0 授权。第三方声明列在套件中。',
  'import.notes': 'Kition Design 尚不支持这些文件的所有功能。注释列出了导入副本中被省略或更改的内容；原始文件保持不变。',
  'missing.fonts': '此文件使用了此编辑器没有的字体。其文本以后备字体绘制。',
  'preferences.host': '主题、语言、界面大小、动效和你的名字遵循 Kition 的设置。',
}

const CATALOGS: Record<string, Record<string, string>> = { 'en-US': enUS, 'zh-CN': zhCN }

/** This package's text for `key` in the editor's locale (English when the locale has none). */
export function own(designKit: { strings: { locale: string } }, key: string, values?: Values): string {
  const locale = designKit.strings.locale
  const catalog = CATALOGS[locale] ?? Object.entries(CATALOGS).find(([tag]) => tag.split('-')[0] === locale.split('-')[0])?.[1]
  const text = catalog?.[key] ?? enUS[key] ?? key
  return values ? text.replace(/\{(\w+)\}/g, (match, name: string) => (name in values ? String(values[name]) : match)) : text
}
