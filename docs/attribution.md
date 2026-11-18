# Attribution

Every asset bundled in this repository (images, icons, fonts, data files) is listed here with its
author, source and license. `cargo xtask assets` fails if a file under `assets/` is missing from
this list.

The editor's user-interface icons are original work (Apache-2.0). No third-party icon sets are
used.

## Assets in this repository

| Path | Author | Source | License | Notes |
|---|---|---|---|---|
| `examples/*.kdesign` | upstream contributors (see NOTICE) | Synthetic demo documents built through the engine's command API; invented copy, shapes and vectors drawn from scratch, no images or embedded fonts | MIT (see NOTICE) | Text uses Inter (OFL, from kition-fonts or the system); no third-party artwork |
| `web/public/sample/libraries/Design System.kdesign`, `web/tests/fixtures/Design System v2.kdesign` | The Kition authors | Synthetic library files written by `bindings/wasm/examples/sample_library.rs` with the kit's engine: invented components, styles and variables, no images or embedded fonts | Apache-2.0 | Text uses Inter (OFL, from kition-fonts or the system); no third-party artwork |
| `assets/readme/hero.png`, `assets/readme/see-1.png`, `assets/readme/see-2.png`, `assets/readme/see-3.png`, `assets/readme/see-4.png` | The Kition authors | Screenshot of Kition Design's editor on the showcase sample sales-analytics-dashboard, taken by `web/scripts/readme-shots.mjs` | Apache-2.0 | Invented names and figures; no third-party artwork |
| `assets/readme/made-1.png` | The Kition authors | Rendered by Kition Design from the showcase sample saas-landing-page | Apache-2.0 | kition-showcase `design/saas-landing-page/thumb.png` |
| `assets/readme/made-2.png` | The Kition authors | Rendered by Kition Design from the showcase sample habit-app-onboarding-flow | Apache-2.0 | kition-showcase `design/habit-app-onboarding-flow/thumb.png` |
| `assets/readme/made-3.png` | The Kition authors | Rendered by Kition Design from the showcase sample design-tokens-light-dark | Apache-2.0 | kition-showcase `design/design-tokens-light-dark/thumb.png` |
| `assets/readme/made-4.png` | The Kition authors | Rendered by Kition Design from the showcase sample button-component-set | Apache-2.0 | kition-showcase `design/button-component-set/thumb.png` |
| `web/src/icons/` | upstream contributors (see NOTICE) and the Kition authors | Ported from the upstream editor's code-drawn icons (`crates/ui-egui/src/icons/draw.rs` at upstream commit 43af145a) by `web/scripts/port-icons.mjs` | MIT (upstream part, see NOTICE) and Apache-2.0 | Original artwork; no third-party icons |

## Fonts (build input, not in this repository)

Builds embed fonts from the optional kition-fonts build input
([KitionAI/kition-fonts](https://github.com/KitionAI/kition-fonts), `KITION_FONTS_DIR`). Its own
`ATTRIBUTION.md` and license files are authoritative; the packages ship each font's license.
