# Dev Mode parity

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-17 · **Target:** Figma Design Dev Mode (desktop app 126.10.7)

**Breadth ~98%, ready ~78% (estimated).** The inspect surface is broad, components connect
to the code components of a repository (with their imports, also inside generated layer trees),
changes compare with any saved version, the canvas and the layers panel only inspect in Dev Mode,
and measurements stay on the canvas for everyone. Left: publishing mappings from the code
repository, third-party integrations. Remaining: 1–2 h.

| Feature | Status | Notes |
|---|---|---|
| Dev Mode toggle (Shift+D), Inspect panel | done | the canvas only inspects: presses select, pan and comment but never move, resize, draw or edit text and vectors; menus and shortcuts refuse design edits with a note (Dev Mode's annotations, statuses, dev resources, measurements, code connections and comments still work); a drawing tool chosen in the toolbar goes back to the design. The layers panel only selects and expands (no renaming, reordering, hiding or locking; a double click says why), and the Pages list only opens pages (no adding, renaming, reordering, duplicating or deleting). Files dropped on the canvas aren't imported (a dropped document still opens as a file of its own), and the Assets panel places, swaps and applies nothing, doesn't change Hide when publishing, and doesn't accept library updates or add, remove or swap libraries; each says why |
| Properties with their styles and variables; redlines on hover | done | positions from the frame around the layer (groups aren't frames), rotation, radius (CSS corner order), opacity, blend; colours in hex, RGB, CSS, HSL or HSB (`inspect.properties {colorFormat}`, the Inspect tab's Color format) |
| Code: CSS, iOS (SwiftUI), Android (Compose) | done | plus HTML, React, Vue and Leptos (ours; Leptos: `#[component]` functions with `#[prop]` defaults, variant enums and `view!` markup); components take properties as parameters, instances are calls; CSS in px or rem (`inspect.code {unit, rootFontSize}`), with rotation, blend modes, gradient directions and kinds (linear, radial, conic), text alignment, case and decoration; class names from layer names transliterate non-ASCII scripts into readable ASCII (로그인 화면 → `rogeuin-hwamyeon`, кнопка → `knopka`, über → `uber`; symbols-only names take the node's kind, `.text`), and the 40-character cut lands on a word boundary |
| Assets list with export | done | layers with export settings, icons (small, vector shapes only: SVG) and images (PNG); the icon rule is ours |
| Annotations: pinned properties, categories, coloured callouts, filter | done | |
| Ready for dev list, statuses, "Changed" after edits | done | the list filters Ready for dev, Changed, Completed or any, and badges changed ones (`inspect.readyForDev {status: changed}`, `changed` per item); only sections and frames at the top of a page or section take a status (others are refused) |
| Focus view (one frame, previous / next) | done | |
| Compare changes (before/after side by side or overlaid, property diffs) | done | against the snapshot taken when a frame was first changed after it was marked ready, or any saved version (`dev.compare {version}`; the view's Compare with menu); a frame that isn't ready for dev compares with versions |
| Code connect (map components to repository code) | done | per language (React, Leptos, Vue, SwiftUI, Compose, ArkUI): import path, name and a template with `{property}` and `{property ? "a" : "b"}` placeholders; instances (also inside generated layer trees) show the snippet with their values; edited in the Inspect panel with a live preview; `kition-design.connect.json` export/import; generated layer trees bring the import lines of the connected components they call (also in definitions, slots and swapped-in instances) once each at the top, per language. No CLI publishing mappings from the code repository |
| Plugins in Dev Mode (Jira, GitHub, etc.) | partial (engine only, not in the kit) | the engine runs its own WASM plugins that only read from the Inspect tab, with their panels (callbacks can only read); the kit leaves plug-ins out; no third-party integrations |
| Measurements (redlines on hover, Alt in the design) | done | distances between the selection and the hovered layer, its parent or a guide, auto layout padding and gaps (Shift+Alt); measurements kept on the canvas (`devMode.addMeasurement` between two layers, along a side or to the layer around it; stored on the layer, saved, synced, undoable) drawn in Dev Mode as labelled redlines that follow the layers, listed, placed and removed in the Inspect tab; a measurement whose other layer was deleted isn't drawn until an undo brings it back |
| VS Code extension | out of scope | kition-sdk's MCP server (`kition-kits-mcp`) already serves the kit's tools, which read design data, code and connected snippets, to agents and editors |
| Dev resources links on layers | done | a URL and a name per link on any layer (`devResource.*`, saved, synced, undoable); instances list their main component's (and its set's) read-only; URLs checked as http/https and never fetched (no page titles: a link without a name is named after its URL); the Inspect tab lists, adds, edits and removes them; `devResource.list` lists them |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
