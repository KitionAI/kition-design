/**
 * Position (2.3 "Position", upstream `panels/right.rs` `position`): alignment and distribution, X
 * and Y (frame-relative), the absolute-position toggle in auto layout, rotation with Rotate 90°
 * and the flips, and the constraint dropdowns. Disabled inside instances.
 */
import { useRef, useState } from 'react'
import { useDesignKit } from '../kit/hooks'
import { useLiveNodes } from '../kit/gesture'
import { Dropdown, IconButton, Menu, NumberField, SectionHeader, Segmented } from '../ui'
import { useDesign, usePropField, useShortcutTip } from './hooks'
import { COL, Line, Space, Sub } from './fields'
import { common, round2 } from './model'

const H_CONSTRAINTS = [
  ['left', 'Left'],
  ['right', 'Right'],
  ['leftRight', 'Left & right'],
  ['center', 'Center'],
  ['scale', 'Scale'],
] as const
const V_CONSTRAINTS = [
  ['top', 'Top'],
  ['bottom', 'Bottom'],
  ['topBottom', 'Top & bottom'],
  ['center', 'Center'],
  ['scale', 'Scale'],
] as const

export function PositionSection() {
  const { t, run } = useDesignKit()
  const design = useDesign()
  const { answer, ids, setProps } = design
  // X, Y and rotation follow a drag on the canvas (the engine's answers, kit/gesture.ts).
  const nodes = useLiveNodes(design.nodes)
  const tipFor = useShortcutTip()
  const nested = answer.insideInstance === true
  const canAlign = answer.canAlign !== false
  const xOf = (n: (typeof nodes)[number]) => round2(n.x ?? 0)
  const yOf = (n: (typeof nodes)[number]) => round2(n.y ?? 0)
  const rotOf = (n: (typeof nodes)[number]) => round2(n.rotation ?? 0)
  const x = usePropField(ids, 'x', { of: xOf, nodes })
  const y = usePropField(ids, 'y', { of: yOf, nodes })
  const rot = usePropField(ids, 'rotation', { of: rotOf, nodes })
  const xs = common(nodes, xOf)
  const ys = common(nodes, yOf)
  const rs = common(nodes, rotOf)
  const parentAl = answer.allParentsAutoLayout === true
  const absolute = common(nodes, (n) => n.absolute === true) ?? false
  // Children of frames without auto layout (or ignoring it) carry constraints.
  const inFrame = nodes.every((n) => n.constraints !== undefined)
  const constraints = common(nodes, (n) => n.constraints ?? null)
  const distribute = useRef<HTMLSpanElement>(null)
  const [menu, setMenu] = useState(false)
  const align = (command: string) => void run(command, { ids })
  const disabledTip = nested ? t('Layers in an instance take their position from the main component') : undefined

  return (
    <>
      <SectionHeader title={t('Position')} />
      <div className="design-d-block" data-disabled={nested || undefined} data-tip={disabledTip} data-testid="design-design-position">
        <Sub a={t('Alignment')} />
        <Line>
          <span className="design-d-col3" data-tip={canAlign || nested ? undefined : t('A layer on its own aligns to its frame: select more layers, or one in a frame')}>
            <Segmented
              value={null}
              disabled={nested || !canAlign}
              segments={[
                { value: 'align.left', label: tipFor(t('Align left'), 'align.left'), icon: 'AlignLeft' },
                { value: 'align.hcenter', label: tipFor(t('Align horizontal centers'), 'align.hcenter'), icon: 'AlignHCenter' },
                { value: 'align.right', label: tipFor(t('Align right'), 'align.right'), icon: 'AlignRight' },
              ]}
              onChange={align}
              testId="design-design-align"
            />
          </span>
          <span className="design-d-col3">
            <Segmented
              value={null}
              disabled={nested || !canAlign}
              segments={[
                { value: 'align.top', label: tipFor(t('Align top'), 'align.top'), icon: 'AlignTop' },
                { value: 'align.vcenter', label: tipFor(t('Align vertical centers'), 'align.vcenter'), icon: 'AlignVCenter' },
                { value: 'align.bottom', label: tipFor(t('Align bottom'), 'align.bottom'), icon: 'AlignBottom' },
              ]}
              onChange={align}
              testId="design-design-align"
            />
          </span>
          {ids.length > 2 ? (
            <span ref={distribute} className="design-d-anchor">
              <IconButton icon="DistributeH" label={t('Distribute and tidy')} disabled={nested} onClick={() => setMenu(true)} testId="design-design-distribute" />
            </span>
          ) : null}
          {menu ? (
            <Menu
              tone="light"
              anchor={distribute.current}
              placement="below-start"
              gap={2}
              onClose={() => setMenu(false)}
              items={[
                { label: t('Distribute horizontal spacing'), shortcut: tipFor('', 'distribute.horizontal').trim(), onSelect: () => align('distribute.horizontal') },
                { label: t('Distribute vertical spacing'), shortcut: tipFor('', 'distribute.vertical').trim(), onSelect: () => align('distribute.vertical') },
                { label: t('Tidy up'), shortcut: tipFor('', 'arrange.tidy').trim(), onSelect: () => align('arrange.tidy') },
              ]}
            />
          ) : null}
        </Line>
        <Sub a={t('Position')} />
        <Line>
          <NumberField label="X" tip={t('X position')} value={xs} mixed={xs === null} width={COL} disabled={nested} onCommit={x.onCommit} scrub={x.scrub} testId="design-design-x" />
          <NumberField label="Y" tip={t('Y position')} value={ys} mixed={ys === null} width={COL} disabled={nested} onCommit={y.onCommit} scrub={y.scrub} testId="design-design-y" />
          {parentAl ? (
            <IconButton
              icon="AbsolutePosition"
              label={absolute ? t('Use auto layout') : t('Ignore auto layout (absolute position)')}
              pressed={absolute}
              disabled={nested}
              onClick={() => setProps({ absolute: !absolute })}
              testId="design-design-absolute"
            />
          ) : null}
        </Line>
        <Sub a={t('Rotation')} />
        <Line>
          <NumberField icon="Angle" tip={t('Rotation')} suffix="°" value={rs} mixed={rs === null} width={COL} disabled={nested} onCommit={rot.onCommit} scrub={rot.scrub} testId="design-design-rotation" />
          <span className="design-d-col3">
            <Segmented
              value={null}
              disabled={nested}
              segments={[
                { value: 'object.rotateRight', label: t('Rotate 90° right'), icon: 'Rotate' },
                { value: 'object.flipH', label: tipFor(t('Flip horizontal'), 'object.flipH'), icon: 'FlipH' },
                { value: 'object.flipV', label: tipFor(t('Flip vertical'), 'object.flipV'), icon: 'FlipV' },
              ]}
              onChange={align}
              testId="design-design-transform"
            />
          </span>
        </Line>
        {inFrame ? (
          <>
            <Sub a={t('Constraints')} />
            <Line>
              <Dropdown checks={false}
                value={constraints?.horizontal ?? null}
                options={H_CONSTRAINTS.map(([value, label]) => ({ value, label: t(label) }))}
                onChange={(value) => setProps({ constraints: { horizontal: value } })}
                mixedLabel={t('Mixed')}
                width={COL}
                disabled={nested}
                ariaLabel={t('Horizontal constraint')}
                testId="design-design-constraint-h"
              />
              <Dropdown checks={false}
                value={constraints?.vertical ?? null}
                options={V_CONSTRAINTS.map(([value, label]) => ({ value, label: t(label) }))}
                onChange={(value) => setProps({ constraints: { vertical: value } })}
                mixedLabel={t('Mixed')}
                width={COL}
                disabled={nested}
                ariaLabel={t('Vertical constraint')}
                testId="design-design-constraint-v"
              />
            </Line>
          </>
        ) : null}
      </div>
      <Space />
    </>
  )
}
