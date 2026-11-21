# Kition Design architecture

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-17 · **Target:** Figma Design

How Kition Design is built today. Engine first, a thin editor, everything a command; a Figma-shaped
data model (a scene graph of frames with auto layout, components and variables).

## Crates and layers

`cargo xtask layers` enforces the layering (`tools/xtask/src/layers.rs`): a crate may depend only on
lower layers, and no crate below L6 may depend on a UI toolkit or a networking crate.

| Layer | Crate | Role |
|---|---|---|
| L0 | `kition-design-geom` | Affine transforms, shape path builders (rounded and smoothed corners, polygons, stars, arcs) on kurbo |
| L0 | `kition-design-color` | Colours (HSB/HSL, LCH/OKLCH, hex/CSS, Display P3), paints (solid, gradients, image), blend modes |
| L1 | `kition-design-doc` | Document model: pages, node tree in a persistent map (its own `pmap`: a copy-on-write B+ tree, stable ids), properties, components, styles, variables, prototype data, comments, versions. Serde everywhere |
| L2 | `kition-design-layout` | Auto layout (horizontal, vertical, wrap, grid; hug/fill/fixed; min/max), constraints, layout grid geometry |
| L2 | `kition-design-text` | Font database (optional embedded kition-fonts + system scan), shaping (harfrust), bidi, line layout, glyph outlines (skrifa), variable fonts |
| L2 | `kition-design-collab` | Real-time collaboration core (no networking): synced object/property model, change sets, fractional positions, server ordering and validation, client reconciliation, wire messages |
| L2 | `kition-design-kdm` | Kition's own: the `kit.design` KDM schema and the bridge between the document and a KDM replica (diff writes, reads, repairs, the `.kdesign` replica entries); the engine's `kdm` module drives it (engine contract 2, [`kdm.md`](features/kdm.md)) |
| L2 | `kition-design-plugin` | Plugin manifest and the WebAssembly sandbox (wasmi, metered, memory cap, no file system, network or clock) |
| L2 | `kition-design-pathops` | Booleans (linesweeper), outline stroke with alignment, arrowheads |
| L3 | `kition-design-render` | Scene → pixels on tiny-skia (CPU): isolated layers for opacity/blend/effects, clips, masks, shadows, blurs (uniform, progressive, background), noise, texture, glass, text; banded in parallel; retained rendering of only what changed; levels of detail when zoomed out |
| L3 | `kition-design-format` | Native `.kdesign` (zip of `document.json` + images; streamed JSON), local library files, versions, REST API file JSON import (`rest/`) |
| L4 | `kition-design-svg` | SVG export and import (usvg) |
| L4 | `kition-design-pdf` | PDF export (krilla) with print options |
| L4 | `kition-design-vectordrawable` | Android Vector Drawable (XML) export: baked path data, gradients through `aapt:attr`, clip groups, compatibility modes and warnings |
| L5 | `kition-design-engine` | Sessions, undo history (`Arc` snapshots), the command registry (`cmd/*`, `spec!`), pointer tools and vector edit mode, snapping and measuring, components sync, libraries, prototype runtime, Dev Mode code generation, the menu catalog (`catalog.rs`) and the keymap (`keymap.rs`), a panic guard |
| L6 | `kition-design-kit` (`bindings/wasm`) | The engine behind kition-sdk's `KitDocument` contract, compiled to WebAssembly for the editor: commands, queries, tools, overlays, rendering, the KDM collaboration members |
| L6 | `kition-design` (`bindings/headless`) | The headless kit executable: the same document served over the Kit Protocol for agents; it never shows a window |
| tools | `xtask` | `ci`, `test`, `layers`, `assets`, `parity`, `version`, `wasm`, `eval` |
| editor | `web/` (not a crate) | The kit's editor: a React app on kition-sdk's `@kition/sdk/editor` that runs the WebAssembly engine in a worker; a host shows it in an iframe over the Kit UI protocol ([`web/README.md`](../web/README.md)) |

## Data model

A `Document` holds pages, a persistent map of nodes (frames, groups, sections, rectangles,
ellipses, polygons, stars, lines, vector networks, text with styled runs, boolean groups,
components, component sets, instances, slices), styles, variable collections, comments, versions
and images by hash. Instances keep their main's id and per-layer overrides; the engine keeps their
materialised children in sync when a main changes (directly for value, size and add/remove edits,
by a rebuild otherwise). Snapshots are cheap (`Arc` and structural sharing), which gives unlimited
undo with one step per gesture.

## Commands: one path for every surface

Every user-visible behaviour is a command in `crates/engine/engine/src/cmd/*` (615 ids)
with an id, label, parameter docs and undo flag. The editor's menus (generated from the catalog),
keyboard shortcuts and Actions palette, and the headless kit executable (`bindings/headless`) that
agents drive, all dispatch the same ids. Commands never open dialogs themselves; they request UI,
and the editor shows the dialog.
`cargo xtask parity` measures the catalog's coverage ([`parity-checklist.md`](parity/parity-checklist.md)).

## Rendering

The CPU renderer is the single source of pixels for the canvas and exports. In the editor the
engine runs in a Web Worker; overlays (selection, handles, guides, labels) leave the engine as
data (`bindings/wasm/src/overlays.rs`) and the editor draws them over the rendered page
(`web/src/canvas/`), never into its pixels. The renderer can keep its last image and re-render
only what changed (`crates/render/render/src/retained.rs`); the kit's binding draws each view the
editor asks for whole, on one thread in the browser (`bindings/wasm/src/render.rs`). SVG and PDF export walk the same scene
and embed rasters where those formats can't express an effect.

## File I/O

`.kdesign` is the native format. In the editor, files are read and written only through the host.
Imports: `.fig` and `.sketch` (best effort), REST API file JSON with variables and images, SVG,
raster images, clipboard content. Exports: PNG, JPG, WebP, SVG, PDF, code
([`file-format-parity.md`](parity/file-format-parity.md)).

## Robustness

Workspace lints deny `unwrap`, `expect`, `panic!` and friends in shipped code; `unsafe_code` is
forbidden. `kition_design_engine::guard` turns a panicking command into an error and rolls the document
back. Junk-parameter sweeps run every command with bad input.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
