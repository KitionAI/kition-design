/**
 * Appearance (2.3, upstream `panels/right/paints.rs` `appearance`): Show/Hide, a variable for
 * visibility, the Blend mode row toggle; mask type, opacity, corner radius (one, or per corner),
 * corner smoothing with "iOS", and the blend mode.
 */
import { useState } from 'react'
import { useDesignKit } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { Button, Dropdown, IconButton, NumberField, SectionHeader, type MenuEntry } from '../ui'
import { useDesign, usePropField, useShortcutTip } from './hooks'
import { BindableField, COL, COL2, Line, MenuField, Space, Sub, openVariables, useVariables } from './fields'
import { BLEND_MENU, blendKey, blendLabel, blendOf, common, hasCornerRadius, hasCornerSmoothing, hasIndependentCorners, opacityOf, radiiOf, round2, uniformRadius, visibleOf, type NodeJson } from './model'

/** Individual corners and the blend row are one toggle each for the whole panel (egui temp data upstream). */
const open = { corners: false, blend: false }

const MASK_TYPES = [
  ['alpha', 'Alpha'],
  ['vector', 'Vector'],
  ['luminance', 'Luminance'],
] as const

export function AppearanceSection() {
  const { t, run } = useDesignKit()
  const { nodes, ids, setProps } = useDesign()
  const tipFor = useShortcutTip()
  const variables = useVariables()
  const [corners, setCorners] = useState(open.corners)
  const [blendOpen, setBlendOpen] = useState(open.blend)
  const visible = common(nodes, visibleOf) ?? true
  const opOf = (n: NodeJson) => Math.round(opacityOf(n) * 100)
  const op = common(nodes, opOf)
  const opacity = usePropField(ids, 'opacity', { of: opOf, nodes, map: (v) => Math.min(1, Math.max(0, v / 100)) })
  const radiusOk = nodes.every(hasCornerRadius)
  const indepOk = radiusOk && nodes.every(hasIndependentCorners)
  const smoothOk = radiusOk && nodes.every(hasCornerSmoothing)
  const radiusOf = (n: NodeJson) => uniformRadius(n) ?? Math.max(...radiiOf(n))
  const r = common(nodes, (n) => {
    const u = uniformRadius(n)
    return u === null ? null : round2(u)
  })
  const radius = usePropField(ids, 'cornerRadius', { of: radiusOf, nodes, map: (v) => Math.max(0, v) })
  const clamp0 = { map: (v: number) => Math.max(0, v) }
  const cornerFields = {
    topLeftRadius: usePropField(ids, 'topLeftRadius', clamp0),
    topRightRadius: usePropField(ids, 'topRightRadius', clamp0),
    bottomLeftRadius: usePropField(ids, 'bottomLeftRadius', clamp0),
    bottomRightRadius: usePropField(ids, 'bottomRightRadius', clamp0),
  }
  const smoothing = usePropField(ids, 'cornerSmoothing', { map: (v) => Math.min(1, Math.max(0, v / 100)) })
  const blend = common(nodes, blendOf)
  const showBlend = blendOpen || (blend !== null && !['normal', 'passthrough'].includes(blendKey(blend)))
  const radii = common(nodes, radiiOf)
  const masks = nodes.length > 0 && nodes.every((n) => n.isMask)
  const maskType = common(nodes, (n) => n.maskType ?? 'alpha')
  const boundVisible = common(nodes, (n) => n.boundVariables?.visible ?? null)
  const bind = (variable: string | null) => void run('variable.bind', { ids, field: 'visible', variable })
  const blendItems: MenuEntry[] = BLEND_MENU.map((label): MenuEntry => (label ? { label: t(label), onSelect: () => setProps({ blend: label }), testId: `design-design-blend-${blendKey(label)}` } : { type: 'separator' }))

  return (
    <div data-testid="design-design-appearance">
      <SectionHeader
        title={t('Appearance')}
        actions={[
          { icon: visible ? 'Eye' : 'EyeOff', label: tipFor(t('Show/Hide'), 'object.toggleVisible'), onClick: () => void run('object.toggleVisible', { ids }), testId: 'design-design-visibility' },
          {
            icon: 'Variable',
            label: t('Apply variable to visibility'),
            onClick: () => {
              const anchor = document.querySelector('[data-testid="design-design-visibility-variable"]')
              if (anchor) openVariables(ids, 'visible', 'boolean', anchor)
            },
            testId: 'design-design-visibility-variable',
          },
          {
            icon: 'Blend',
            label: t('Blend mode'),
            onClick: () => {
              open.blend = !blendOpen
              setBlendOpen(!blendOpen)
            },
            testId: 'design-design-blend-toggle',
          },
        ]}
      />
      {boundVisible ? (
        <Line>
          <div className="design-field design-d-bound" style={{ width: COL2 }} data-tip={variables.find((v) => v.id === boundVisible)?.name}>
            <span className="design-d-bound-lead">
              <Icon name="Eye" size={14} />
            </span>
            <span className="design-d-chip">{(variables.find((v) => v.id === boundVisible)?.name ?? boundVisible).split('/').pop()}</span>
            <button type="button" className="design-d-detach" data-tip={t('Detach variable')} aria-label={t('Detach variable')} onClick={() => bind(null)}>
              <Icon name="Detach" size={12} />
            </button>
          </div>
        </Line>
      ) : null}
      {masks ? (
        <>
          <Sub a={t('Mask')} />
          <Line>
            <Dropdown checks={false} value={maskType} icon="Mask" options={MASK_TYPES.map(([value, label]) => ({ value, label: t(label) }))} mixedLabel={t('Mixed')} onChange={(value) => setProps({ maskType: value })} width={COL2} ariaLabel={t('Mask')} testId="design-design-mask-type" />
          </Line>
        </>
      ) : null}
      <Sub a={t('Opacity')} b={radiusOk ? t('Corner radius') : null} />
      <Line>
        <BindableField ids={ids} nodes={nodes} field="opacity" icon="Opacity" tip={t('Opacity')} suffix="%" value={op} mixed={op === null} min={0} max={100} width={COL} onCommit={opacity.onCommit} scrub={opacity.scrub} testId="design-design-opacity" />
        {radiusOk ? <BindableField ids={ids} nodes={nodes} field="cornerRadius" icon="CornerRadius" tip={t('Corner radius')} value={r} mixed={r === null} min={0} width={COL} onCommit={radius.onCommit} scrub={radius.scrub} testId="design-design-radius" /> : null}
        {radiusOk && smoothOk ? (
          <IconButton
            icon="IndependentCorners"
            label={indepOk ? t('Individual corners') : t('Corner smoothing')}
            pressed={corners}
            onClick={() => {
              open.corners = !corners
              setCorners(!corners)
            }}
            testId="design-design-corners-individual"
          />
        ) : null}
      </Line>
      {smoothOk && corners ? (
        <>
          {indepOk
            ? (
                [
                  [
                    ['topLeftRadius', 'Top left radius', 'CornerTopLeft', 0],
                    ['topRightRadius', 'Top right radius', 'CornerTopRight', 1],
                  ],
                  [
                    ['bottomLeftRadius', 'Bottom left radius', 'CornerBottomLeft', 3],
                    ['bottomRightRadius', 'Bottom right radius', 'CornerBottomRight', 2],
                  ],
                ] as const
              ).map((pair, row) => (
                <Line key={row}>
                  {pair.map(([key, name, icon, k]) => (
                    <BindableField key={key} ids={ids} nodes={nodes} field={key} icon={icon} tip={t(name)} value={radii ? radii[k] : null} mixed={radii === null} min={0} width={COL} onCommit={cornerFields[key].onCommit} scrub={cornerFields[key].scrub} testId={`design-design-${key}`} />
                  ))}
                </Line>
              ))
            : null}
          <Sub a={t('Corner smoothing')} />
          <Line>
            <NumberField
              value={common(nodes, (n) => Math.round((n.cornerSmoothing ?? 0) * 100))}
              mixed={common(nodes, (n) => n.cornerSmoothing ?? 0) === null}
              suffix="%"
              min={0}
              max={100}
              width={COL}
              ariaLabel={t('Corner smoothing')}
              onCommit={smoothing.onCommit}
              scrub={smoothing.scrub}
              testId="design-design-smoothing"
            />
            <Button tip={t('Set corner smoothing to 60%')} onClick={() => setProps({ cornerSmoothing: 0.6 })} testId="design-design-smoothing-ios">
              {t('iOS')}
            </Button>
          </Line>
        </>
      ) : null}
      {showBlend ? (
        <>
          <Sub a={t('Blend mode')} />
          <Line>
            <MenuField label={blend ? t(blendLabel(blend)) : t('Mixed')} mixed={blend === null} icon="Blend" items={blendItems} width={COL2} menuWidth={200} ariaLabel={t('Blend mode')} testId="design-design-blend" />
          </Line>
        </>
      ) : null}
      <Space />
    </div>
  )
}
