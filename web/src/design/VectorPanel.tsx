/**
 * The Design panel while a vector's points are edited (R-C16, the vector panel of 2.3): the
 * selected points' X and Y (the top-left of their box, in the parent's coordinates), their box's
 * width and height when there are several, the corner radius at them, how their handles mirror,
 * and the stroke's cap and join at them. It stands in for the layer's Position section. Every
 * field runs a `vector.edit.*` command; a typed value is one undo step, a scrub one step when let
 * go. With the Variable width tool the panel shows nothing of its own.
 */
import { useMemo } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Dropdown, NumberField, SectionHeader } from '../ui'
import { useCommandField } from './hooks'
import { COL, COL2, Line, Space, Sub } from './fields'
import { round2 } from './model'

type Vertex = { x: number; y: number; radius?: number; cap?: string | null; join?: string | null }
type EditState = {
  tool: string
  network: { vertices: Vertex[]; segments: Array<{ a: number; b: number }> }
  mirroring: string[]
  overlay: { selectedVertices: number[]; selectedSegments: number[] } | null
}
type Bounds = { x: number | null; y: number | null; width: number; height: number }

const CAPS: Array<[string, string]> = [
  ['none', 'None'],
  ['round', 'Round'],
  ['square', 'Square'],
  ['lineArrow', 'Line arrow'],
  ['triangleArrow', 'Triangle arrow'],
  ['reversedTriangle', 'Reversed triangle'],
  ['circleFilled', 'Circle'],
  ['diamondFilled', 'Diamond'],
]
const JOINS: Array<[string, string]> = [
  ['miter', 'Miter'],
  ['bevel', 'Bevel'],
  ['round', 'Round'],
]
const MIRRORING: Array<[string, string]> = [
  ['none', 'No mirroring'],
  ['angle', 'Mirror angle'],
  ['angleAndLength', 'Mirror angle and length'],
]
/** A point's own cap or join is unset: it takes the layer's. */
const LAYER = 'layer'

/** The value every item shares, else null (mixed). */
function shared<T>(items: readonly T[]): T | null {
  if (!items.length) return null
  const first = items[0]
  return items.every((item) => item === first) ? first : null
}

/** A number field running `command` with `{ [key]: value }`; relative edits apply to `shown`. */
function usePointField(command: string, key: string, shown: number | null, clamp: (v: number) => number = (v) => v) {
  return useCommandField((value) => ({ command, params: { [key]: clamp(value) } }), { ids: [0], of: () => shown, build: (_id, value) => ({ command, params: { [key]: clamp(value) } }) })
}

export function VectorSection() {
  const { t, run } = useDesignKit()
  const state = useQuery<{ state: EditState | null }>('vectorEdit')?.state ?? null
  const points = useMemo(() => {
    if (!state?.overlay) return []
    const out = new Set(state.overlay.selectedVertices)
    for (const s of state.overlay.selectedSegments) {
      const seg = state.network.segments[s]
      if (seg) {
        out.add(seg.a)
        out.add(seg.b)
      }
    }
    return [...out].sort((a, b) => a - b)
  }, [state])
  const bounds = useQuery<Bounds>('vectorBounds', { points }, { skip: points.length === 0 })
  const x = bounds?.x ?? null
  const y = bounds?.y ?? null
  const fx = usePointField('vector.edit.position', 'x', x == null ? null : round2(x))
  const fy = usePointField('vector.edit.position', 'y', y == null ? null : round2(y))
  const fw = usePointField('vector.edit.resize', 'width', bounds ? round2(bounds.width) : null, (v) => Math.max(0, v))
  const fh = usePointField('vector.edit.resize', 'height', bounds ? round2(bounds.height) : null, (v) => Math.max(0, v))
  const vertices = points.map((i) => state?.network.vertices[i]).filter((v): v is Vertex => v != null)
  const radius = shared(vertices.map((v) => round2(v.radius ?? 0)))
  const fr = usePointField('vector.edit.cornerRadius', 'radius', radius, (v) => Math.max(0, v))
  if (!state || state.tool === 'width') return null
  const several = points.length > 1
  const mirroring = shared(points.map((i) => state.mirroring[i] ?? 'none'))
  const cap = shared(vertices.map((v) => v.cap ?? LAYER))
  const join = shared(vertices.map((v) => v.join ?? LAYER))
  return (
    <div data-testid="design-vector-panel">
      <SectionHeader title={t(several ? 'Points' : 'Point')} />
      {points.length === 0 ? (
        <div className="design-note" data-testid="design-vector-none">
          {t('Select points to edit their position, radius and mirroring')}
        </div>
      ) : (
        <>
          <Sub a={t('Position')} />
          <Line>
            <NumberField label="X" value={x == null ? null : round2(x)} width={COL} onCommit={fx.onCommit} scrub={fx.scrub} testId="design-vector-x" />
            <NumberField label="Y" value={y == null ? null : round2(y)} width={COL} onCommit={fy.onCommit} scrub={fy.scrub} testId="design-vector-y" />
          </Line>
          {several && bounds ? (
            <>
              <Sub a={t('Dimensions')} />
              <Line>
                <NumberField label="W" value={round2(bounds.width)} width={COL} min={0} onCommit={fw.onCommit} scrub={fw.scrub} testId="design-vector-w" />
                <NumberField label="H" value={round2(bounds.height)} width={COL} min={0} onCommit={fh.onCommit} scrub={fh.scrub} testId="design-vector-h" />
              </Line>
            </>
          ) : null}
          <Sub a={t('Corner radius')} />
          <Line>
            <NumberField icon="CornerRadius" tip={t('Corner radius')} value={radius} mixed={radius === null} min={0} width={COL} onCommit={fr.onCommit} scrub={fr.scrub} testId="design-vector-radius" />
          </Line>
          <Sub a={t('Mirroring')} />
          <Line>
            <Dropdown
              checks={false}
              value={mirroring}
              mixedLabel={t('Mixed')}
              options={MIRRORING.map(([value, label]) => ({ value, label: t(label) }))}
              width={COL2}
              onChange={(mode) => void run('vector.edit.mirroring', { mode })}
              testId="design-vector-mirroring"
            />
          </Line>
          <Sub a={t('End point')} b={t('Join|stroke').split('|')[0]} />
          <Line>
            <Dropdown
              checks={false}
              value={cap}
              mixedLabel={t('Mixed')}
              options={[{ value: LAYER, label: t('Same as layer') }, ...CAPS.map(([value, label]) => ({ value, label: t(label) }))]}
              width={COL}
              onChange={(value) => void run('vector.edit.strokeCap', { cap: value === LAYER ? null : value })}
              testId="design-vector-cap"
            />
            <Dropdown
              checks={false}
              value={join}
              mixedLabel={t('Mixed')}
              options={[{ value: LAYER, label: t('Same as layer') }, ...JOINS.map(([value, label]) => ({ value, label: t(label) }))]}
              width={COL}
              onChange={(value) => void run('vector.edit.strokeJoin', { join: value === LAYER ? null : value })}
              testId="design-vector-join"
            />
          </Line>
        </>
      )}
      <Space />
    </div>
  )
}
