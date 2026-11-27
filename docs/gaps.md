# Gaps

> **Last updated:** 2026-11-27 · **Source:** `features.yaml` (updated 2026-11-27), the engine's command list, and the editor checked by hand (`web/tests/hand/`, plan 2026-11-26)

Every known shortfall of Kition Design, from the feature catalogue: features that are not yet `has`,
then the differences recorded on features that work. Update the feature in `features.yaml` when you
land work, then this list. The comparison with the target application is in
[parity/target-app-parity.md](parity/target-app-parity.md); the order of work is in [roadmap.md](roadmap.md).

## Features not done

- `design.agent.ids` (P0, partial): Stable tool ids and typed command references for layers and resources. An independent reading covers all 372 write handlers; 368 commands are mechanically compared across 803 paths (four widget commands need an unavailable plugin). The gate permits zero held-less commands and twelve held-more commands: import name membership, page-window occupants, prototype mode-reference replay, painted-bound facts in five spatial commands and unconditional reflow inputs in three no-op text commands and unread current subtrees in Dev Mode resolution. Batch selection/dataflow, layout limits, text and group membership, variable defaults/aliases and consumed appearance are covered by independent-writer tests. The 300-cell constraint matrix has zero wrong writes and false refusals; other combinations remain unproven. The pinned SDK still accepts implicit-selection wrong writes, refuses unrelated address/membership changes, and cannot observe a linked library file changing by itself.
- `design.editor.first-ten-minutes` (P0, partial): A designer's first ten minutes by hand in the editor: the thirty things of the acceptance list and six more, and a whole phone login screen. Checked in `web/dev.html` with real pointer and keys in Chromium and WebKit: 29 of the 30 and the 6 more pass; the login screen takes 101 actions and gets stuck no time (131 and 9 on 2026-11-26). A marquee from the canvas that touches part of a top-level frame selects nothing (the reference selects the frame): the engine's selection behaviour suite pins today's rule, and changing it needs a maintainer. The new file and the first save's file name are the host's (`dev.html` has no new file). A real mouse, trackpad and input method, and Kition Desktop's frame, are not checked.
- `design.editor.input-method` (P0, partial): Chinese, Japanese and Korean input on the canvas: a composition shows at the caret, underlined, without changing the document; the candidate window opens at the caret; the keys of a composition are the input method's; the text keys move by word as the platform's do. Checked in `web/dev.html` with Chromium's DevTools input method and real typing; a real input method on a Mac or Windows, and Kition Desktop's frame, are not checked. The input method's cursor inside a composition shows at its end. kit-editor's text sink still gives the editor's commands a composition's keys and sends no composition updates; the editor stops the keys and sends the updates itself.
- `design.editor.gesture-cost` (P0, partial): A drag sample and a hover cost one trip to the engine and repaint only what they changed: the panels hold their queries while a pointer gesture runs and ask once after it, the Design panel's X, Y, W, H and rotation and the canvas chrome follow the drag from the engine's answers, a hover tells the editor only when the hovered layer changes, and the hover's outline and the distances Alt measured go when the pointer leaves the canvas, nothing re-renders the whole editor for an engine answer, every tool event's answer names the area it changed so the canvas repaints that area of its frame, and a zoomed-out frame is drawn at its own scale. Measured on a loaded machine (1-minute load 24-105), 1x, 50 %: dragging a rectangle paints once per pointer event, 47 times a second, the next paint 7 ms after it (348 layers), 48 times and 19 ms (2784 layers); at 2x, 2784 layers, 17 times and 78 ms. A fast pan no longer waits for a new frame (2784 layers: 0 of 124 moves without a paint for 50 ms; at 2x, 63 paints a second). Outline mode repaints the whole view for every edit. A patched curved edge can differ by a few levels of anti-aliasing from a whole render until the next one.
- `design.editor.system-clipboard` (P0, partial): Copy, Cut and Paste use the system clipboard: an image, SVG text or text another app copied pastes as an image layer, an editable frame or a text layer; the editor's own copy of layers pastes as layers; characters copied in a text reach other apps and text from them types into a text. Checked in `web/dev.html` with the browser's paste and copy events and, in Chromium, real Cmd+C and Cmd+V; Safari and Kition Desktop's frame are not checked. The SVG or text other apps get follows the editor's mark a moment after the copy, through the asynchronous clipboard. The menus' Paste reads the system clipboard only where the browser allows it without asking. An image pasted over one selected layer makes a new layer rather than that layer's fill; layers pasted into another document arrive as their SVG.
- `design.editor.file-drop` (P1, partial): Images and SVG files dropped on the canvas are imported where they land, with a hint while they are over it; Choose image and Local variables' Import and export go through the host. A real drag from the desktop and Kition Desktop's frame are not checked; a dropped document is not opened (the Kit UI protocol has no request for it), the editor says to open it from Kition; an image dragged onto the colour picker's image area does not replace it.
- `design.editor.canvas-picking` (P0, partial): What a press, a drag and a double-click on the canvas pick and do is what a person expects, and the panels keep their place: a press inside the selection drags it and a click there picks the layer pressed, Alt during a drag switches between copy and move, Alt+dragging a variant out of its set places an instance, a double-click on an auto layout edge hugs (Alt: fills); Tab in a layer rename goes on to the next row; a new page opens its name; each page keeps its view; Assets names the page of same-named components. A marquee from the canvas that touches part of a top-level frame selects nothing (the reference gives the layers in it the marquee touches): the engine's selection behaviour suite pins today's rule, and changing it needs a maintainer. A click on another branch after a deep selection selects the top-level frame's child, not the sibling of the selection's ancestor; Cmd+drag from a frame's background drags that frame rather than starting a deep marquee; the inside of an auto layout frame without a fill is not clickable: all three as the reference does them. Add variant grows the set over the layers below it, as the reference does.
- `design.editor.canvas-input` (P0, partial): The canvas's input is one layer in a fixed order (prototype connections, zoom tool, comments, ruler guides, handles, then the tool); holding Z zooms (click, Alt+click, drag an area); a drag held at the edge pans the view; modifiers pressed with the pointer still reach the tool; ruler guides drag out, snap, move, copy with Alt and go back to the ruler, and a selected one takes Delete and the arrows; Paste here away from a selected frame pastes on the page. Checked in dev.html with real pointer and keys in Chromium and WebKit; a real mouse and trackpad by hand and Kition Desktop's frame are not checked. Alt while dragging a guide does not measure to the layers it crosses. A modifier pressed while hovering reaches the tool after 120 ms and not at all when it is part of a shortcut (kit-editor gives a key pressed while the engine answers back without its key code, so Shift+1 pressed during that answer would not fit). The mouse's side buttons do not pan; there is no preference for the wheel to zoom.
- `design.editor.auto-layout-handles` (P1, partial): Auto layout is adjusted on the canvas: a padding band dragged sets its side (Alt or Shift: the opposite side too; both: all four), a click on a band types its value, an Auto gap dragged or typed becomes a fixed gap, a padding field takes two values at once (10, 30), a child dragged past its neighbour reorders the layout. Checked in dev.html with real pointer and keys in Chromium and WebKit; a real mouse and trackpad by hand and Kition Desktop's frame are not checked. A drag starts anywhere on a band (the reference starts it only on the band's 20 x 12 px grip). The handles show for one selected, unrotated auto layout frame with the Move tool, as the reference's do; grid tracks have no handles of their own. There is no clickable constraints diagram beside the two constraint dropdowns (the reference has the dropdowns only).
- `design.editor.vector-editing` (P1, partial): Vector paths are drawn and edited by hand: the pen closes, curves and ends a path with Enter or a double-click (no doubled point; edit with the Move tool, Enter again leaves), undo while drawing takes back one anchor; in edit mode V, P, B, Q, X, Shift+E, Shift+W pick the tools without leaving, Cmd+A selects every point, the pen adds a point on a segment, Cmd bends, a click on another layer leaves while a drag there marquees; the Design panel shows the points' X, Y, size, radius, mirroring, cap and join; the pencil keeps every sample. Checked in dev.html with real pointer and keys in Chromium and WebKit; a real mouse, a pen tablet and Kition Desktop's frame are not checked. Several selected points show the top-left of their box as X and Y (the reference shows each point's own value, or Mixed, and applies +10 to each); the panel cannot align or distribute points. The Variable width tool's width point has no fields. The pencil has no brush in the toolbar (tool.pencilBrush only) and its stroke is black on dark pages too. Fill rule and reverse path have no controls; the Scale tool has no panel or anchor. Behaviours of the reference not ported: handles made by a mirror mode on a sharp point, and loops closing through a bridge segment after a point between was deleted.
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

## What the editor still lacks by hand

The hand-use plan of 2026-11-26 (stages 1–11) made the editor usable with the mouse and keys; these
are what it left, cut, or handed to the owner. Each is also on its feature above or below.

Not done or cut:

- A marquee from the canvas that touches part of a top-level frame selects nothing; the reference
  selects the frame (the engine's selection suite pins today's rule; a maintainer decides).
- The keyboard marquee (`select.area`, Alt+Space / F8) is not offered.
- The zoom goes from 1 % to 6400 % (kit-editor's canvas); the reference goes from 2 % to 25,600 %.
- Smart selection spacing has no handles in the gaps on the canvas (the Layout section's fields
  only); grid tracks have no handles; padding and gap bands drag from anywhere on the band.
- The Scale tool has no panel and no anchor; the pencil's brush has no toolbar entry; the
  Variable width tool's point has no fields; fill rule and reverse path have no controls; several
  selected vector points show their box's top-left, not Mixed, and cannot be aligned or
  distributed.
- No clickable constraints diagram; no undo history panel, component playground, nested instance
  properties or glyph panel.
- Alt while dragging a ruler guide does not measure; the mouse's side buttons do not pan; there is
  no preference for the wheel to zoom; no coarse preview while zooming.
- Copy and paste between two documents carries SVG, not the layers; an image pasted over one
  selected layer is a new layer, not its fill; a document dropped on the canvas is not opened; an
  image dropped on the colour picker does not replace its image.
- Typing `1. ` or `- ` does not start a list; a code block does not exist in the text model; an
  input method's own cursor inside a composition shows at its end.
- Libraries are not updated item by item; variables are not reordered by dragging; style folders
  are not deleted.
- The Windows and Linux keys and Firefox are not checked by hand.

Waiting for the owner:

- Kition Desktop: whether the editor has the keyboard as soon as a design opens, whether its frame
  may read the system clipboard and take dropped files, and where an input method's candidate window
  opens there (only `web/dev.html` is checked).
- By hand on real devices: an input method on a Mac and on Windows, a trackpad's pinch and inertia,
  a real mouse wheel, Shift+wheel, copy and paste between apps, a file dragged from the desktop.
- kit-editor reports the view before it put the frame on whole device pixels (`view()`,
  `onView`); the editor corrects it (`placedView`) until kition-sdk is fixed, which needs an existing
  kition-sdk test changed. The editor's `onDevicePixels` stays for the same reason (removing it
  changes the steps of `regressions/issue-hand-m5-device-pixels`).
- A key pressed while the engine answers is replayed by kit-editor without its key code (a
  modifier pressed while hovering reaches the tool 120 ms later, not at all when part of a
  shortcut).
- A mixed stroke weight scrubbed in CI twice gave one layer about +1363 (`test:paint`, Chromium);
  not reproduced here (`test:scrub-repeat` watches it).

## Differences on features that work

- `design.editor.keyboard` (P0, has): measured in `web/dev.html` only; whether Kition Desktop gives the editor's frame the keyboard when a design opens is not checked here. The keyboard marquee (`select.area`, Alt+Space / F8) is not offered in the editor: its keys are not bound and the shortcuts dialog does not list it. In a design file the dialog lists no whiteboard tool (their keys work on boards only).
- `design.editor.text-and-view` (P0, has): Measured in `web/dev.html` with synthetic wheel events: a real mouse wheel and a real trackpad are not checked. At zooms whose device scale is not a whole number (150 % on a 1x screen) the canvas can still copy its frame half a pixel off: kit-editor places the frame, the editor only moves the view.

## Catalogue

The catalogue has 59 features: 18 `has`, 41 not yet.
