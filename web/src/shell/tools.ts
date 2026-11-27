/** The bottom toolbar's groups (2.1 §4.3, engine `canvas/tools.rs`): tool ids, icons, labels, shortcuts. */
import type { IconName } from '../icons/Icon'

export type ToolDef = { id: string; icon: IconName; label: string; shortcut: string | null }
export type ToolGroup = { key: string; flyout: string; tools: ToolDef[] }

export const TOOL_GROUPS: ToolGroup[] = [
  {
    key: 'move',
    flyout: 'Move tools',
    tools: [
      { id: 'move', icon: 'Move', label: 'Move', shortcut: 'V' },
      { id: 'hand', icon: 'Hand', label: 'Hand tool', shortcut: 'H' },
      { id: 'scale', icon: 'Scale', label: 'Scale', shortcut: 'K' },
    ],
  },
  {
    key: 'frame',
    flyout: 'Frame tools',
    tools: [
      { id: 'frame', icon: 'Frame', label: 'Frame', shortcut: 'F' },
      { id: 'section', icon: 'Section', label: 'Section', shortcut: 'Shift+S' },
      { id: 'slice', icon: 'Slice', label: 'Slice', shortcut: 'S' },
    ],
  },
  {
    key: 'shape',
    flyout: 'Shape tools',
    tools: [
      { id: 'rectangle', icon: 'Rectangle', label: 'Rectangle', shortcut: 'R' },
      { id: 'line', icon: 'Line', label: 'Line', shortcut: 'L' },
      { id: 'arrow', icon: 'Arrow', label: 'Arrow', shortcut: 'Shift+L' },
      { id: 'ellipse', icon: 'Ellipse', label: 'Ellipse', shortcut: 'O' },
      { id: 'polygon', icon: 'Polygon', label: 'Polygon', shortcut: null },
      { id: 'star', icon: 'Star', label: 'Star', shortcut: null },
    ],
  },
  {
    key: 'pen',
    flyout: 'Drawing tools',
    tools: [
      { id: 'pen', icon: 'Pen', label: 'Pen', shortcut: 'P' },
      { id: 'pencil', icon: 'Pencil', label: 'Pencil', shortcut: 'Shift+P' },
      { id: 'shapeBuilder', icon: 'ShapeBuilder', label: 'Shape builder', shortcut: 'Shift+M' },
    ],
  },
  {
    key: 'text',
    flyout: 'Text',
    tools: [
      { id: 'text', icon: 'Text', label: 'Text', shortcut: 'T' },
      { id: 'textOnPath', icon: 'TextOnPath', label: 'Text on a path', shortcut: null },
    ],
  },
  { key: 'comment', flyout: 'Comment', tools: [{ id: 'comment', icon: 'Comment', label: 'Comment', shortcut: 'C' }] },
]

export const ALL_TOOLS: ToolDef[] = TOOL_GROUPS.flatMap((group) => group.tools)

/** The vector edit pill's tools (2.1 §4.6) by the engine's ids, when its `vectorEdit` query does not list them. */
export const VECTOR_TOOLS: Array<ToolDef> = [
  { id: 'move', icon: 'Move', label: 'Move', shortcut: 'V' },
  { id: 'pen', icon: 'Pen', label: 'Pen', shortcut: 'P' },
  { id: 'bend', icon: 'Bend', label: 'Bend', shortcut: null },
  { id: 'paint', icon: 'PaintBucket', label: 'Paint bucket', shortcut: 'Shift+B' },
  { id: 'lasso', icon: 'Lasso', label: 'Lasso', shortcut: 'Q' },
  { id: 'cut', icon: 'Cut', label: 'Cut', shortcut: 'X' },
  { id: 'eraser', icon: 'Eraser', label: 'Eraser', shortcut: 'Shift+E' },
  { id: 'width', icon: 'VariableWidth', label: 'Variable width', shortcut: 'Shift+W' },
]

/** Upstream's zoom steps (2.5 §20). */
export const ZOOM_STEPS = [0.02, 0.03, 0.06, 0.13, 0.25, 0.5, 1, 2, 4, 8, 16, 32, 64, 128, 256]
export const MIN_ZOOM = 0.02
export const MAX_ZOOM = 256

export function zoomStep(zoom: number, direction: 1 | -1) {
  if (direction > 0) return ZOOM_STEPS.find((step) => step > zoom * 1.001) ?? MAX_ZOOM
  return [...ZOOM_STEPS].reverse().find((step) => step < zoom * 0.999) ?? MIN_ZOOM
}
