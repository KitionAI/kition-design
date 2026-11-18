# Localization parity

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-17 · **Target:** Figma Design (desktop app 126.10.7)

Kition Design's interface has a localization layer with an English catalog, a Russian catalog reviewed
by a native speaker, and Spanish (with a Latin American overlay), French, German, Brazilian
Portuguese, Japanese, Korean, Simplified Chinese, Indonesian, Vietnamese and Czech drafts (machine-drafted,
waiting for native speakers' review), plus Swedish and Turkish catalogs. This page lists the twelve key
languages, then the other languages Figma ships. How to add a
language: [`translating.md`](../features/translating.md).

## What Figma ships

Figma's help center lists its web, desktop and mobile apps in **English, Japanese, Korean, French,
German, Spanish (Spain and Latin America) and Portuguese (Brazil)**. Simplified
Chinese, Hindi, Arabic, Indonesian and Vietnamese are not offered; Figma points users to the
browser's page translation. The `.lproj` folders in `/Applications/Figma.app` (60+) are the
Electron/Chromium locale packs, not Figma's own UI languages. Text *content* in any script is
supported by both apps (CJK, Arabic and Devanagari shaping, IME); Figma has no vertical text.

## What Kition Design has (counted from source)

- **Layer:** the kit's editor (`web/`) translates the upstream editor's English labels through one
  TOML catalog per language in `web/src/i18n/upstream/`, loaded for the host's locale
  (`web/src/strings.ts`). Keys are the English text (`t('Fill')`, `{named}` placeholders, `n`
  picking a plural form with the browser's `Intl.PluralRules`), so a missing translation shows
  English, never a raw key. English labels the catalogs lack are translated by `PHRASES` (`zh-CN` so
  far) and the editor's own keys by `CATALOGS` (`en-US`, `zh-CN`), both in `web/src/strings.ts`. How to add a language: [`translating.md`](../features/translating.md).
- **Strings:** 1,747 UI keys in `en.toml` (42 with plural forms; ~110 are labels from engine and
  document data, such as tool names, blend modes and Dev Mode properties) plus 574 command labels
  and 31 menu wordings by command id: 2,352 entries (counted against `en.toml` and the complete
  Spanish catalog's command and menu tables). Command ids and what the headless kit returns stay
  English.
- **Language:** the editor follows the host's locale and changes language when the host does; it
  has no picker of its own. A locale reads the first catalog kit-editor's `catalogNames` finds: a
  regional tag falls back to its language (fr-CA to fr; es-MX, es-AR and the other regional
  Spanish tags to es), zh, zh-CN, zh-SG and zh-Hans read zh-Hans, while Traditional Chinese
  (zh-Hant, zh-TW, zh-HK, zh-MO) has no catalog and reads English, never Simplified; pt-PT and pt
  read English (there is only pt-BR).
- **Regional overlays:** `es-419.toml` lists only what differs from `es.toml`. The editor reads it
  on its own for the es-419 locale, not layered over `es.toml`, so what it lacks shows English
  there.
- **Check:** none: no language is held to full coverage, and a string added later shows English
  until someone translates it. The drafts translate 94–100% of the strings (table below).
- **Not yet localized:** messages the engine writes (errors, a few toasts, Dev Mode property values
  such as "inside") and number formatting (decimal points stay `.`). A plural form a catalog
  doesn't give reads `other` (CLDR's "many" for millions in French, Spanish and Portuguese reads
  as the plural).
- **Script support in documents:** Unicode bidi with Arabic shaping and mirrored brackets in text
  layers; right-to-left list markers and indents. Devanagari shapes through harfrust. No vertical
  text (Figma has none either).
- **Script support in the UI:** the editor is HTML, so the browser lays out and draws its text;
  its own faces are Inter and, for Chinese, Japanese and Korean, Droid Sans Fallback, which
  `pnpm engine` copies from kition-fonts (`web/src/main.tsx`, `web/scripts/build-engine.sh`);
  other scripts use the browser's faces. No mirrored (right-to-left) layout of panels. No font file
  is in this repo.

## Languages

| Language | Code | UI strings translated | Dialogs / tooltips / help | Script support | Native review | Status | Hours to full |
|---|---|---|---|---|---|---|---|
| English | en | 2,352 / 2,352 (100%, source) | yes | Latin | yes (source) | full | 0 |
| Simplified Chinese (Mandarin) | zh-Hans | 2,216 / 2,352 (94.2%, draft) | yes | CJK text in documents; UI text drawn by the browser (the editor's CJK face: Droid Sans Fallback); no Traditional Chinese catalog (zh-TW, zh-HK read English) | no | draft | 1–2 (after native review) |
| Spanish | es, es-419 | 2,352 / 2,352 (100%, draft); es-419 overlays 104 Latin American wordings (draft) | yes | Latin | no | draft | 0–1 (after native review) |
| Hindi | hi | 0 (0%) | no | Devanagari shaping works in documents; UI font fallback to check | no | none | 4–7 |
| Arabic | ar | 0 (0%) | no | Bidi and shaping work in documents; the UI does not mirror | no | none | 9–15 |
| French | fr | 2,328 / 2,352 (99.0%, draft) | yes | Latin | no | draft | 1–2 (after native review) |
| Portuguese | pt-BR | 2,331 / 2,352 (99.1%, draft) | yes | Latin | no | draft | 1–2 (after native review) |
| Indonesian | id | 2,292 / 2,352 (97.4%, draft) | yes | Latin | no | draft | 1–2 (after native review) |
| Japanese | ja | 2,304 / 2,352 (98.0%, draft) | yes | CJK text in documents; UI text drawn by the browser (the editor's CJK face: Droid Sans Fallback) | no | draft | 1–2 (after native review) |
| German | de | 2,226 / 2,352 (94.6%, draft) | yes | Latin; long words elide in narrow fields | no | draft | 1–2 (after native review) |
| Korean | ko | 2,216 / 2,352 (94.2%, draft) | yes | Hangul text in documents; UI text drawn by the browser (the editor's CJK face: Droid Sans Fallback) | no | draft | 1–2 (after native review) |
| Vietnamese | vi | 2,352 / 2,352 (100%, draft) | yes | Latin with stacked diacritics | no | draft | 1–2 (after native review) |
| Thai | th | 1,791 / 1,791 UI strings, 589 of 598 command labels, 31 menu wordings (draft) | yes | Thai text in documents; UI text in the browser's Thai face | no | draft | 1–2 (after native review) |

Other languages: Russian (`ru`) 2,352 / 2,352 (100%), reviewed and approved
by VanTan, one/few/many integer plurals; Swedish (`sv`) 2,314 / 2,352 (98.4%), native review welcome; Turkish (`tr`) 2,291 / 2,352 (97.4%), whose header
says it was machine-drafted and reviewed by a native speaker; Czech (`cs`) 2,292 / 2,352 (97.4%), a draft
with one/few/other plurals. Figma ships none beyond the eight above.

The layer is done. Per-language hours above are agent hours for a catalog of ~1,800 entries
(a machine draft, then a pass over the panels for overflow), plus each language's plural rule where
it isn't one/other; every language also needs a native speaker's review (human). Left before full
parity besides the languages: localizing engine messages and number formatting (4–6 h);
right-to-left mirroring of the panels is in Arabic's hours. Total: **29–51 h**.

Priority: native reviews of the drafts and the es-419 overlay (every one of Figma's languages
and variants has one, plus Simplified Chinese, Indonesian, Vietnamese and Czech, which Figma doesn't ship), then
Hindi and Arabic. Traditional Chinese is not planned (no catalog; its users see English).

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
