# Translating Kition Design

The kit's editor (`web/`) shows the upstream editor's English labels and translates them through
one TOML catalog per language in `web/src/i18n/upstream/`. The catalogs are bundled with the
editor and each is loaded only when a locale needs it. The editor follows the host's locale: when
it starts, and again whenever the host changes its locale, it loads the English catalog and the
catalog of that locale (`web/src/main.tsx`, `loadStrings` in `web/src/strings.ts`). The editor has
no language picker of its own.

Which catalog a locale reads: kit-editor's `catalogNames(locale)` lists the names to try, best
first (the whole tag in lower case; for Chinese, `zh-hant` for `zh-TW`, `zh-HK`, `zh-MO` and
`zh-Hant`, else `zh-hans`; then the language alone), and the first that names a file here wins
(file names compare without case). So `fr-CA` reads `fr`, `pt-BR` reads `pt-BR`, `zh`, `zh-CN`
and `zh-SG` read `zh-Hans`, and `es-419` reads `es-419`, while `es-MX` and the other regional
Spanish tags read `es`. Traditional Chinese has no catalog and reads English, never Simplified;
`pt-PT` and `pt` read English too (there is only `pt-BR`). English locales read no catalog: the
English text shows as it is.

The catalogs: English (`en`, the source), and Spanish (`es`, with the Latin American overlay
`es-419`), French (`fr`), German (`de`), Dutch (`nl`), Brazilian Portuguese (`pt-BR`), Japanese
(`ja`), Korean (`ko`), Simplified Chinese (`zh-Hans`), Czech (`cs`), Indonesian (`id`), Finnish
(`fi`), Bengali (`bn`), Italian (`it`), Vietnamese (`vi`) and Thai (`th`), machine-drafted and waiting for native speakers' review. Polish (`pl`,
Polski) was reviewed and approved by Lucas (@lukasz-sklad). Georgian (`ka`) was written with AI
assistance and reviewed by a native speaker, and stays a draft until a maintainer changes its status.
Russian (`ru`, Русский) was reviewed and approved by VanTan; its catalog keeps Frame, Dev Mode and
Smart animate as familiar tool names and uses « » quotes. Turkish (`tr`) was machine-drafted and
reviewed by a native speaker, and Swedish (`sv`) translates every UI string, command label and menu wording.

## The catalogs

- **`en.toml`** lists the English keys under `[ui]`. A key is the English text as the editor
  writes it, so the code stays readable: `t('Fill')`, `t('Delete {name}?', { name })`,
  `t('{n} layers', { n: count })`. Values equal their keys except plurals, which give the forms:
  `"{n} layers" = { one = "{n} layer", other = "{n} layers" }`. The editor always loads it, for
  the plural forms of English text.
- **`es.toml`** and other languages translate the same keys under `[ui]`, plus command labels by
  command id under `[commands]` (`strings.command(id, english)`) and, under `[menu]`, the few main
  menu items worded differently from their command (`strings.menuLabel(id, english)`).
- `[meta] name` is the language's own name and `[meta] status` marks a draft or a review; the
  editor does not read `[meta]`.
- A **regional overlay** (`es-419.toml`) lists only the entries worded differently there. The
  editor reads it on its own for the `es-419` locale: it is not layered over `es.toml`, so an entry
  the overlay lacks shows English there.

A key a catalog lacks shows its English text, so a partial catalog is safe to ship.

Typography follows each language: French puts a no-break space (`\u00A0` in the TOML) before
`:` `;` `?` `!`, inside « » and before `%`; German uses „ “ and formal address (Sie); Brazilian
Portuguese uses Brazilian words (arquivo, salvar, tela, você), not European ones. Latin American
Spanish overrides Spain's wording where it differs (agregar, presionar, configuración for settings,
Enter for the key, control for a gamepad, resorte) and keeps Spanish's tú. Japanese is
concise, without です/ます endings, with full-width （）「」：、。 where Japanese UI text has them;
Korean labels end without 합니다 (instructions in -세요). Simplified Chinese is concise, with
full-width （）：，、。“” in Chinese text and a space between Chinese and Latin words or numbers.
None of the three has plural forms. Turkish uses bare imperatives for buttons and menu items
(Kaydet, Sil) and the polite -in/-ın for instructions (seçin, tıklayın), puts the percent sign before
the number (%50) and keeps a noun singular after a count, so its plural entries give only `other`.
Czech uses formal address, „ “ quotes and no space before `:`. Use consistent design terms (rámec,
komponenta, instance, varianta, automatické rozvržení, omezení, výplň, tah, efekt, prototyp,
proměnná, styl). Indonesian uses bare imperatives for buttons and menu items (Simpan, Hapus), the
formal Anda (never kamu), keeps established design terms in English (frame, layer, component,
instance, auto layout, plugin, file) and keeps a noun unchanged after a count, so its plural entries
give only `other`. Georgian has no letter case (never write Mtavruli for capitals), uses verbal nouns
for commands (გვერდის დამატება) and keeps a noun singular after a numeral, so counts read
"შრე: {n}" and its plural entries give the same text for `one` and `other`. Vietnamese uses concise
imperatives for buttons and menu items (Lưu, Xóa) and keeps a noun unchanged after a count, so its
plural entries give only `other`. Thai is concise, with bare imperatives for buttons and
menu items (บันทึก, ลบ), no ครับ/ค่ะ endings, no spaces between Thai words (a space only separates
clauses, and Thai from Latin words or numbers), and design terms transliterated as Thai designers say
them (เฟรม, เลเยอร์, คอมโพเนนต์, อินสแตนซ์, ออโต้เลย์เอาต์). It has no plural forms: plural entries give
only `other`, and dates read `{day} {month} {year}` with Thai month abbreviations.

The editor draws its interface in Inter and loads Droid Sans Fallback for Chinese, Japanese and
Korean text (`web/src/main.tsx`); `pnpm engine` copies both faces from a kition-fonts checkout into
`web/public/fonts` (`web/scripts/build-engine.sh`), and without them the browser's own faces are
used.

## Adding a language

1. Copy `web/src/i18n/upstream/es.toml` to `<code>.toml` in the same folder (a BCP 47 code that
   `catalogNames` finds for the locale: `fr`, `pt-BR`, `ja`) and set `[meta] name` and
   `status = "draft — needs native review"`.
2. Translate every value; keep the keys. Keep `{placeholders}` exactly, "…" where English has it,
   brand names and keyboard glyphs. The English labels of commands are the engine's (each `spec!`
   in `crates/engine/engine/src/cmd/`); `es.toml`'s `[commands]` lists their ids.
3. Plurals: the editor picks a form with the browser's `Intl.PluralRules` for the locale and falls
   back to `other`, so give the forms the language's rule names (Japanese, Korean, Chinese,
   Vietnamese and Thai give only `other`). `other` is required; `one`, `few` and `many` are
   optional. `one` covers 1 (also 0 in French and Brazilian Portuguese). Russian uses `one` for 1,
   21, 31… except 11; `few` for 2–4, 22–24… except 12–14; and `many` for the other integers,
   including 0. Keep `{n}` in all Russian forms. A missing form reads `other`. Czech uses `one`
   for 1, `few` for 2–4 and `other` for 0 and 5+ (e.g. `{n} vrstva`, `{n} vrstvy`, `{n} vrstev`).
   Keep `{n}` in all three forms. Czech's CLDR `many` applies only to decimals.
   Polish uses `one` for 1 only, `few` like Russian (2–4, 22–24… except 12–14) and `many` for
   the other integers, including 0 and 21 (e.g. `{n} warstwa`, `{n} warstwy`, `{n} warstw`).
4. A string added later shows English in every language until someone translates it; a missing
   translation never fails a check or a build.
5. Look at the panels in the editor (longer words must fit the right panel): run `pnpm dev` in
   `web/` and open the development host with the locale, `dev.html?locale=<code>`.

Machine-drafted catalogs stay marked as drafts until a native speaker has reviewed them; a reviewed
catalog stays a draft until a maintainer approves it (its `status` says which).

## Adding strings in code

In `web/`, pass every string a user sees or a screen reader hears through `t` (from
`useDesignKit()`): `t('English label')`, `{named}` placeholders from the values
(`t('Delete {name}?', { name })`), and `n` among them to pick a plural form. Command labels go
through `strings.command(id, english)` and main menu items through `strings.menuLabel(id, english)`.
Don't translate ids, command ids, JSON, log messages, file extensions or document content.

Where a translation comes from (`web/src/strings.ts`): English text the upstream catalogs have is
translated through them; an English label they lack goes into `PHRASES` (per locale); keys of the
editor's own that are not upstream text go into `CATALOGS` (`en-US` and `zh-CN`).

The importers' fixed notes (the string literals passed to `warn` in `crates/formats/format/src/fig` and
`crates/formats/format/src/rest`) are keys too: the editor's import report passes each note through
`t`, so it shows in the editor's language where a catalog has it. Notes built with `format!` show
in English.
