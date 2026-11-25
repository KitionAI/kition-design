/** Dev Mode only inspects (`menus.rs` `dev_mode_refuses`): the left panel's edits are refused there with a toast. */
import type { DesignKit } from '../kit/context'

/** Commands Dev Mode still runs from the left panel (going to a page, selecting, expanding). */
const ALLOWED = new Set(['page.select', 'select.set', 'edit.deselectAll'])

/** Whether Dev Mode refuses `command` (and says so). */
export function devRefuses(designKit: Pick<DesignKit, 'ui' | 't' | 'toast'>, command: string): boolean {
  if (designKit.ui.rightTab !== 'inspect' || ALLOWED.has(command)) return false
  designKit.toast(designKit.t('Dev Mode only inspects: switch to Design (Shift+D) to edit'))
  return true
}
