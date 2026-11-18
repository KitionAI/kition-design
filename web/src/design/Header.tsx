/**
 * The selection header (2.3 "Selection header", upstream `panels/right.rs` `header`): the type
 * label (a menu converting frames and groups, with frame presets for a frame), Swap orientation for
 * one frame, and the quick actions Create component, Use as mask and Boolean groups.
 */
import { useRef, useState } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon, type IconName } from '../icons/Icon'
import { IconButton, Menu, prettyShortcut, type MenuEntry } from '../ui'
import { useDesign, useShortcutTip } from './hooks'
import { menuHeading } from './fields'
import { common } from './model'

const BOOL_OPS: Array<{ op: string; label: string; command: string; icon: IconName }> = [
  { op: 'union', label: 'Union', command: 'bool.union', icon: 'Union' },
  { op: 'subtract', label: 'Subtract', command: 'bool.subtract', icon: 'Subtract' },
  { op: 'intersect', label: 'Intersect', command: 'bool.intersect', icon: 'Intersect' },
  { op: 'exclude', label: 'Exclude', command: 'bool.exclude', icon: 'Exclude' },
]

type Preset = { category: string; name: string; width: number; height: number }

export function SelectionHeader() {
  const { t, run } = useDesignKit()
  const { answer, nodes, ids, setProps } = useDesign()
  const tipFor = useShortcutTip()
  const keymap = useQuery<{ bindings: Array<{ id: string; keys: string[] }> }>('keymap')
  const keysOf = (command: string) => prettyShortcut(keymap?.bindings.find((b) => b.id === command)?.keys[0])
  const kinds = common(nodes, (n) => n.type)
  const label = !kinds ? t('Mixed') : nodes.length === 1 ? t(answer.kind ?? '') : t(`${answer.kind ?? ''}s`)
  const isFrame = nodes.length === 1 && kinds === 'FRAME'
  const isGroup = nodes.length > 0 && kinds === 'GROUP'
  const presets = useQuery<Preset[]>('framePresets', {}, { skip: !isFrame })
  const typeButton = useRef<HTMLButtonElement>(null)
  const boolButton = useRef<HTMLButtonElement>(null)
  const [menu, setMenu] = useState<'type' | 'bool' | null>(null)
  const current = answer.booleanOperation ? BOOL_OPS.find((b) => b.op === String(answer.booleanOperation).toLowerCase()) : undefined

  const kindItems = (frame: boolean): MenuEntry[] => [
    { label: t('Frame'), checked: frame, onSelect: () => (frame ? undefined : void run('object.convertToFrame', { ids })), testId: 'design-design-type-frame' },
    { label: t('Group'), checked: !frame, onSelect: () => (frame ? void run('object.convertToGroup', { ids }) : undefined), testId: 'design-design-type-group' },
  ]
  const typeItems: MenuEntry[] = isGroup
    ? kindItems(false)
    : [
        ...kindItems(true),
        { type: 'separator' },
        ...(presets ?? []).flatMap((preset, i, all): MenuEntry[] => [
          ...(i === 0 || all[i - 1].category !== preset.category ? [menuHeading(t(preset.category))] : []),
          { label: preset.name, shortcut: `${preset.width}×${preset.height}`, onSelect: () => setProps({ width: preset.width, height: preset.height }), testId: `design-design-preset-${preset.name}` },
        ]),
      ]
  const boolItems: MenuEntry[] = [
    ...BOOL_OPS.map((b) => ({ label: t(b.label), shortcut: keysOf(b.command), checked: current ? current.op === b.op : undefined, onSelect: () => void run(b.command, {}), testId: `design-design-bool-${b.op}` })),
    { label: current ? `    ${t('Flatten')}` : t('Flatten'), shortcut: keysOf('vector.flatten'), onSelect: () => void run('vector.flatten', {}) },
  ]

  return (
    <div className="design-d-header" data-testid="design-design-header">
      <button
        ref={typeButton}
        type="button"
        className="design-d-type"
        aria-haspopup={isFrame || isGroup ? 'menu' : undefined}
        aria-expanded={menu === 'type'}
        onClick={() => (isFrame || isGroup ? setMenu(menu === 'type' ? null : 'type') : undefined)}
        data-testid="design-design-type"
      >
        <span className="design-d-type-label">{label}</span>
        {isFrame || isGroup ? (
          // Upstream centres the chevron at 18 + 6.5 px per byte of the label, not after the text.
          <Icon name="Chevron" size={10} className="design-d-type-chevron" style={{ left: 18 + new TextEncoder().encode(label).length * 6.5 - 5 }} />
        ) : null}
      </button>
      {isFrame ? <IconButton icon="Rotate" label={t('Swap orientation')} onClick={() => void run('frame.swapOrientation', { ids })} testId="design-design-swap" className="design-d-swap" /> : null}
      <span className="design-d-header-actions">
        <IconButton icon="Component" label={tipFor(t('Create component'), 'component.create')} onClick={() => void run('component.create', {})} testId="design-design-create-component" />
        <IconButton icon="Mask" label={tipFor(t('Use as mask'), 'object.mask')} onClick={() => void run('object.mask', {})} testId="design-design-mask" />
        <span ref={boolButton} className="design-d-anchor">
          <IconButton icon={current?.icon ?? 'Union'} label={t('Boolean groups')} onClick={() => setMenu(menu === 'bool' ? null : 'bool')} testId="design-design-boolean" />
        </span>
      </span>
      {/* Upstream's presets sit in a 420 px scroll area; the menu it opens measures 398 px tall. */}
      {menu === 'type' ? <Menu tone="light" items={typeItems} anchor={typeButton.current} placement="below-start" gap={0} minWidth={isGroup ? 180 : 260} maxHeight={isGroup ? undefined : 398} onClose={() => setMenu(null)} testId="design-design-type-menu" /> : null}
      {menu === 'bool' ? <Menu tone="light" items={boolItems} anchor={boolButton.current} placement="below-start" gap={0} minWidth={180} onClose={() => setMenu(null)} testId="design-design-boolean-menu" /> : null}
    </div>
  )
}
