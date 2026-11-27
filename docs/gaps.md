# Gaps

> **Last updated:** 2026-11-26 · **Source:** `features.yaml` (updated 2026-11-17) and the engine's command list

Every known shortfall of Kition Design, from the feature catalogue: features that are not yet `has`,
then the differences recorded on features that work. Update the feature in `features.yaml` when you
land work, then this list. The comparison with the target application is in
[parity/target-app-parity.md](parity/target-app-parity.md); the order of work is in [roadmap.md](roadmap.md).

## Features not done

- `design.agent.ids` (P0, partial): Stable tool ids and typed command references for layers and resources. An independent reading covers all 372 write handlers; 368 commands are mechanically compared across 803 paths (four widget commands need an unavailable plugin). The gate permits zero held-less commands and twelve held-more commands: import name membership, page-window occupants, prototype mode-reference replay, painted-bound facts in five spatial commands and unconditional reflow inputs in three no-op text commands and unread current subtrees in Dev Mode resolution. Batch selection/dataflow, layout limits, text and group membership, variable defaults/aliases and consumed appearance are covered by independent-writer tests. The 300-cell constraint matrix has zero wrong writes and false refusals; other combinations remain unproven. The pinned SDK still accepts implicit-selection wrong writes, refuses unrelated address/membership changes, and cannot observe a linked library file changing by itself.
- `design.board.whiteboard` (P1, partial): Whiteboard: sticky notes, shapes with text, connectors, widgets and data. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.comment.threads` (P1, partial): Comments: add, reply, react, resolve. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.component.instances` (P1, partial): Instances, nested instances and slots. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.component.manage` (P1, partial): Components and variants. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.edit.history` (P1, partial): Undo, redo, cut, copy, paste and other Edit menu commands. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.file.import-export` (P1, partial): Import, place, export and save to version history. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.guide.manage` (P1, partial): Guides. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.image.adjust` (P1, partial): Place and adjust images; image presets. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.layout.auto` (P1, partial): Auto layout and layout grids. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.library.publish` (P1, partial): Team libraries: publish, add, swap, remove. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.object.arrange` (P1, partial): Arrange, align, distribute and tidy up. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.object.frame-group` (P1, partial): Frame, group, ungroup, mask and other Object menu commands. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.object.layers-api` (P1, partial): Create, read, move and reorder layers by id. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.object.transform` (P1, partial): Nudge, move, resize, rotate and flip. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.page.manage` (P1, partial): Pages. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.paint.effects` (P1, partial): Shadows, blurs and other effects. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.paint.fill-stroke` (P1, partial): Fills, strokes and document colours. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.proto.interactions` (P1, partial): Prototyping: interactions, flows, actions and conditionals. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.select.layers` (P1, partial): Select all, none, inverse and matching layers. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.shape.primitives` (P1, partial): Draw rectangles, ellipses, polygons, stars, lines and arrows. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.style.manage` (P1, partial): Styles. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.text.format` (P1, partial): Text editing and formatting: styles, lists, links, case, spacing. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.tool.canvas` (P1, partial): Canvas tools: move, scale, hand, frame, section, pen and the others. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.variable.manage` (P1, partial): Variables, collections and modes. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.vector.boolean` (P1, partial): Boolean operations and Shape Builder. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.vector.edit` (P1, partial): Vector editing: flatten, outline stroke, join, heal and point editing. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.version.history` (P1, partial): Saved versions through the host's checkpoints. A host with the checkpoints capability keeps them (Kition, the development host); elsewhere the panel says it keeps none. The checkpoint's version is the replica's when the host asks after the file write, so an edit made during that wait is in the version saved (kition-sdk). Versions are not compared with each other.
- `design.view.display` (P1, partial): Rulers, grids, outlines, zoom and other View menu commands. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.app.batch` (P2, partial): Run several commands in one step. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.app.preferences` (P2, partial): Preferences. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.
- `design.devmode.handoff` (P2, partial): Dev Mode: changes, focus view, inspect, code, measurements, annotations, resources. The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen.

## Differences on features that work

- `design.editor.keyboard` (P0, has): measured in `web/dev.html` only; whether Kition Desktop gives the editor's frame the keyboard when a design opens is not checked here. The keyboard marquee (`select.area`, Alt+Space / F8) is not offered in the editor: its keys are not bound and the shortcuts dialog does not list it.
- `design.editor.text-and-view` (P0, has): Copy, Cut and Paste while editing a text use the editor's own text clipboard, not the system clipboard (text copied in another app does not paste into a text yet). Measured in `web/dev.html` with synthetic wheel events: a real mouse wheel and a real trackpad are not checked.

## Catalogue

The catalogue has 47 features: 14 `has`, 33 not yet.
