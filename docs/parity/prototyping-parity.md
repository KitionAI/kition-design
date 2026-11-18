# Prototyping parity

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-17 · **Target:** Figma Design (desktop app 126.10.7)

**Breadth ~92%, ready ~74% (estimated).** Every Figma trigger except media, every action except
media control, every transition and easing exist; key/gamepad triggers take controller buttons
(the kit's editor reads no controllers yet). Every rule of the public
documentation on triggers, actions, overlays, animations, easing, scrolling and interactive
components has a row below. The rules marked pass are test cases on the engine's model
(`crates/engine/engine/src/proto_behaviour_tests.rs`, 7 tests; the tail:
`crates/engine/engine/src/proto_tail_tests.rs`, 4 tests; and the other `proto_*_tests`), and all
pass; the rules marked not tested have no case in this repository yet, and the kit's player
(`web/src/prototype/`) is not yet tested against any of them. Not yet checked against larger
real prototypes. Remaining: 2–5 h.

| Group | Figma | Kition Design | Missing |
|---|---|---|---|
| Triggers | On click, on drag, while hovering, while pressing, key/gamepad, mouse enter, mouse leave, mouse down, mouse up, after delay, on media hit, on media end | 10: OnClick, OnHover, OnPress, OnDrag, MouseEnter, MouseLeave, MouseDown, MouseUp, AfterDelay, KeyPress (keys, combinations and controller buttons) | media triggers; reading controllers in the kit's editor |
| Actions | Navigate to, change to, back, scroll to, open link, open / swap / close overlay, set variable, set variable mode, conditional, play/pause/mute/seek media | 11: Navigate, Back, ScrollTo (with an offset), OpenOverlay, SwapOverlay, CloseOverlay, OpenUrl, ChangeTo, SetVariable, SetVariableMode, Conditional | media actions |
| Transitions | Instant, dissolve, smart animate, move in/out, push, slide in/out | all 8 | — |
| Easing | Linear, ease in/out/in-and-out, back variants, gentle, quick, bouncy, slow, custom bezier, custom spring | all 13 (springs physical) | — |

## Rules from the documentation

Status: **pass** (a case in the suite holds), **not tested** (the engine's player does it; no case
in this repository checks it yet), **extra** (Kition Design's own, beyond the reference
tool; opt-in), **silent** (the documentation doesn't say; the case pins the current behaviour).

| Rule | Status | Evidence / notes |
|---|---|---|
| On click: press and release on the layer; a drag is not a click | **not tested** | |
| On drag: a drag that starts on the layer; a click is not a drag | **not tested** | |
| On drag with a directional animation (move, push, slide): only a drag that way starts it; a drag another way can scroll instead | pass | `on_drag_starts_only_a_drag_its_animation_goes` (exactly diagonal counts for both axes: silent) |
| On drag: the animation it starts follows the pointer while it stays down, then plays on from there | **not tested** | |
| Mouse down / mouse up: as the button goes down / comes up | **not tested** | |
| While pressing: shown while the button is down, reverts on release | **not tested** | |
| While hovering: shown while the pointer is over the layer, reverts when it leaves; mouse enter fires once and doesn't revert; mouse leave fires on leaving | **not tested** | |
| Key/gamepad: a key or a combination of keys, pressed anywhere on the screen | pass | `K` doesn't fire for Shift+K; the editor's recorder records the modifiers held, and REST files' modifier key codes import as a combination; `key_triggers_fire_for_their_key_or_combination` |
| Key/gamepad on a controller: the trigger takes a controller button in place of a key, pressed anywhere on the screen | pass | `Gamepad A` … `Gamepad Home` (the standard layout's 17 buttons, face buttons by position) as the trigger's key, picked from the Controller menu beside the key field; the engine's player takes a press as an event (`bindings/wasm/src/player.rs`); the kit's editor reads no controllers yet; `controller_buttons_fire_key_gamepad_triggers` |
| A controller button is not the keyboard key of the same name; the d-pad is not the arrow keys; a keyboard modifier held makes a button press another input | silent | pinned as stated |
| REST files: controller triggers (`device` other than `KEYBOARD`) | pass | their code reads as the standard layout's button index (silent: the schema doesn't say what the codes count); two buttons at once or an unknown code is skipped with a warning; `controller_triggers_import_as_their_button` |
| After delay: timed from when the screen shows | **not tested** | |
| After delay on an overlay: timed from when the overlay opens, again each time it opens | **not tested** | |
| Nested layers with the same trigger: the top-most layer's interaction wins; a layer without that trigger lets it through to the one under it | pass | `hotspots_priority_and_click_through` |
| A layer without interactions over a hotspot | silent | it doesn't block it: clicks reach the hotspot under it |
| Hidden layers aren't hotspots; clipping frames cut their hotspots | pass | same |
| Navigate to: replaces the screen; from an overlay it closes the overlays; preserve scroll position keeps the offset (as far as the destination scrolls) | **not tested** | |
| Back: closes the top overlay first, else returns to the previous screen playing its animation backwards; restart empties the history | pass | `back_and_reverting_play_the_animation_the_other_way` |
| Scroll to: every frame around the layer scrolls to bring it in, animated with the interaction's animation | **not tested** | |
| Scroll to with an offset: the layer stops that far from the top-left (room for a fixed header) | pass | `offset: [x, y]` on the action, X / Y fields in the interaction editor; `scroll_to_keeps_an_offset` (nested scrolling frames: the innermost keeps the offset; silent) |
| Open overlay: positions, background dimming, close on a click outside (on by default, can be turned off) | **not tested** | |
| Overlays stack; swap overlay replaces the top one; close overlay closes it | **not tested** | |
| Manual overlay position relative to the hotspot | pass | `offset: [x, y]` on the action, from the top-left of the layer that opens it (X / Y fields in the interaction editor; without one, below that layer) |
| Overlay position presets relative to the layer that opens it, inside or outside (not in Figma: [forum request](https://forum.figma.com/suggest-a-feature-11/set-overlay-position-relative-to-element-not-to-page-35366)) | **extra** | `anchor: triggerInside \| triggerOutside` on the open overlay action; Relative to (Whole screen / Trigger layer) and Inside / Outside in the interaction editor. Outside: top presets above the layer, bottom presets below it, edges lined up (a dropdown is Bottom left + Outside); Center centres it on the layer. No flipping at the screen edge. `.fig` export writes a manual offset; `proto_overlay_anchor_tests`, MCP `overlay_anchor` |
| Delay on mouse enter / leave / down / up | **not tested** | `delayMs` on the interaction, a Delay field in the editor; fires once after the delay while its screen or overlay still shows (a pointer that leaves early: silent, it still fires) |
| Open link | pass | always a new tab or browser window (opening in place would leave the prototype) |
| Change to: another variant of the instance, in place, with the interaction's animation | **not tested** | |
| Interactive components: every instance plays the interactions of the variant it shows; an interaction on the instance with the same trigger wins | pass | instances don't copy the interactions when placed; copies a file carries give way to the variant shown; `instances_play_their_variants_interactions` |
| Change to on a layer inside a nested instance changes the instance whose variant set holds the destination | pass | not the innermost instance (an icon inside a button) |
| Set variable, set variable mode, conditional | pass | `proto_run_tests`, `proto_conditional_tests` |
| Change to, set variable and set variable mode lay out what they change as the canvas does: text sizes to what it shows, hugging frames around it | **not tested** | |
| Expressions: a variable reads the mode where the interaction runs (a frame that chose a mode, else the page's); `name[Mode]` reads a given mode whatever is chosen; operators on numbers, text (`+` joins), booleans and comparisons | pass | `mode_lookups_read_a_given_mode_or_the_layers_current_one`, `proto_expr_mode_tests` |
| Set variable mode, then a conditional or set variable later in the same interaction: they see the new mode | pass | `set_variable_mode_changes_what_later_expressions_read` |
| A set variable writes every mode of the variable, so a lookup of any mode reads the new value; a frame inside the screen that chose a mode keeps it after set variable mode | silent | pinned as stated |
| Widgets in a presented prototype react to clicks on their clickable parts | pass | the plugin is called back as on the canvas (same callback, sandbox and fuel limits), on the prototype's copy: the widget's state is the prototype's, the file and its undo history don't change, restarting starts over (silent; pinned); an interaction on the layer wins over the widget; a plugin that's gone or fails shows its error over the presenter; `widgets_react_to_clicks_on_the_prototypes_own_copy` |
| Smart animate: layers matched by name and hierarchy tween position, size, rotation, opacity, solid fills and strokes and corner radius; unmatched layers dissolve | **not tested** | |
| Smart animate with a spring or back easing overshoots (and pulls back) | pass | |
| Visibility, text content, gradients and images in smart animate | silent | they show the destination's |
| Move in/out, push, slide in/out move the way their direction names; reversed they go the other way | pass | `back_and_reverting_play_the_animation_the_other_way` |
| Easing curves: linear, ease in, ease out, ease in and out, back variants, custom bezier | pass | `easing_curves_and_springs` |
| Spring presets are physical springs (mass 1; stiffness / damping: gentle 100/15, quick 300/20, bouncy 600/15, slow 80/20) | pass | |
| A spring plays until it settles: its duration follows from its mass, stiffness and damping, not from the duration field | **not tested** | the editor shows the spring's duration in place of the field |
| Scrolling: horizontal, vertical or both; momentum; fixed layers stay put; sticky layers stop at the edge | **not tested** | sticky on the left edge too: silent |
| Fixed layers draw and take clicks above the content scrolling under them, wherever they are in the layer list | **not tested** | the file's order doesn't change |
| Flows and starting points; device frames; presentation options; hotspot hints | pass | `proto_present_tests` |

## Features

| Feature | Status | Notes |
|---|---|---|
| Connections drawn and dragged on the canvas; interaction editor | done | |
| Several actions per interaction; drag to reorder; conditional blocks with else-if/else | done | one undo step per edit |
| Expressions (`count + 1`, `mode(Theme) == "Dark"`, `accent[Dark]`) | done | checked when entered; `proto.evaluate` computes one where a layer's interaction would run (for agents) |
| Flows, starting points, descriptions | done | |
| Overlays: position, close on click outside, background, animate out, stacking, timers from when they open | done | |
| Scroll behaviour: overflow directions, fixed (drawn on top), sticky, preserve scroll position, scroll to with an offset, momentum and overscroll | done | |
| Key combinations (`Shift+K`), recorded in the editor and imported from REST files | done | |
| Controller buttons on key/gamepad triggers (Controller menu, REST import; the engine's player takes them as events) | done | the kit's editor reads no controllers yet; button names follow one layout (no PlayStation or Switch labels) |
| Widgets react to clicks in a presented prototype | done | clicks only, like on the canvas |
| Interactive components: instances play their variant's interactions, overridden per trigger | done | |
| Drag-driven animations that follow the pointer, in their direction | done | |
| Presenter: device frames and colours, rotation, custom sizes, fit/fill/100%, responsive frames | done | device bodies drawn in code. The frame is laid out again for the device's screen on the prototype's copy (constraints and auto layout; the file is untouched); on phones and tablets overflowing content drags while the device stays put: a top-level frame bigger than the screen always scrolls (as in Figma, with no overflow setting), content past the frame's own edges as its overflow setting allows ("No scrolling" clips it). Devices not compared with Figma's |
| Hotspot hints, inspect panel in the presenter, comments in the presenter | done | |
| Preview pane following edits live | done | |
| Edit interactions on several layers at once | done | |
| Open link: a notice before a website opens, with a checkbox to skip it on this device | done | `http` and `https` links (a mail link opens at once); Open link opens it, Cancel or Escape drops it; the editor keeps the checkbox until the page reloads (`web/src/prototype/Player.tsx`); `proto.linkNotice {show}` turns it on or off |
| Opening a link in place | missing | |
| Controllers in the editor's player | missing | nothing in `web/` reads the browser's Gamepad API yet |
| Prototype in a separate window; mirror to a phone | missing | see [hardware-parity](hardware-parity.md) |
| Video in prototypes | missing | needs video fills |
| Share a prototype link | missing | needs multiplayer/sharing |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
