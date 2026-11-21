import type { KitModuleManifest } from '@kition/sdk/editor'

/**
 * Faces the web build does not embed, from the package's own `fonts/` folder (build-engine.sh
 * copies them from kition-fonts): Droid Sans Fallback for Chinese and Japanese text (never Noto
 * CJK or Source Han).
 */
export const ENGINE_FONTS = ['DroidSansFallbackFull.ttf']

/** The engine, from this package's own folder. */
export function engineManifest(): KitModuleManifest {
  return {
    kitId: 'kit.design',
    base: new URL('./engine/', document.baseURI).href,
    module: 'kition_design_kit',
    documentClass: 'DesignDocument',
    fonts: ENGINE_FONTS.map((file) => new URL(`./fonts/${file}`, document.baseURI).href),
  }
}
