/**
 * Arc (every layer an ellipse) and Polygon / Star (every layer a polygon or star): 2.3 "Arc
 * (ellipses) and Polygon/Star", upstream `panels/right.rs` `arc_section` and `points_section`.
 */
import { useDesignKit } from '../kit/hooks'
import { NumberField, SectionHeader, Separator } from '../ui'
import { usePropField, useDesign } from './hooks'
import { BindableField, COL, Line, Space, Sub } from './fields'
import { common, round2, type NodeJson } from './model'

const FULL = { start: 0, sweep: 360, inner: 0 }

/** Shown when every layer is an ellipse; answers null otherwise. */
export function ArcSection() {
  const { t } = useDesignKit()
  const { nodes, ids } = useDesign()
  const start = usePropField(ids, 'arcStart')
  const sweep = usePropField(ids, 'arcSweep', { map: (v) => Math.min(100, Math.max(-100, v)) * 3.6 })
  const ratio = usePropField(ids, 'arcInner', { map: (v) => Math.min(0.99, Math.max(0, v / 100)) })
  if (!nodes.length || !nodes.every((n) => n.type === 'ELLIPSE')) return null
  const arc = (n: NodeJson) => n.arc ?? FULL
  const s = common(nodes, (n) => round2((((arc(n).start % 360) + 360) % 360)))
  const w = common(nodes, (n) => round2((arc(n).sweep / 360) * 100))
  const r = common(nodes, (n) => round2(arc(n).inner * 100))
  return (
    <div data-testid="design-design-arc">
      <SectionHeader title={t('Arc')} />
      <Sub a={t('Start')} b={t('Sweep')} />
      <Line>
        <BindableField ids={ids} nodes={nodes} field="arcStart" icon="ArcStart" tip={t('Arc start')} suffix="°" value={s} width={COL} onCommit={start.onCommit} scrub={start.scrub} testId="design-design-arc-start" />
        <BindableField ids={ids} nodes={nodes} field="arcSweep" icon="ArcSweep" tip={t('Arc sweep')} suffix="%" value={w} width={COL} onCommit={sweep.onCommit} scrub={sweep.scrub} testId="design-design-arc-sweep" />
      </Line>
      <Sub a={t('Ratio')} />
      <Line>
        <BindableField ids={ids} nodes={nodes} field="arcInner" icon="ArcRatio" tip={t('Arc ratio')} suffix="%" value={r} width={COL} onCommit={ratio.onCommit} scrub={ratio.scrub} testId="design-design-arc-ratio" />
      </Line>
      <Space />
      <Separator />
    </div>
  )
}

/** Shown when every layer is a polygon or a star; answers null otherwise. */
export function PointsSection() {
  const { t } = useDesignKit()
  const { nodes, ids } = useDesign()
  const count = usePropField(ids, 'pointCount', { map: (v) => Math.min(120, Math.max(3, Math.round(v))) })
  const ratio = usePropField(ids, 'innerRadius', { map: (v) => Math.min(1, Math.max(0.001, v / 100)) })
  if (!nodes.length || !nodes.every((n) => n.type === 'POLYGON' || n.type === 'STAR')) return null
  const stars = nodes.every((n) => n.type === 'STAR')
  const c = common(nodes, (n) => n.pointCount ?? 3)
  const r = stars ? common(nodes, (n) => Math.round((n.innerRadius ?? 0) * 1000) / 10) : null
  return (
    <div data-testid="design-design-points">
      <SectionHeader title={t(stars ? 'Star' : 'Polygon')} />
      <Sub a={t('Count')} b={stars ? t('Ratio') : null} />
      <Line>
        <NumberField icon="Polygon" tip={t('Count')} value={c} mixed={c === null} decimals={0} width={COL} onCommit={count.onCommit} scrub={count.scrub} testId="design-design-point-count" />
        {stars ? <NumberField icon="Star" tip={t('Ratio')} suffix="%" value={r} mixed={r === null} width={COL} onCommit={ratio.onCommit} scrub={ratio.scrub} testId="design-design-star-ratio" /> : null}
      </Line>
      <Space />
      <Separator />
    </div>
  )
}
