/** Panel widths (2.1 §1, `state.rs fit_panels`). */
export const RAIL = 56
export const PANEL_DEFAULT = 240
export const LEFT_RANGE = [200, 480] as const
export const RIGHT_RANGE = [240, 480] as const
export const CANVAS_MIN = 160

const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), Math.max(min, max))

/**
 * The panels' widths for a `width`-wide editor and the asked widths: the right panel first, then
 * the left one, each leaving the canvas at least 160; the right panel gives way last.
 */
export function fitPanels(width: number, asked: { left: number; right: number }, leftOpen: boolean) {
  const avail = width - RAIL
  const right = clamp(asked.right, RIGHT_RANGE[0], clamp(avail - (leftOpen ? LEFT_RANGE[0] : 0) - CANVAS_MIN, RIGHT_RANGE[0], RIGHT_RANGE[1]))
  const left = leftOpen ? clamp(asked.left, LEFT_RANGE[0], clamp(avail - right - CANVAS_MIN, LEFT_RANGE[0], LEFT_RANGE[1])) : 0
  return { left, right }
}
