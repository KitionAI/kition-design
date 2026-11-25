# Changelog

Changes to Kition Design, newest first. A version is the kit package version in `kit.json`;
each package is published with a `package-v<version>` tag.

## Unreleased

- The plain document model (kition-sdk 91cf510, no CRDT; plan 2026-11-24): a `.kdesign` holds `kdm/content.json` (the replica's content as JSON, DEFLATE level 6, no version and no history), written by `Replica::content_snapshot`; the seal, `prepare_save` and the `not-sealed` refusal are gone, and the repair no longer dedupes tree nodes (a tree holds an object once). A saved version is a checkpoint that carries its own snapshot: `version.preview`, `version.restore` and `dev.compare` take `snapshot` (base64) instead of a replica version, so a version shows and restores even when the file was opened without its stored replica; another design's snapshot is refused. The engine answers `replica/restore` (`restoreState`). Replicas take changes in order only; the merge tests and the convergence suite go. The committed samples are converted with their document and object ids. `@kition/sdk` npm:@kitionstage/sdk@0.5.2-stage.23.
- Saved versions are the host's checkpoints (Kit UI protocol 7m, owner 2026-11-23): the version panel saves one with the file (`saveCheckpoint`), lists them newest first a page at a time, renames and deletes them with the host, and shows or restores one from the design's history at its replica version (`version.preview` and `version.restore` take `version`; a restore is one undoable change). Nothing about versions is in the `.kdesign` or the replica; the session-only store, `version.list`, `version.rename`, `version.delete`, `version.duplicate` and `version.autosave` are gone. Without the capability the panel says it keeps none. Dev Mode compares with a saved version by its replica version.
- Branches are cut (owner, 2026-11-23): no `branch.*` commands, no `branches/` in the file, no branch merge in the engine; a person saves a copy of the file instead. Files with `branches/` are not read for them.
- A `.kdesign` carries no `thumbnail.png`: saves write none and nothing reads one from a `.kdesign` (`.fig` exports keep theirs).
- The editor is on `@kitionstage/sdk` 0.5.2-stage.18, whose editor seals the design before every save (`prepareSave`): a save the replica was not sealed for is now refused (`not-sealed`) instead of sealing a copy (task schema-freeze round 4).
- A `.kdesign` holds the design's content, not its history (task schema-freeze, kition-sdk rule M16b): `kdm/content.bin` (the replica's sealed content snapshot), `kdm/meta.json` naming the document, and the images the content uses. `document.json`, the change log and saved versions (`versions/`) are gone; branches, the thumbnail and the published library stay as they were. Hosts seal before saving (`prepareSave`); the history is the replica the host stores. Saved versions live in the session until the host offers kits a checkpoint store. kition-sdk 82a3224.
- Schema 1 (task schema-freeze): a text layer's characters are `Node.text` (a KDM text with marks), paints and effects are `Paint` and `Effect` objects with their bindings, each voter's ballot is its own settings key, and who has read a comment thread is no longer replicated. Files and stored replicas of earlier schemas do not open as such (no migration).

## 0.1.3

- The same kit as 0.1.2, built on kition-sdk 6672663 (the replica's undo passes over an undo group whose changes came to nothing): the 0.1.2 package was built on kition-sdk 6b4d471.

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
