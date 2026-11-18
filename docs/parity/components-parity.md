# Components, variants, styles and variables parity

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-17 · **Target:** Figma Design (desktop app 126.10.7)

Design systems are why teams adopt Figma. **Components: breadth ~96%, ready ~72%. Styles and
variables: breadth ~95%, ready ~76% (estimated).** QA'd with a synthetic design system of ~300
components used 2,000 times (`crates/engine/engine/src/content/synthetic_ds.rs`, `components_scale_tests.rs`);
not with real libraries. Remaining: 4–10 h here, plus team libraries' tail (4–8 h, partly after
accounts).

## Components and variants

| Feature | Status | Notes |
|---|---|---|
| Create component, instances, detach, reset, go to main, restore main, push changes | done | push from a nested layer goes to the outer main |
| Per-layer overrides with reset; kept across main edits, nested swaps, copy/paste | done | main edits match a full rebuild exactly at 2,000 instances |
| What an instance overrides: fills, strokes, effects, visibility, layer names, text content and every text property (resizing included), auto layout padding, gap and alignment, layout grids, export settings, prototype interactions, nested swaps and property values; the size and resizing (hug, fill, fixed, min and max) of layers inside it | done | each kept when the main changes that property, reset per group; opacity and corner radius too (documentation silent); size and resizing are kept through relayout, undo, save, `.fig` and REST import and detach, and stay with the instance on push |
| What it doesn't: position, rotation, constraints and absolute or grid placement of layers inside it, its auto layout direction and other settings, its structure | done | refused with a message, the fields greyed out; the instance itself resizes and its layers follow the main's constraints |
| Component sets / variants, add variant, switch | done | Combine as variants is in the Object menu, and the canvas and layers context menu and the Design panel offer it when two or more main components are selected; Add variant on a main component not in a set yet makes it a set of itself and the new variant, one undo step; components that are variants already aren't combined again (no set inside a set) |
| A main's copy within its file is an instance of it: Duplicate, Alt-drag, Copy then Paste | done | a variant duplicated in its set stays a new variant and a whole set copies as a new set; only the copied roots become instances (a main inside a copied frame is copied as a main); Cut then Paste and pastes into other files keep the component |
| Switching variants keeps overrides where names match and both variants had the same value; text layers keep theirs | done | |
| Default variant: the top-left one (new instances, Assets thumbnail, generated code) | done | |
| Variant property order (names rewritten), duplicate combinations reported | done | drag a property in the Properties section to reorder it, one undo step, or `component.editProperty {index}` |
| Properties: boolean, text, instance swap (preferred values), variant | done | every component menu scrolls past 320 pt, the default of an instance-swap property's too; no search in them yet |
| Slots (preferred components, fill menu, kept across updates) | done | Figma 2025 feature |
| Exposed nested instances | done | hidden ones hide their properties |
| Descriptions | done | |
| Instance swap: panel, Alt-drop from Assets | done | the panel keeps overrides on same-named layers, an Alt-drop from Assets only text layers' |
| Assets panel: folders by page, frame and slash names (fold away), thumbnails, search, drag into frames and auto layout slots | done | virtualized; `component.list` gives each one's `folder` |
| Hidden from publishing: a leading `.` or `_` in the name | done | the name decides both ways (adding the prefix takes a published component out, removing it brings it back); Hide when publishing in the Assets panel's context menu and Libraries ▸ Hide when publishing (see [ui-parity](ui-parity.md)) |
| Instances pasted into other files follow their mains | done | missing main restorable |
| Component playground (try properties without placing) | partial | variant, boolean, text and instance-swap properties in the Inspect tab (the swap menu lists the preferred components first, then every component in the file, as in the Design panel) and `component.playground`; slots have no control there yet |
| Real-world libraries QA | **not checked** | |

The behaviour of instances, overrides, variants and swaps is checked against the public
documentation by `crates/engine/engine/src/components_behaviour_tests.rs` (one table row per documented
rule, with its derivation). Where the documentation is silent, the suite pins today's behaviour:
layer opacity and corner radius are overridable; rotation of a layer inside an instance is refused
with its position; boolean and text property values stay across variants; a missing combination
picks the variant matching the most values; reset all keeps the instance's size; an instance's own
min / max width and height stay its own. An instance's own export settings and interactions are
its own (the main's later changes to them don't reach existing instances, so connections people
added are never lost). Nested instance overrides: `nested_override_tests.rs`; accepting library
updates with overrides kept: `library_tests.rs`, `components_scale_tests.rs`.

## Libraries

| Feature | Status | Notes |
|---|---|---|
| Publish a file's components, sets, styles and variables (per-asset versions) | done | as local library files |
| Enable a library in a file; read-only linked copies | done | |
| Library updates available; overrides kept | done | in the kit, the host lends the library files the design names (`bindings/wasm/src/libraries.rs`) |
| Swap library (instances, styles and variables to another library's of the same names, overrides kept) | done | what has no match stays and is listed; one undo step |

## Styles and variables

| Feature | Status | Notes |
|---|---|---|
| Paint, text, effect, grid styles; style picker; text styles on character ranges | done | behaviour suite: `styles_behaviour_tests.rs` |
| Editing a style updates its layers; detach and delete keep values; editing a styled property unlinks that style | done | an edited fill/stroke/effect/grid list or text chars unlink; applying a style replaces variables bound to what it sets; text styles on text layers only |
| Organising styles: folders by `/` names, order, duplicate, rename folder, Local styles list | done | `style.move` (into a folder renames), `style.duplicate` (right below), `style.renameFolder`; the page's Local styles list by type and folder (drag to reorder, right-click) |
| Variables inside styles (paint, effect, grid and text styles; each user resolves them in its own modes) | done | `style.bind`, `style_vars_tests`; the editor binds paint colours, shadow colours, blur radii, grid sizes and a text style's font, font style, size, line height and letter spacing, the rest through the command |
| Variables: colour, number, string, boolean; collections, modes, aliases, scopes, groups | done | behaviour suite: `variables_behaviour_tests.rs`; aliases same type, no loops (also on create); scopes filter pickers only |
| Modes: add, duplicate, rename, delete, set as default / reorder | done | layers on Auto follow a new default; explicit modes stay |
| Deleting a bound variable or its collection | done | layers keep their last value and are unbound; aliases to it take its values |
| Code syntax per platform (Web, iOS, Android) | done | generated CSS uses the Web name; in the variables JSON; SwiftUI/Compose code doesn't name variables yet |
| Bindings: fills, strokes, effects, opacity, padding, gaps, sizes, radius, font family/style, font size/weight, line height, letter spacing, paragraph spacing/indent, text, visibility, layout grids, instance properties | done | type-checked per field; number variables fill text content; the Design panel's font family and font style menus start with Apply variable (string variables), and a bound one shows its variable in place of the menu; a colour variable bound to a solid paint brings its whole colour, alpha included, once (the paint's opacity restarts at 100% on binding, an opacity set while bound multiplies on top; unbinding moves the alpha back into the opacity) |
| Explicit modes per frame, library collections' modes | done | |
| Gradient stops and variant properties bound to variables | done | a stop's binding follows it when earlier stops are removed |
| Extended collections | done | library collections, including a library's own extended collections (chains through libraries); overrides per mode with reset; follow library updates, variables new to the library collection being an update of their own; modes inherited. Checked with synthetic libraries only |
| Variables import/export (JSON) | done | the REST local variables shape (round-trips; merges by collection, mode and variable name; `variableCollections` and `variables` read under `meta` or at the top level, a warning when no variables are found or fewer than the collections list); CSS custom properties and Dart constants (Flutter) export; in the browser the export arrives as a download (import there still needs a file picker) |
| Expressions in prototype set-variable actions | done | see [prototyping-parity](prototyping-parity.md) |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
