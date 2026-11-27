# Roadmap

> **Last updated:** 2026-11-27 · **Source:** `features.yaml` (updated 2026-11-27) and the engine's command list

Open work on Kition Design, in order. Each item names where its detail lives.

1. **Prove the P0 features end to end:** `design.agent.ids`, and the editor by hand: `design.editor.first-ten-minutes` (a marquee touching part of a top-level frame, waiting for a maintainer), `design.editor.canvas-picking`, `design.editor.canvas-input`, `design.editor.gesture-cost`, `design.editor.system-clipboard`, `design.editor.input-method` (each still needs checks on real devices and in Kition Desktop's frame). Detail in [gaps.md](gaps.md), including what the editor still lacks by hand.
2. **Then the P1 features:** `design.editor.file-drop`, `design.editor.auto-layout-handles`, `design.editor.vector-editing`, `design.board.whiteboard`, `design.comment.threads`, `design.component.instances`, `design.component.manage`, `design.edit.history`, `design.file.import-export`, `design.guide.manage`, `design.image.adjust`, `design.layout.auto`, `design.library.publish`, `design.object.arrange`, `design.object.frame-group`, `design.object.layers-api`, `design.object.transform`, `design.page.manage`, `design.paint.effects`, `design.paint.fill-stroke`, `design.proto.interactions`, `design.select.layers`, `design.shape.primitives`, `design.style.manage`, `design.text.format`, `design.tool.canvas`, `design.variable.manage`, `design.vector.boolean`, `design.vector.edit`, `design.version.history`, `design.view.display`.
3. **Then the P2 features:** `design.app.batch`, `design.app.preferences`, `design.devmode.handoff`.
4. **Keep the catalogue complete:** every new engine command joins a feature in `features.yaml` with a case; every editor behaviour a person uses joins one with a test in `web/tests/hand/` that does it with real pointer and keys.
