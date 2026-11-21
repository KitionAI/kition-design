# kit.design editor (web/)

The Design kit's own editor: a React app on `@kition/sdk/editor` that rebuilds the upstream editor's
UI. A host (Kition, or `dev.html` here)
shows it in an iframe over the Kit UI protocol; the engine (`bindings/wasm`) runs in a worker.

```sh
pnpm install
sh scripts/build-engine.sh   # engine → public/engine, fonts → public/fonts (from ../kition-fonts)
pnpm dev                     # http://127.0.0.1:5196/dev.html
pnpm typecheck && pnpm build
pnpm test:flow               # Chromium and WebKit, through the dev host
```

## How it fits kit-editor's frame

The upstream editor has no menu bar: its main menu sits under the logo in the left rail. The kit
still draws kit-editor's `EditorFrame`, in **Menus** display (`kit.json` `ui.display: "menus"`):
its top bar holds the engine's menus (File … Libraries, from the engine's catalog), Undo, Redo,
Ask Agent, the display-mode button and Save, which keeps the frame contract (host buttons, hidden
and disabled elements, read-only, display modes, the command dialogs, Save through the host, Cmd+S
and the engine's shortcuts). Everything under the top bar is the upstream layout (`Shell`): rail,
left panel, canvas, floating toolbar, right panel. The frame's status bar is off (upstream has
none; its zoom is in the right panel header). Single line and Classic modes show the frame's
command row above the same layout; Focus (Cmd+\\) hides the rail, panels and toolbar, as
upstream's View > Hide UI does. The logo button still opens upstream's dark main menu.

Share opens no online session in Kition: the button is drawn but disabled, with a tooltip saying
sharing happens in Kition. Present asks for the `present` UI request (the prototype package
draws the player); Preview runs `proto.preview`.

## Folders

| Folder | What |
|---|---|
| `src/kit/` | context, hooks, types, UI requests, F6 regions |
| `src/ui/` | the control kit (below) |
| `src/shell/` | layout, rail, toolbar pill and flyouts, right header and zoom menu, help, shell keys |
| `src/canvas/` | `DesignCanvas` (kit-editor canvas, page background, chrome, auto layout handles), `chrome.ts`, and `engineOverlays.ts` (the engine's overlays in the upstream style) |
| `src/left/` | `LeftPanel`: file header, Pages, Layers, Assets and libraries, Local variables, the variable and style pickers |
| `src/design/` | `DesignPanel`: the section router and the structure sections |
| `src/paint/` | `PaintSections`: Typography, Fill, Stroke, Effects; the colour picker (`ColorPicker`, and `TargetColorPicker` on engine paints) and its canvas handles |
| `src/prototype/` | `PrototypePanel`, the interaction editor and the player |
| `src/dev/` | `DevPanel`, Dev Mode's Inspect, and version history |
| `src/menus/` | `MainMenu` and the top bar's menus from one model (`model.ts`), the keymap and key dispatcher, `ActionsPalette`, the canvas context menu, `MenusHost` (mounted in the canvas cell) |
| `src/dialogs/` | the dialogs (`openDialog(kind, data)`), the dialog window |
| `src/comments/` | comment pins, threads and the Comments panel (Comment tool) |

## Extension points

Everything a panel needs comes from `src/kit/hooks.ts`:

- `useDesignKit()`: `{ inspect, run, query, invoke, interaction, viewTo, refresh, canvas, tools, ui, t, strings, readOnly, toast }`.
- `useInspect()`: the engine's `inspect()` (`DesignInspect` in `kit/types.ts`): pages, selection,
  selection bounds, tool, editing modes, area.
- `useSelection()`: `{ ids, count, bounds, set, toggle, clear }`.
- `useQuery(name, params?, { skip? })`: a panel query (`bindings/wasm/src/query.rs`); re-run after every change of the document or selection, cached and deduplicated per
  revision; the previous answer stays until the next arrives.
- `useRun()`: `run(command, params)` answers the parsed result, or null after a toast with the
  engine's error. Commands that only ask for UI answer `uiRequests`; listen with
  `useUiRequest(kind, handler)` from `kit/requests.ts` (kinds: `panel.*`, `dialog.*`,
  `contextMenu`, `variables`, `present`, `open.help`, …; the shell answers the `panel.*` ones).
- `useInteraction()`: `const i = interaction('Padding'); i.update('node.set', …); await i.commit()`
  (or `cancel()`): a drag or scrub as one undo step, through the binding's
  `beginInteraction`/`commitInteraction`/`cancelInteraction`; updates are coalesced (the latest
  waits while one runs). Without those extras each update is its own step.
- `useNumberProp(ids, prop, label, valueOf?)`: `onCommit` and `scrub` for a `NumberField` that
  writes `node.set { ids, props: { [prop]: v } }`, with relative edits per layer (`command.batch`).
- `useChecked()`: View and Preferences toggles that are on. `useView()`: the canvas view (zoom,
  document point at the top-left, size). `designKit.viewTo(command)`: move the view where a View command
  takes it (`viewTarget`).
- Keyboard regions: put `{...regionProps('layers' | 'properties' | …, label)}` on a region root and
  `data-design-prefer` on the control F6 should land on (the selected layer row).
- The colour picker (`src/paint`): `<ColorPicker paint onChange={(paint, phase) => …} onClose />`
  is controlled (`phase` is `live` during a drag, `done` when an edit is complete; `paint.ts`
  reads and writes the engine's paint JSON). To edit an engine paint, open the shared picker with
  `openPicker(target)` or the UI request `picker.open` (`{ target: 'paint' | 'effect' |
  'selectionColor' | 'textRange' | 'pageBackground' | 'variable' | 'style', …fields }`); it writes
  with the target's command, a drag as one undo step, and draws gradient, crop and progressive blur
  handles on the canvas.
- Canvas overlays (`src/canvas/overlays.tsx`): `useCanvasOverlay(key, element)` shows HTML over the
  canvas while mounted; `registerCanvasLayer(key, Component)` mounts a component with the canvas.

Controls (`src/ui/index.ts`), all sized and coloured to the upstream editor's metrics:
`NumberField` (scrub with Pointer Lock, Shift ×10, Alt ×0.1, math such as `10+5`, `*2`, `+10` per
layer when mixed, Up/Down, Mixed and placeholder, suffix, addon), `TextField`, `Dropdown`, `Menu`
(`tone="dark"` main/context/flyout menus, `tone="light"` dropdowns; submenus with a 150 ms hover
delay and a safe triangle; keyboard), `Popover`/`Layer`, `IconButton` (and icon toggles with
`pressed`), `Button`, `Segmented`, `Checkbox`, `Radio`, `SectionHeader` (actions; a dim section's
header is its "+"), `Row`, `SubLabels`, `Separator`, `Note`, `Disclosure`, `columnWidth`.
Tooltips: any element with `data-tip="Label  ⇧L"` (use `tip(label, shortcut)`). Toasts:
`showToast(text)`. Icons: `<Icon name="…" size={16} />` from `src/icons/Icon.tsx`.

Styles: only the `--design-*` tokens in `src/app.css` (light and dark follow the host's colour
scheme; `.design-on-dark` forces dark), the type classes (`design-r11`, `design-s11`, …) and the metrics
variables. Add a panel's CSS to `app.css` under its own heading, or a CSS file next to it.

Strings (`src/strings.ts`): `t('English label')` translates through the upstream catalog of the
host's locale (`src/i18n/upstream/*.toml`, `[ui]`); `strings.command(id, english)` and
`strings.menuLabel(id, english)` use `[commands]` and `[menu]`. A label the catalogs lack goes into
`PHRASES['zh-CN']`; keys of the editor's own (not upstream text) go into `CATALOGS` (en-US and
zh-CN). The editor follows the host's locale changes.

## Open engine requests

- Canvas chrome: a section's size on screen (for "Contents hidden" inside hidden sections).
- A node link scheme (`kition-design://node/<id>`) that `comment.open`-like commands accept;
  `comment.zoom` and `comment.open { view }` move the engine's view, which the kit's canvas does
  not follow (the editor zooms and pans itself).
- `chrome` lists no page guides: `guides::segment` answers nothing for a page, so a guide on the
  page (not in a top-level frame) is neither drawn nor right-clickable; upstream draws page guides
  across the whole canvas.
- `branch.create` (File ▸ Create branch…) is offered while `branch.open` and `branch.review` are
  left out: the version panel hides its Branches section where `branch.open` is not offered, so a
  branch made from the File menu cannot be seen. Leave `branch.create` out too while branches are
  deferred.

## kit-editor and Tessera requests

- kit-editor lets a kit draw engine overlays itself (`overlayStyle`, `drawOverlay`) and
  put chrome under them (`decorateUnder`); `src/canvas/engineOverlays.ts` draws them in the
  upstream style. The overlays still carry no semantic role, so the drawer reads upstream's kinds
  back from their sizes (anchor 8: a selection corner; 7, 9, 11: points; a guide-red line before
  a measure: its distance label). The Kition platform's `handle` roles (`docs/platform/sets/pointer.md`) would make that
  exact.
- Tessera has no dark menu tone; `src/ui/Menu.tsx` draws both tones until it does.
- kit-editor has no API to toggle Focus (View > Show/Hide UI); the menus send the frame its own
  Mod+\\ key. The frame binds only a command's first shortcut: the editor dispatches the others.
