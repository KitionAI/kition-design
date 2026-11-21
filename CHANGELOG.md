# Changelog

Changes to Kition Design, newest first. A version is the kit package version in `kit.json`;
each package is published with a `package-v<version>` tag.

## 0.1.2

- A host tells a design file's document id (`document.identity`) and makes a copied design a new document (`document.fork`): the fork keeps the pages, layers and their ids, `document.json` and the thumbnail, and carries a new id and none of the source's history: no change entries, no saved versions or branches, and only the pictures it uses (Kit Process Protocol section 5.6, kition-sdk 6b4d471). Two copies of the same `.kdesign` were one document until now.
- The headless program speaks Kit Process Protocol version 1 (`headless.protocol` is `kition-kit/1`), as hosts on kition-sdk from bb1b07b require.
- kition-sdk 6672663: KDM undo and redo pass over a group whose changes came to nothing, so an undo step that changed nothing no longer stops undo.

## 0.1.1

- `kit.json` declares the engine's editor state (`engine.editorState`), so a host that does not run the engine's Rust code reads `inspect()` without it.
- Lint: the check's clip visit returns `!outside_clip` directly, as clippy 1.100 asks; no change in behaviour.
- README: an agent connects with `npm install -g @kition/cli` and `kition agent add`.

## 0.1.0

First public release of the source, under the Apache License 2.0. It contains:

- The Rust engine: pages, frames, groups, sections, shapes, vector networks and text; auto layout
  and constraints; components, variants and instances; paint, text, effect and grid styles;
  variables with collections and modes; prototypes; comments, version history and branches; Dev
  Mode inspection and code; every behaviour a command (615 of them), rendered on the CPU and
  exported to PNG, JPG, SVG and PDF. `.fig` and `.sketch` files open read-only, as a best-effort
  import.
- The kit engine (`kition-design-kit`), native and WebAssembly: the design is a KDM replica
  (schema `kit.design` v4) kept in the `.kdesign` file, replicas merge concurrent edits, and undo
  is per person and per agent. A check catalogue reports overflowing text, layers off the pixel
  grid, low contrast and missing components, images, styles or variables.
- The headless kit (`kition-design`) for agents over the Kit Protocol: it reads a design's pages
  and layers by id, edits them with the editor's own commands, checks and exports them, and
  previews a page or a layer. A write names the revision it read and is refused when someone
  else changed what it rests on.
- The kit's editor (`web/`): the toolbar, the Layers and Assets panels, the canvas, and the
  Design, Prototype and Dev Mode panels, in kit-editor's frame.
- NOTICE and docs/attribution.md list the upstream work, every asset and the fonts build input,
  and Loro (MIT), linked by the engine and bundled by the editor.
