/**
 * Command ids as the upstream catalogs list them (`upstream/*.toml`, sections `[commands]` and
 * `[menu]`) → this kit's command ids. Those files keep the ids they were written with; code that joins them with the kit's commands goes through this table.
 * Generated from kition-plan `tools/command-renames/maps/design.json` (the rename of 2026-11-25):
 * do not edit by hand, and do not rename the keys.
 */
export const UPSTREAM_COMMAND_IDS: Record<string, string> = {
  'branch.create': 'branch.add',
  'select.all': 'edit.selectAll',
  'select.none': 'edit.deselectAll',
  'object.group': 'arrange.group',
  'object.ungroup': 'arrange.ungroup',
  'component.create': 'component.add',
  'component.createMultiple': 'component.addMultiple',
  'arrange.front': 'arrange.bringToFront',
  'arrange.forward': 'arrange.bringForward',
  'arrange.backward': 'arrange.sendBackward',
  'arrange.back': 'arrange.sendToBack',
  'layers.collapseAll': 'layer.collapseAll',
  'text.bold': 'format.bold',
  'text.italic': 'format.italic',
  'text.underline': 'format.underline',
  'text.strikethrough': 'format.strikethrough',
  'text.alignLeft': 'format.alignLeft',
  'text.alignCenter': 'format.alignCenter',
  'text.alignRight': 'format.alignRight',
  'text.alignJustify': 'format.justify',
  'text.sizeUp': 'format.increaseFontSize',
  'text.sizeDown': 'format.decreaseFontSize',
  'pref.keepTool': 'prefs.keepTool',
  'pref.highlightHover': 'prefs.highlightHover',
  'pref.renameDuplicates': 'prefs.renameDuplicates',
  'pref.showDimensions': 'prefs.showDimensions',
  'pref.numberKeysOpacity': 'prefs.numberKeysOpacity',
  'pref.nudge': 'prefs.nudge',
  'node.create': 'node.add',
  'frame.create': 'frame.add',
  'text.create': 'text.add',
  'vector.create': 'vector.add',
  'export.update': 'export.set',
  'selection.colors': 'select.colors',
  'selection.setPaint': 'select.setPaint',
  'selection.applyColorStyle': 'select.applyColorStyle',
  'selection.bindColorVariable': 'select.bindColorVariable',
  'selection.detachColor': 'select.detachColor',
  'selection.selectColor': 'select.selectColor',
  'style.create': 'style.add',
  'style.update': 'style.set',
  'variable.createCollection': 'variable.addCollection',
  'variable.create': 'variable.add',
  'instance.create': 'instance.add',
}

const CATALOG_ID = new Map(Object.entries(UPSTREAM_COMMAND_IDS).map(([upstream, id]) => [id, upstream]))

/** The id those files list this kit's command `id` under (itself when it never changed). */
export function catalogCommandId(id: string): string {
  return CATALOG_ID.get(id) ?? id
}

/** This kit's id of a command those files list as `id`. */
export function kitCommandId(id: string): string {
  return UPSTREAM_COMMAND_IDS[id] ?? id
}
