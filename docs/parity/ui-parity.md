# UI and interaction parity

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-26 · **Target:** Figma Design UI3 (desktop app 126.10.7)

Fine canvas control is what Figma users feel first: handles, snapping, nudging, modifier keys,
drag behaviour, numeric precision. This checklist covers the tools, gestures and panels. Status:
**done** (built and checked against Figma's documented behaviour or by eye), **partial**, **missing**.
Nothing here has been compared tool by tool against the running Figma yet; that pass is the main
remaining work (40–60 h).

**Summary (estimated):** UI/UX fidelity ~58%. Layout and look of UI3 compared side by side (~65%);
interaction feel not yet compared.

## Window and panels

| Item | Status | Notes |
|---|---|---|
| UI3 layout: icon rail, file header, pages + layers, floating bottom toolbar with flyouts, Design / Prototype / Inspect panels | done | the kit's editor (`web/`) inside the host's iframe; the window, its title bar and the tabs are the host's |
| Light and dark themes | done | colours from the host's theme tokens (`web/src/app.css`); a file or a page made while the dark theme shows gets a `#1E1E1E` page (`#F5F5F5` in the light theme), its own from then on: changing the theme repaints no page and opened files keep theirs; the headless kit and exports have no theme and keep `#F5F5F5`; text drawn with the Text tool is white on a dark page and black on a light one (so is text a command makes without naming a fill; headless sessions keep black) (`theme_pages_tests`, `dark_page_text_tests`) |
| Minimize UI (Cmd+\\) | done | |
| Resizable panels | done | each panel's edge drags its width (left 200–480, right 240–480, 240 by default; a double-click on the edge or View ▸ Panels ▸ Reset panel widths puts the default back); Minimize UI hides the panels and brings them back as they were; the view keeps its centre and zoom while a panel is resized. Not compared with Figma's widths; no snap points |
| Actions palette (Cmd+K, Cmd+/) | done | tabs: All (every command, grouped by menu, or Other) and Assets (no plug-ins tab: the kit has no plug-ins); results ranked prefix first, then shortest, at most 60 rows; Up/Down move a highlight and Enter runs it; Recents list what was run from the palette (`web/src/menus/ActionsPalette.tsx`). Ranking and the Assets results not compared with Figma's |
| Context menus | partial | canvas and layers; submenus as in the main menu below, and a right-click in a new spot starts a fresh menu (Select layer closed); Shift+F10 opens them from the keyboard (at the focused layer row or below the canvas selection) and the menu takes the keyboard; Kition Design's keymap has no binding for the Menu key, so it does nothing yet |
| Main menu from the logo | done | rows from the engine's catalog (`catalog.rs`, through `query('menus')`) without the host's own items (new, open, save, close, theme, full screen, plug-ins, live sessions), plus the items the editor runs itself (`web/src/menus/catalog.ts`); submenus open after 150 ms of hover (at once by click or key) and keep their place along a safe triangle while the pointer aims at them (`web/src/ui/Menu.tsx`). Timings not measured against Figma |
| Keyboard shortcuts panel | done | docked, tabbed by group like the documentation, searchable, per-platform keycaps; held keys, gestures and presentation keys listed beside the bindings; a label too long for its column ends with "…" and shows in full on hover |
| Multiplayer avatars, Share button | missing | the Share button is disabled with a note that sharing happens in Kition, and the people menu lists only you (`web/src/shell/RightHeader.tsx`); in Kition, people co-edit a design through its KDM replica ([collaboration-parity](collaboration-parity.md)) |
| Colour names in hex fields | done | every hex field (fills, strokes, text ranges, effects, layout grids, colour variables, page and prototype backgrounds, the colour picker's Hex) takes a CSS colour name as well: `red` becomes `FF0000`; case, spaces and hyphens are ignored (`Light Blue`); names are opaque and keep the colour's opacity; hex wins where both could match. The picker's CSS field takes every CSS name too (`Color::from_hex_or_name`, `color::named`) |
| Fields select their text on focus | done | a click or Tab into a one-line field selects all of its text, so typing replaces the value: hex and colour fields, number fields, inline fields (a fill's hex, variable values, renames), names, search boxes and dialog fields; a drag from the press selects only what it covers; multi-line fields (descriptions, comments) keep the caret where clicked. The first click into an inline field (a fill's hex) now takes the keyboard; it took a second click. A rename (double-click, F2, File ▸ Rename, a page) selects the whole name before its field first draws, so a key typed in that same frame replaces the name |
| Colour picker models | done | Hex, RGB, HSL, HSB, LCH, OKLCH and CSS; LCH and OKLCH keep lightness and hue when a colour leaves sRGB (chroma comes down); the values typed stay while the colour is theirs (a hue typed on a grey, a chroma sRGB can't hold); CSS reads `lch()` and `oklch()` with any hue unit and `none` (`kition_design_color::{Lch, Oklch}`). Values are the stored channels read as sRGB |
| Colour picker on a new fill or stroke | done | + opens the picker on the new paint when the selection shares one paint list; on a mixed selection + adds a paint to each layer and no picker opens |
| Blend mode menus: the layer's (Design panel) and each paint's (colour picker) | done | Pass through and Normal are distinct (Normal isolates a group's or frame's children); paints have no pass-through; plus darker drawn exactly on the canvas |

## Layers and Assets panels

| Item | Status | Notes |
|---|---|---|
| Rows: click, Shift-click range, Cmd/Ctrl-click toggle; the selection follows on the canvas and a canvas selection is revealed | done | |
| Expand and collapse: Alt-click opens or closes everything inside; Collapse layers (Alt+L) keeps the way to the selection open | done | Alt-click pinned (the documentation is silent) |
| Drag to reorder and into frames, groups and auto layout: insertion line, spring-loaded open, several layers together, Escape cancels | done | several layers dropped together land together; instances and a layer's own children refuse drops |
| Eye and padlock: click, or drag across rows to set several in one step; layers in a locked frame show a faint padlock | done | the toggles stay under the pointer while it is over them |
| Rename: double click, F2 / Cmd+R, Tab to the next row; several layers open Rename layers (match, regular expressions, current name and number codes, start value, preview) | done | numbered in stacking order, pinned |
| Find layers (filter by name) | done | not covered by the documentation |
| Pages: add, rename, drag to reorder, duplicate ("Copy of"), delete, dividers (a blank page named with a hyphen) | done | the line between Pages and Layers drags to size the pages list, kept in the preferences; a long list scrolls in its own area and a page dropped outside the visible list stays put; + opens the new page's name for typing; each page keeps its view while the document is open (`web/tests/hand/keys.mjs` `E10`, `view.mjs` `B14`) |
| Assets panel: folders from page, frames and slash names (fold away), search, thumbnails, drag onto the canvas or into frames and auto layout, Alt-drop swap, enabled libraries | done | Shift+I (Quick insert) and the toolbar's Resources open it; same-named components of this file on different pages show their page's name (`gestures.mjs`) |
| Hide when publishing from the Assets panel's context menu | done | a right-click on one of this file's components (also Libraries ▸ Hide when publishing for the selection, `component.hideWhenPublishing {ids?, on?}`): the model's sign of it is the name, so hiding puts a `.` before the name and showing takes the leading `.` and `_` away (one undo step, saved and synced like a rename; a variant or a layer inside hides its whole component or set); a hidden component shows a crossed eye on its tile and a dimmer name, the menu item its check mark; the next publish leaves it out and brings it back when shown again (`hide_publishing_tests`). Library components (read-only here) have no such menu |

## Selection and transform

| Item | Status | Notes |
|---|---|---|
| Click, Shift-click, marquee, Shift-marquee toggle | done | Shift-drag on a selected layer moves the selection, kept to one axis; a marquee selects what it touches; from a top-level frame's background, the layers inside it; locked and hidden layers are skipped (`selection_behaviour_tests`) |
| Deep select (Cmd/Ctrl-click, Cmd/Ctrl-marquee) | done | |
| A press inside the selection drags it; a click there picks the layer pressed | done | with a frame selected, a drag from a layer in it moves the frame and a click selects that layer; a click on one of several selected layers keeps just it (`web/tests/hand/gestures.mjs` `press in the selection`) |
| Double-click an auto layout edge: Hug (Alt: Fill) | done | the left or right edge sizes the width, the top or bottom the height; one undo step (`gestures.mjs` `double-click an edge`) |
| Select layer menu (every layer under the click) | done | |
| Enter / Shift+Enter (children / parent), Tab / Shift+Tab (siblings) | done | Tab renames the next layer while renaming (`web/tests/hand/keys.mjs` `E4`, also in WebKit) |
| Resize handles: Shift keeps proportions, Alt from centre, constraints-aware | done | Alt shrinks too; past an edge flips (several layers mirror); groups and mixed rotations scale each layer along its own axes, keeping rotation and flip (no skew: children turned off a right angle are approximate); Cmd/Ctrl ignores a frame's constraints (`transform_behaviour_tests`) |
| Rotate just outside a corner; Shift in 15° steps | done | rotated handle box when layers share a rotation |
| Flip, rotate 90° | done | |
| Rotation origin (Alt+R) | done | a handle at the selection's centre to drag (snaps to the box's corners, edge middles and centre), double-click puts it back, Escape or Alt+R hides it; the corner handles, the rotation field, Rotate 90°/180° and `transform.rotate` turn about it; it follows the layers and lasts as long as the selection, not saved (the documentation doesn't say; pinned) |
| Enter with a shape or frame tool chosen | done | places what a click makes (100×100, lines 100 long, sections 400×300) in the middle of the view, selected (`tool.place`); from the toolbar, Enter on the chosen tool. No crosshair to move with the arrows first |
| Alt-drag duplicate | done | the copy snaps to the layer it came from too (edges, centres, equal spacing); Duplicate (Cmd+D) then repeats the copy's offset, also after a Duplicate whose copy was moved, as long as the copies stay selected (the documentation doesn't say; pinned from community answers); Alt pressed or let go during the drag switches between a copy and a move, the release decides; a variant Alt-dragged out of its set is an instance of it (inside the set's box, a new variant) (`web/tests/hand/gestures.mjs` `alt during a drag`, `variant out of its set`) |
| Drag threshold; double-click into groups and text | done | a press moves or resizes only once the pointer has travelled 3 px on screen, so a click or double-click made with an unsteady hand drags nothing; a double-click goes one level into a group and the next one edits the text under it, wherever on the text it lands, with all of it selected (a double-click in the text being edited selects the word) (`double_click_tests`); Enter on a selected text does the same (all of it selected, typing replaces it) and Escape leaves editing with the layer still selected |
| Scale tool (K): handles about an anchor, live factor, panel | done | |
| Nudge 1 / Shift 10 (preference), Cmd+arrows resize | done | arrows reorder auto layout children |
| Smart guides: edges, centres, equal spacing (between two layers or continuing a row's gap, labelled), spacing labels, parent, layout grid, ruler guides | done | red, `snap.rs`; thresholds not yet compared with Figma's |
| Ctrl to suspend snapping while dragging (macOS) | done | Control on macOS (Cmd keeps working), Ctrl elsewhere; moving, resizing and drawing |
| Measure (Alt hover, Cmd+Alt nested, Shift+Alt padding/gaps) | done | labels never overlap; the arrows still nudge while measuring (Shift: the big step) and the distances follow; the selection's handles and size label hide while measuring; the distances and the hover outline go when the pointer leaves the canvas (`web/tests/hand/gestures.mjs` `leave`) |
| Pixel grid snapping, pixel preview, 2x preview | done | moves, resizes and new shapes land on whole pixels; the pixel grid shows from 400% (`view_behaviour_tests`) |
| Numeric fields: arithmetic, scrubbing by dragging the label, mixed values | done | Typography line height, letter spacing and paragraph spacing scrub from their leading icons like the other number fields, keeping their unit parsing, Auto/Mixed values, variable chips and selected-character targeting. Scrubbing the label: a step a point (1 px; 1% of opacity, 1° of rotation), ten with Shift, a tenth with Alt, each stretch at the rate held while it was dragged, one undo step when let go; the pointer is locked while scrubbing, so it goes on past the edge of the screen (`web/src/ui/NumberField.tsx`). Arrow keys: Up/Down step a pixel field by the small nudge and Shift by the big nudge (the preferences), other fields by their own step and ten of it; they work while typing (from what is typed) and with the keyboard on the field. Typing: `+ - * /`, brackets, units (`px`, `°`, `%`, `x`, `ms`), a decimal comma; `+10`, `*2`, `/4` are relative to the value, and `50%`, `+10%`, `-10%` too in a field that isn't a percentage. Enter, Tab or a click elsewhere commits (Tab and Shift+Tab move to the next and previous control), Escape restores; a field left as it was commits nothing (no undo step, and a rounded display isn't written back). Mixed values show "Mixed": a number sets them all, a relative edit or an arrow key goes to each layer from its own value in one undo step, for X, Y, W, H, rotation, opacity, corner radius and stroke weight (elsewhere `+10` on Mixed sets 10 and `*2` does nothing). Left: scrubbing a Mixed field (nothing happens), `x2`-style suffix edits, the pointer isn't wrapped to the other side of the screen; rates not compared with Figma's |
| Transform box at small sizes | done | a press inside a layer smaller than its handles moves it, the nearest handle wins, handles answer just outside and are hidden under 16 px on screen; cursors follow each handle and the box's rotation |
| Locked layers | done | selectable from the layers panel; no handles, not dragged with the rest of a selection |
| Smart selection spacing | partial | with layers in a row, a column or an aligned grid selected, the Layout section shows horizontal and vertical spacing ("Mixed" until set; typed, scrubbed, fractional and negative) and pink handles in the gaps on the canvas drag them, one undo step, Escape cancels (`arrange.spacing`; sizes, parents and offsets within each track kept; at most 2,000 layers; flow children, locked layers and layers inside instances left out). No reorder rings for swapping layers; checked by its author on macOS and Windows |
| Selection colors | partial | for a mixed selection the Design panel lists their solid and gradient paints (children and text ranges included; hidden paints, masks, images, patterns and boolean operands left out; styles and variables listed apart): editing a colour changes every matching fill and stroke in one undo step, a row selects the layers that use it or applies a color style or color variable of the document, and a row linked to a style or variable detaches it, keeping the color (`selection.colors`, `selection.setPaint`, `selection.applyColorStyle`, `selection.bindColorVariable`, `selection.detachColor`, `selection.selectColor`). No library styles or variables from there, and no new style or variable from a row |
| Keyboard box selection | missing | the engine has `select.area` (a box around the selection or in the middle of the view, moved and sized by the arrows, Enter selects), but the editor draws no box for it: its keys (Alt+Space / Ctrl+Space, F8) are not bound and the shortcuts dialog does not list it (2026-11-26) |

## Tools

| Tool | Status | Notes |
|---|---|---|
| Move, hand (Space), zoom (hold Z, Alt out, drag area) | done | |
| Frame (presets panel, swap orientation), section, slice | done | |
| Rectangle, ellipse (arc handles and Design panel arc fields), polygon, star, line, arrow; Shift/Alt | done | Alt draws lines from the centre too (`shapes_behaviour_tests`) |
| Corner radius handles (inside each corner of a rectangle or frame, a polygon's or star's top point; Alt: one corner), star ratio handle; Count and Ratio fields | done | handles hide under 48 px on screen; no count handle on the canvas (the Count field instead); corner smoothing rounds rectangles and frames only |
| Pen: vector networks, branch, close, bend; pencil | done | pen pressure where the browser reports it with a pointer event; outside edit mode the Pen closes on its first point, connects to its other points and continues a selected open path from an end point (`vector_multi_behaviour_tests`); Escape or Enter ends the path and keeps its layer in edit mode with the pen, so the next path joins the same vector and another Escape leaves edit mode (`pen_end_tests`); switching from Design to Prototype or Inspect ends the path and returns to Move |
| Vector edit mode: bend, paint bucket, lasso, cut, eraser, transform box, point snapping, mirroring | done | a double-click a few pixels off a thin stroke enters edit mode; entering edit mode and picking a region keep the canvas still; a point dropped on another point merges with it, and dropping one end of an open path on the other closes it (a 2-point line's end just moves); Flatten keeps open paths open; paint bucket drags across areas; cut drags divide a shape into layers; delete and heal follows the curve; rounded points are drawn; Shift-click deselects a point at a corner of the box (`vector_behaviour_tests`); several vectors edited at once, Shift-click handles to drag them together, a cap and join per point, rounded points beside curves (`vector_multi_behaviour_tests`); filled regions with the Move tool: the area under the pointer is hatched ("/" lines in the selection colour, clipped by the area's own fill rule, holes left out, gone 3.5 px from a point or 2 px from a segment at any zoom), the line spacing follows octave folding (fitted to measurements at several zoom levels: 8 px at every power-of-two zoom, growing with the zoom and stepping back at √2 · 2ⁿ) and stays fixed to the shape when panning; a click selects the area and its points (moved with it but drawn unselected), Shift adds or removes an area, a press on a selected area drags the whole selection, no hover hatch during drags, a click on an empty spot (a hole) clears it and stays in edit mode; `vector.edit.select {regions}` (`regions_tests`, `vector_regions_tests`). Not yet: areas of other layers edited at once |
| Shape builder (Shift+M) on fills, strokes and open paths; variable width (Shift+W), one side with Alt | done | |
| Boolean groups: clicked where the result is, double-click or Cmd/Ctrl-click into operands, live results while dragging or editing points, per-operand operations | done | |
| Text: point, auto height, fixed; caret, words, lines, IME | done | the font picker previews a family on the canvas while hovered (no undo step; Escape restores) and Up/Down step through it; a selection highlight after a line break starts at that line's start and an empty last line shows its mark; carets step over whole emoji and accented letters; colour emoji draw from their colour tables; IME composition replayed in tests as macOS sends it, not yet checked on a Mac; CJK words by script run (no dictionary); vertical trim in Type settings; no spell check |
| Eyedropper (I) | done | picks the colour seen under the pointer (opaque) into the selection's fill and the recent colours, one undo step (`fx_behaviour_tests`) |
| Comment (C), Shift+C to show/hide | done | local only |
| Text on a path (Figma Draw) | done | the Text on a path tool in the Text flyout (`tool.textOnPath`) shows the I-beam over a shape's or vector's path, and a click there puts new text on the path, starting at the click, with the path's effects and its fill when one shows; the path layer is removed, and the path comes back if nothing is typed. Also from the Design panel or `text.onPath`; the path is edited by resizing only |
| Stretch and scatter brushes (Figma Draw) | done | eight presets drawn in code; the Pencil's brush is set by `tool.pencilBrush`, not from the toolbar |
| Image crop handles | done | |

## Canvas

| Item | Status | Notes |
|---|---|---|
| Pan/zoom: wheel, pinch, Space-drag, Shift+0/1/2, Cmd+0, `+`/`-` | done | a mouse wheel's notch with Cmd (Ctrl off the Mac) zooms 1.2 times around the pointer, a trackpad's and a pinch's small steps zoom smoothly as before; the first layer drawn on a new page stays where it was drawn while the page's area grows round it (`web/tests/hand/view.mjs`); Shift-wheel pans sideways, pinch and the zoom tool keep the point under the pointer; zoom steps, limits (2%–25,600%) and frame order pinned where the documentation is silent (`view_behaviour_tests`) |
| Drag and hover cost (one engine trip a sample, panels held during a gesture, only the changed area repainted) | partial | a drag sample sends at most 4 worker messages (the pointer, kit-editor's tool state and disabled commands, a render of the area it changed) and no panel query; each render draws at most a quarter of the view and the canvas then shows what a whole render shows; the Design panel's X, Y, W, H and rotation and the frame labels and size pill follow the drag from the engine's answers; a hover over the same layer sends only the pointer and renders no panel (`web/tests/hand/roundtrips.mjs`, `bindings/wasm/tests/damage.rs`); zoomed out, a frame is drawn at its scale, not at 1:1 and reduced. 79 repaints a second for the 348-layer sample at 50 %, 58 for 2784 layers (1x, loaded machine). A fast pan of 2784 layers still waits for new frames, and whole repaints still draw four times the view (kit-editor) |
| Rulers and guides (drag out, move, remove; rotated frames; frame guides) | done | guides belong to the page or a top-level frame (a frame moved into another keeps its guides but they stop applying); a click selects one for Delete, the arrows or the context menu's Remove guide; Alt-drag drops a copy; Alt while dragging measures to the layers in the frame it crosses; ruler numbering checked by eye only |
| Frame labels (rotated, right-to-left), size badge | done | |
| Outline mode | done | Outlines, outlines for the selection, mask outlines, frame outlines; shapes are clicked on their outlines, reaching layers behind; View ▸ Outlines ▸ Include hidden layers (`view.outlinesHiddenLayers`) outlines hidden layers too and Include object bounds (`view.outlinesObjectBounds`) draws the box of every layer whose outline isn't its box (groups, text, ellipses, vectors); what either draws can be clicked there (a hidden layer on its outline, a group on its box) and nothing else can; both are view settings like the other outline flags (per file, off in a new one, checked in the menu); the lines are light on a dark page (`outline_options_tests`). The main menu now shows a check mark beside every View toggle that is on |
| Slices shown on the canvas (View ▸ Slices) | done | dashed rectangles |
| Dragging a layer inside a group keeps it in the group | done | frames and auto layout take other drops |
| Frames and sections drawn around layers take those fully inside; sections never drop into frames | done | not in auto layout frames |
| Resizing a section leaves its contents in place | done | |
| Masks: what a mask hides doesn't take clicks; a mask group's box is the mask's | done | |
| Layout grids (Ctrl+G) | done | columns, rows and grid; stretch, left, centre, right; Auto count; several per frame; styles; not shown or snapped on a rotated frame; a nested frame's grids snap but only top-level frames draw them |
| Gamut warnings for Display P3 files | done | colour picker note; View ▸ Soft-proof sRGB stripes what sRGB can't show (the canvas itself is sRGB) |
| Gradient and padding/gap handles on the canvas | done | |
| Hover outline, selection colours, handle sizes | done | measured from UI3 screenshots |
| Cursors per tool and modifier | done | the engine names a cursor for every tool, modifier and hover target (`tool.overlay.cursor`, tabled in `cursor_tests`) and the editor shows it at the pointer. Arrow for Move and Scale; two-headed arrows on the handles that follow the handle and the box's rotation; outside a corner a curved rotate arrow turned to that corner, turning with the pointer while rotating; crosshair for the frame, section, slice, shape, pencil and Text tools; the I-beam over a text with the Text tool and over the text being edited; the pen as a cross with what a click does beside it (nothing: a new point; a ring: closes, connects to or goes on from a point; a plus: adds a point on a segment, also Alt in vector edit mode; a curve: bends, the Bend tool or Cmd/Ctrl); open and closed hand for the Hand tool, Space and the middle button; zoom in and out (Alt) while Z is held; eyedropper; comment pin; the copy cursor with Alt over a selected layer and while Alt-dragging; not-allowed while a drag resizes, rotates or moves a layer inside an instance (it is undone on release) and over a layers row that takes no drop. Ruler guides and gradient, crop and padding handles keep their own cursors (the tool's used to replace them). Left: no cursor for removing a point with the pen (the pen doesn't remove points); not compared with Figma's side by side |

## Keyboard shortcuts

One keymap (`kition_design_engine::keymap`) drives the menus, the canvas and the shortcuts panel. `crates/engine/engine/src/shortcut_parity_tests.rs` holds one row per
shortcut in the public documentation (the help centre's feature articles: keyboard layouts, text
properties, alignment, frames, components, prototypes, plugins, keyboard navigation…), with the
macOS and Windows/Linux keys, what does it here and a note in our words, and checks every row on
both platforms; rows the documentation is silent on pin today's keys, so every default binding is
in the table.

| | Count | Notes |
|---|---|---|
| Documented shortcuts (rows) | 129 | each with macOS and Windows/Linux keys |
| Bound as documented | 128 | 108 keymap bindings, 14 held keys, gestures and presentation keys, 6 vector edit tool keys |
| Bound on other keys (documented conflict) | 1 | Run last plugin: the documentation also gives Ctrl+Alt+P to pixel preview on Windows; the plugin takes Ctrl+Alt+Shift+P there |
| Missing | 0 | moving the rotation origin (Alt+R, `transform.rotationOrigin`) and placing an object with Enter after choosing its tool (`tool.place`) are bound too |
| Documented keys a platform keeps | 2 | Ctrl+Space (Windows/Linux keyboard box selection) switches some CJK input methods; F6 goes to a browser's address bar |
| Undocumented bindings pinned | 57 | of which Cmd/Ctrl+W, Cmd/Ctrl+N and Ctrl+Tab are taken by browsers and Ctrl+Alt+T / Ctrl+Alt+L by some Linux desktops |

- **Bound from the documentation:** A for the Frame tool, Shift+' for the pixel grid (Cmd+' kept: the articles
  disagree), Alt+Cmd+\ / Ctrl+Alt+\ for others' cursors (`view.multiplayerCursors`), Ctrl+F6
  to the toolbar on Windows/Linux, Shift+Cmd+U to link text (Cmd+K kept), Cmd/Ctrl+] and [ to
  nest list items while editing (`text.listIndent` / `text.listOutdent`), Cmd/Ctrl+Backspace
  to ungroup, Ctrl+Shift+A / Ctrl+Alt+Shift+A to suggest auto layout, Ctrl+Alt+Cmd+K /
  Ctrl+Alt+Shift+K to go to the main component, the arrows panning the view 64 px (Shift: 320 px) when
  nothing is selected, Control suspending snapping on macOS, and Space, N, C and F while
  presenting. The paint bucket's tooltip shows Shift+B (B still works).
- **Paste:** Shift+Cmd/Ctrl+V (Paste over selection) and Alt+Cmd/Ctrl+V (Paste properties) act
  as bound; Cmd/Ctrl+V pastes an image copied in another app.
- **Where the documentation disagrees with itself** (noted per row): pixel grid (Shift+' or
  Cmd+'), letter spacing (Alt or Cmd with < >; Cmd would be the font size keys, Alt bound),
  Windows remove fill (Alt+/ or Alt+,; Alt+, is letter spacing), the Layers panel on Windows (Alt+1
  or Ctrl+1), send to back / bring to front (one table shows only the bracket).
- **Platform keys:** macOS rows use Cmd and Option, Windows/Linux rows Ctrl and Alt (tested: no
  macOS glyph on Windows/Linux keycaps, no "Ctrl"/"Alt" words on macOS); no default binding uses a
  chord the system keeps (Cmd+H/M/Q/Tab/Space, Ctrl+Space and the screenshot keys on macOS;
  Alt+Space/Tab/F4, Ctrl+Esc, Ctrl+Alt+Delete and Linux workspace and terminal-switch keys).
- **Keyboard layouts:** keys follow the character typed, named as on a US keyboard. A symbol
  another layout types with Shift (`/` is Shift+7 on a German keyboard) counts without the
  Shift. Not done: per-layout tables like the documentation's (a German Shift+' types `"` and
  reaches Shift+2).

## Accessibility (ours vs Figma's)

| Item | Status | Notes |
|---|---|---|
| Keyboard navigation: F6 regions, Tab within, focus ring | done | |
| The keyboard is the canvas's after a field, popup, menu, palette, comment, window or toolbar button; from the first key after opening | done | Enter or Escape in a field, a closed popup, menu, Actions palette, dialog, variables window or comment composer give the keyboard back to the editor; a key that still finds the page's body holding it is given to the editor once; a posted comment's thread opens without taking the keyboard; Escape after clicking a toolbar tool goes back to Move; a layer row keeps the arrows and leaves the tool letters to the tools (`web/src/kit/focus.ts`, `web/tests/hand/keys.mjs`) |
| Every binding's second keys, the number keys for opacity, N / Shift+N, a double-click on a frame label | done | A for the Frame tool and every other second key of the keymap (`keymapKeyDown`); 1…9 / 0 and two quick digits set the selection's opacity; a double-click on a frame's or section's name label renames it in a field over the label (Enter, a press elsewhere; Escape keeps it); the toolbar shows a tool a command chose (`web/tests/hand/keys.mjs`) |
| Typing a new text on the canvas | done | Cmd+A selects its characters (not the page's layers) and Cmd+B bolds them; Copy, Cut and Paste act on the selected characters as plain text and add no layer (the editor's own text clipboard: the system clipboard is not read yet); a triple-click selects the paragraph; T, a click, typing, Escape and one Cmd+Z take the new text away; an unnamed text is named after its first line (at most 64 characters) until it is renamed; the tools' letters work as soon as editing ends (`web/tests/hand/text.mjs`) |
| Layers panel from the keyboard: arrows, F2 / Cmd+R rename (the row gets the keyboard back), Shift+F10 context menu; 10k-layer lists | done | |
| Reduced motion | done | the system setting (`prefers-reduced-motion`) |
| Tested with VoiceOver, NVDA, Narrator, Orca | missing | human QA |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
