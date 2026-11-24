# Target-app parity

> **Last updated:** 2026-11-17 · **Source:** `features.yaml` (updated 2026-11-17) and the engine's command list

Each feature in `features.yaml` records its status against what the feature promises and, where it
has one, the target application it is compared with (Figma desktop 126.10.7 (macOS)).
The difference column is the feature's recorded gap.

| Feature | Priority | Status | Target | Difference |
| --- | --- | --- | --- | --- |
| `design.agent.act` An agent sets up auto layout on a frame | P0 | has | — | — |
| `design.agent.context` No agent call depends on or leaves behind the current page, the selection, the view or the modes | P0 | has | — | — |
| `design.agent.ids` Stable tool ids and typed command references for layers and resources | P0 | partial | — | An independent reading covers all 372 write handlers; 368 commands are mechanically compared across 803 paths (four widget commands need an unavailable plugin). The gate permits zero held-less commands and twelve held-more commands: import name membership, page-window occupants, prototype mode-reference replay, painted-bound facts in five spatial commands and unconditional reflow inputs in three no-op text commands and unread current subtrees in Dev Mode resolution. Batch selection/dataflow, layout limits, text and group membership, variable defaults/aliases and consumed appearance are covered by independent-writer tests. The 300-cell constraint matrix has zero wrong writes and false refusals; other combinations remain unproven. The pinned SDK still accepts implicit-selection wrong writes, refuses unrelated address/membership changes, and cannot observe a linked library file changing by itself. |
| `design.agent.observe` An agent reads the layers and edits the right one by its numeric id | P0 | has | — | — |
| `design.agent.preview` The agent's preview is a pure function of the design and an explicit view | P0 | has | — | — |
| `design.agent.recover` An agent fixes what the check reports | P0 | has | — | — |
| `design.component.button-instances` Make a component and place renamed instances of it | P0 | has | Figma desktop 126.10.7 (macOS) | — |
| `design.layout.auto-row` Add auto layout to a frame and hug its contents | P0 | has | Figma desktop 126.10.7 (macOS) | — |
| `design.object.frames-shapes-text` Frames holding rectangles, ellipses and text, renamed and moved | P0 | has | Figma desktop 126.10.7 (macOS) | — |
| `design.page.vectors-guides` Vector paths, frame guides and more pages | P0 | has | Figma desktop 126.10.7 (macOS) | — |
| `design.paint.fill-stroke-effect-styles` Fills, strokes, drop shadows and paint styles | P0 | has | Figma desktop 126.10.7 (macOS) | — |
| `design.variable.bound` Variable collections, number variables and binding them to properties | P0 | has | Figma desktop 126.10.7 (macOS) | — |
| `design.agent.conversation` An agent makes a component and uses it over a conversation | P1 | has | — | — |
| `design.board.whiteboard` Whiteboard: sticky notes, shapes with text, connectors, widgets and data | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.comment.threads` Comments: add, reply, react, resolve | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.component.instances` Instances, nested instances and slots | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.component.manage` Components and variants | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.edit.history` Undo, redo, cut, copy, paste and other Edit menu commands | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.file.import-export` Import, place, export and save to version history | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.guide.manage` Guides | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.image.adjust` Place and adjust images; image presets | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.layout.auto` Auto layout and layout grids | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.library.publish` Team libraries: publish, add, swap, remove | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.object.arrange` Arrange, align, distribute and tidy up | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.object.frame-group` Frame, group, ungroup, mask and other Object menu commands | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.object.layers-api` Create, read, move and reorder layers by id | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.object.transform` Nudge, move, resize, rotate and flip | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.page.manage` Pages | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.paint.effects` Shadows, blurs and other effects | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.paint.fill-stroke` Fills, strokes and document colours | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.platform.display-modes` The editor in every display mode: Single line and Classic open on Edit with its most used rows; Show/Hide UI is Focus | P1 | has | — | — |
| `design.proto.interactions` Prototyping: interactions, flows, actions and conditionals | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.select.layers` Select all, none, inverse and matching layers | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.shape.primitives` Draw rectangles, ellipses, polygons, stars, lines and arrows | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.style.manage` Styles | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.text.format` Text editing and formatting: styles, lists, links, case, spacing | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.tool.canvas` Canvas tools: move, scale, hand, frame, section, pen and the others | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.variable.manage` Variables, collections and modes | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.vector.boolean` Boolean operations and Shape Builder | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.vector.edit` Vector editing: flatten, outline stroke, join, heal and point editing | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.version.history` Saved versions through the host's checkpoints | P1 | partial | Figma desktop 126.10.7 (macOS) | A host with the checkpoints capability keeps them; elsewhere the panel says it keeps none. Versions are not compared with each other. |
| `design.view.display` Rulers, grids, outlines, zoom and other View menu commands | P1 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.app.batch` Run several commands in one step | P2 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.app.preferences` Preferences | P2 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `design.devmode.handoff` Dev Mode: changes, focus view, inspect, code, measurements, annotations, resources | P2 | partial | Figma desktop 126.10.7 (macOS) | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |

| Priority | has | partial | missing |
| --- | ---: | ---: | ---: |
| P0 | 11 | 1 | 0 |
| P1 | 2 | 29 | 0 |
| P2 | 0 | 3 | 0 |
