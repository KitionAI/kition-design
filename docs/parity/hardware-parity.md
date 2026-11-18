# Hardware and platform parity

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-17 · **Target:** Figma Design (desktop app 126.10.7)

How Kition Design uses hardware and which platforms it runs on, against Figma. Figma's desktop app is
Electron with a GPU helper process; its canvas renders on the GPU (WebGL, with WebGPU rolling out
since 2025). Kition Design renders the canvas on the CPU, in the engine's Web Worker, and its editor shows it
in the page (kit-editor's canvas, `web/src/canvas/DesignCanvas.tsx`).

**Summary (estimated):** hardware ~60%, performance ~60%, platforms ~75% (not yet measured in the
kit's editor).

## Hardware

| Feature | Figma | Kition Design (the editor in the browser) | Hours |
|---|---|---|---|
| Canvas rendering | GPU | CPU (tiny-skia) in the engine's Web Worker, on one thread (`render_view_single`; native builds band the view across cores); each view the editor asks for is drawn whole (the renderer's retained mode is not used by the kit, `bindings/wasm/src/render.rs`); levels of detail when zoomed out | 30–60 for a GPU path |
| Effects (blurs, glass, noise) | GPU | CPU | in the above |
| HiDPI / Retina | yes | yes: the editor's canvas is sized in device pixels (`devicePixelRatio`) | — |
| Wide-gamut display (Display P3) | yes, on capable displays | colours stored and exported in P3; the screen shows sRGB (the binding converts the rendered page to sRGB, `bindings/wasm/src/render.rs`). The colour picker warns about colours outside sRGB (`web/src/paint/ColorPicker.tsx`) and View ▸ Soft-proof sRGB stripes them on the canvas | 2–5 |
| Trackpad pinch zoom and two-finger pan | yes | yes (kit-editor's canvas: wheel and pinch) | — |
| Pen tablet pressure (pencil, Figma Draw brushes) | yes (forum reports of regressions in 2025) | the pressure a pointer event carries, when the browser reports one (`bindings/wasm/src/tools.rs`); the Pencil records it as the stroke's width profile. A mouse, or a pen whose browser reports no pressure, draws a constant width. Not checked with a tablet in the editor | 1–3 |
| Several windows / multi-monitor | yes (a window per file; presenter in its own window) | the kit is one editor per open file inside the host's page; windows and tabs are the host's, and the kit leaves the `window.*` commands out (`bindings/wasm/src/commands.rs`) | 1–2 |
| Screen readers | partial | the editor's panels and menus are HTML elements, so they are in the browser's accessibility tree; never tested with a real screen reader | 4–8 + human QA |
| Keyboard layouts (non-US) | yes | keymap uses logical keys; not checked on AZERTY/QWERTZ/JIS | 1–2 |

## Performance (measured natively, synthetic files, a maintainer's Mac)

| Case | Kition Design | Figma |
|---|---|---|
| 50k-layer page, zoomed out, full render | ~8 ms (levels of detail) | not timed side by side |
| 10k layers: full 1440×900 render | 7–15 ms | not timed |
| 10k layers: move one layer, re-render | ~0.2 ms | not timed |
| Design system of ~300 components × 2,000 instances: recolour a main used ~900 times | 4–18 ms | not timed |
| `.kdesign` at 50k layers: save / load | ~0.09 s / ~0.16 s | n/a |

Nothing has been timed against Figma on the same document (needs `.fig` import).

## Platforms

| Platform | Figma | Kition Design | Notes |
|---|---|---|---|
| Web | primary (figma.com) | the kit's editor (`web/`) inside Kition, in an iframe; files only through the host | the engine is WebAssembly in a Web Worker; the editor's flow tests run in Chromium and WebKit (`web/package.json`) |
| Agents | — | the headless kit executable `kition-design` (`bindings/headless`), built with Cargo | serves the same document over the Kit Protocol; never shows a window |
| iOS / Android | apps: view, comment, mirror prototypes | none (the editor is not touch-first) | 20–40 h for a viewer |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First public version |
