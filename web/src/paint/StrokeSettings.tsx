/**
 * A stroke's settings (2.4 "Stroke section", upstream `paints.rs` and `stroke.rs`): the alignment,
 * weight, individual sides and Advanced stroke row; per-side weights; and, while Advanced stroke is
 * open, the dash pattern, width profile, brush, join and miter angle, and the end points of open
 * paths.
 */
import { useEffect, useState, type ReactNode } from 'react'
import { useDesignKit, useNumberProp } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { Dropdown, IconButton, Menu, NumberField, Row, SubLabels, columnWidth, type MenuEntry } from '../ui'
import { type SelectionData, common, MIXED } from './data'
import { setStrokeAdvanced, usePaintState } from './store'

const v = <T,>(x: T | undefined | typeof MIXED): T | null | undefined => (x === MIXED ? null : x)

type Profile = { preset: string | null; points: Array<[number, number] | { t: number; scale?: number; left?: number; right?: number }> }

const PRESETS: Array<[string, string, Array<[number, number]> | null]> = [
  ['uniform', 'Uniform', null],
  ['wedge', 'Wedge', [[0, 1], [1, 0]]],
  ['taper', 'Taper both ends', [[0, 0], [0.2, 1], [0.8, 1], [1, 0]]],
  ['eye', 'Eye', [[0, 0], [0.5, 1], [1, 0]]],
  ['rounded', 'Rounded', [[0, 0.3], [0.05, 0.8], [0.15, 1], [0.85, 1], [0.95, 0.8], [1, 0.3]]],
  ['pinch', 'Pinch', [[0, 1], [0.5, 0.25], [1, 1]]],
]

const BRUSHES: Array<[string, string, 'stretch' | 'scatter']> = [
  ['taper', 'Taper', 'stretch'],
  ['ink', 'Ink', 'stretch'],
  ['ribbon', 'Ribbon', 'stretch'],
  ['bristle', 'Bristle', 'stretch'],
  ['dots', 'Dots', 'scatter'],
  ['stars', 'Stars', 'scatter'],
  ['confetti', 'Confetti', 'scatter'],
  ['leaves', 'Leaves', 'scatter'],
]

const CAPS: Array<[string, string]> = [
  ['None', 'None'],
  ['Round', 'Round'],
  ['Square', 'Square'],
  ['LineArrow', 'Line arrow'],
  ['TriangleArrow', 'Triangle arrow'],
  ['ReversedTriangle', 'Reversed triangle'],
  ['CircleFilled', 'Circle'],
  ['DiamondFilled', 'Diamond'],
]

/** Brush outlines (SVG path data) by `preset:width` (the engine's `strokePreviewOutline`). */
const brushArt = new Map<string, Promise<string | null>>()

/**
 * `brush_art`: the brush's outline along a short line, stroked with closed 1 px lines in the row's
 * text colour (upstream draws the outline, not the fill), from the engine's path data.
 */
function BrushArt({ preset, width }: { preset: string; width: number }) {
  const { invoke } = useDesignKit()
  const [path, setPath] = useState<string | null>(null)
  useEffect(() => {
    let live = true
    const key = `${preset}:${width}`
    let made = brushArt.get(key)
    if (!made) {
      made = invoke<string>('strokePreviewOutline', JSON.stringify({ preset, width, height: 10 })).catch(() => null)
      brushArt.set(key, made)
    }
    void made.then((value) => live && setPath(value))
    return () => {
      live = false
    }
  }, [invoke, preset, width])
  return (
    <svg className="design-brush-art" width={width} height={10} viewBox={`0 0 ${width} 10`} aria-hidden data-testid={`design-brush-art-${preset}`}>
      {path ? <path d={path} fill="none" stroke="currentColor" strokeWidth={1} strokeLinejoin="round" /> : null}
    </svg>
  )
}

/** `profile_art`: a width profile as a filled band, its thickest point the full height. */
/** A width profile's scale at `t`: Hermite with Fritsch–Carlson tangents, as the engine's `interp`. */
function profileScale(pts: Array<[number, number]>, t: number): number {
  const first = pts[0]
  const last = pts[pts.length - 1]
  if (!first || !last) return 1
  if (t <= first[0]) return first[1]
  if (t >= last[0]) return last[1]
  const secant = (i: number) => {
    const a = pts[i]
    const b = pts[i + 1]
    return a && b && b[0] - a[0] > 1e-12 ? (b[1] - a[1]) / (b[0] - a[0]) : 0
  }
  const tangent = (i: number) => {
    const end = pts.length - 1
    if (i === 0) return secant(0)
    if (i >= end) return secant(end - 1)
    const d0 = secant(i - 1)
    const d1 = secant(i)
    return d0 * d1 <= 0 ? 0 : (2 * d0 * d1) / (d0 + d1)
  }
  let k = 0
  while (k + 1 < pts.length && pts[k + 1][0] <= t) k += 1
  const a = pts[k]
  const b = pts[k + 1]
  if (!b) return last[1]
  const h = b[0] - a[0]
  if (h <= 1e-12) return b[1]
  const u = (t - a[0]) / h
  const u2 = u * u
  const u3 = u2 * u
  return Math.max(0, (2 * u3 - 3 * u2 + 1) * a[1] + (u3 - 2 * u2 + u) * h * tangent(k) + (3 * u2 - 2 * u3) * b[1] + (u3 - u2) * h * tangent(k + 1))
}

export function ProfileArt({ points, width = 56, height = 10 }: { points: Array<[number, number]> | null; width?: number; height?: number }) {
  const steps = 40
  const max = points ? Math.max(1e-6, ...points.map((p) => p[1])) : 1
  // Upstream's `profile_art`: a band whose half-height is the side's width over the profile's
  // largest (the thickest point fills the box); no profile is a uniform band half as tall. Widths
  // between points follow the engine's monotone cubic (`WidthProfile::interp`).
  const at = (t: number) => (points ? profileScale(points, t) / max : 0.5)
  const top: string[] = []
  const bottom: string[] = []
  for (let i = 0; i <= steps; i += 1) {
    const t = i / steps
    const half = Math.max(0.5, (at(t) * height) / 2)
    top.push(`${(t * width).toFixed(2)},${(height / 2 - half).toFixed(2)}`)
    bottom.unshift(`${(t * width).toFixed(2)},${(height / 2 + half).toFixed(2)}`)
  }
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="design-stroke-art" aria-hidden="true">
      {/* egui's mesh is not anti-aliased: upstream smooths its edges with 0.75 px hairlines. */}
      <polygon points={[...top, ...bottom].join(' ')} fill="currentColor" stroke="currentColor" strokeWidth={0.75} />
    </svg>
  )
}

/** `cap_art`: a short stroke ending in `cap` (on its left end for a start point), 64 wide. */
export function CapArt({ cap, start }: { cap: string; start: boolean }) {
  const y = 6
  const [left, right] = [6, 50]
  const [tip, tail, d] = start ? [left, right, -1] : [right, left, 1]
  const at = (back: number, dy = 0): [number, number] => [tip - d * back, y + dy]
  const line = (to: [number, number], w: number) => <line x1={tail} y1={y} x2={to[0]} y2={to[1]} stroke="currentColor" strokeWidth={w} />
  const poly = (pts: Array<[number, number]>) => <polygon points={pts.map((p) => p.join(',')).join(' ')} fill="currentColor" />
  let shape: ReactNode
  switch (cap) {
    case 'Round':
      shape = (
        <>
          {line(at(2), 4)}
          <circle cx={at(2)[0]} cy={y} r={2} fill="currentColor" />
        </>
      )
      break
    case 'Square':
      shape = line(at(-2), 4)
      break
    case 'LineArrow':
      shape = (
        <>
          {line(at(0), 1.5)}
          <polyline points={[at(5.5, -5.5), at(0), at(5.5, 5.5)].map((p) => p.join(',')).join(' ')} fill="none" stroke="currentColor" strokeWidth={1.5} />
        </>
      )
      break
    case 'TriangleArrow':
      shape = (
        <>
          {line(at(7), 1.5)}
          {poly([at(0), at(8, -4.5), at(8, 4.5)])}
        </>
      )
      break
    case 'ReversedTriangle':
      shape = (
        <>
          {line(at(8), 1.5)}
          {poly([at(8), at(0, -4.5), at(0, 4.5)])}
        </>
      )
      break
    case 'CircleFilled':
      shape = (
        <>
          {line(at(4), 1.5)}
          <circle cx={at(4)[0]} cy={y} r={4} fill="currentColor" />
        </>
      )
      break
    case 'DiamondFilled':
      shape = (
        <>
          {line(at(5), 1.5)}
          {poly([at(0), at(5, -5), at(10), at(5, 5)])}
        </>
      )
      break
    default:
      shape = line(at(0), 4)
  }
  return (
    <svg width={56} height={12} viewBox="0 0 56 12" className="design-stroke-art" aria-hidden="true">
      {shape}
    </svg>
  )
}

function profilePoints(profile: Profile | null | undefined): Array<[number, number]> | null {
  if (!profile) return null
  const preset = PRESETS.find(([id]) => id === profile.preset)
  if (preset) return preset[2]
  const pts = (profile.points ?? []).map((p) => (Array.isArray(p) ? ([p[0], p[1]] as [number, number]) : ([p.t, p.scale ?? ((p.left ?? 1) + (p.right ?? 1)) / 2] as [number, number])))
  return pts.length ? pts : null
}

export function StrokeSettings({ data }: { data: SelectionData }) {
  const { t, run } = useDesignKit()
  const ids = data.ids
  const { strokeAdvanced } = usePaintState()
  const weight = v(common<number>(data, 'strokeWeight'))
  const align = v(common<string>(data, 'strokeAlign'))
  const sides = v(common<number[] | null>(data, 'strokeSides'))
  const sided = data.nodes.every((n) => ['FRAME', 'COMPONENT', 'COMPONENT_SET', 'INSTANCE', 'RECTANGLE'].includes(n.type))
  const weightProp = useNumberProp(ids, 'strokeWeight', t('Stroke weight'), (id) => Number(data.nodes.find((n) => n.id === id)?.strokeWeight ?? 0))
  const [sidesMenu, setSidesMenu] = useState<Element | null>(null)
  const w = weight ?? 1
  const sideItems: MenuEntry[] = [
    [t('All'), null],
    [t('Top'), [w, 0, 0, 0]],
    [t('Right'), [0, w, 0, 0]],
    [t('Bottom'), [0, 0, w, 0]],
    [t('Left'), [0, 0, 0, w]],
    [t('Custom'), [w, w, w, w]],
  ].map(([label, value]) => ({ label: label as string, onSelect: () => void run('node.set', { ids, props: { strokeSides: value } }), testId: `design-stroke-sides-${String(label).toLowerCase()}` }))
  return (
    <>
      <Row testId="design-stroke-settings">
        <Dropdown checks={false}
          value={align ?? null}
          options={[
            { value: 'inside', label: t('Inside') },
            { value: 'center', label: t('Center') },
            { value: 'outside', label: t('Outside') },
          ]}
          width="calc((100% - 8px) / 2)"
          onChange={(strokeAlign) => void run('node.set', { ids, props: { strokeAlign } })}
          mixedLabel={t('Mixed')}
          ariaLabel={t('Stroke alignment')}
          testId="design-stroke-align"
        />
        <NumberField
          value={weight ?? null}
          mixed={weight === null}
          icon="Menu"
          tip={t('Stroke weight')}
          min={0}
          width={`calc((100% - 8px) / 2 - ${sided ? 60 : 32}px)`}
          onCommit={weightProp.onCommit}
          scrub={weightProp.scrub}
          testId="design-stroke-weight"
        />
        {sided ? <IconButton iconSize={16} icon="IndependentCorners" label={t('Individual strokes')} onClick={() => setSidesMenu(document.querySelector('[data-testid="design-stroke-sides"]'))} testId="design-stroke-sides" /> : null}
        <IconButton iconSize={16} icon="Dots" label={t('Advanced stroke')} pressed={strokeAdvanced} onClick={() => setStrokeAdvanced(!strokeAdvanced)} testId="design-stroke-advanced" />
      </Row>
      {sidesMenu ? <Menu tone="light" items={sideItems} anchor={sidesMenu} gap={0} minWidth={48} onClose={() => setSidesMenu(null)} testId="design-stroke-sides-menu" /> : null}
      {sides ? <SideWeights data={data} sides={sides} /> : null}
      {strokeAdvanced ? <Advanced data={data} /> : null}
    </>
  )
}

const SIDE_PROPS = ['strokeTopWeight', 'strokeRightWeight', 'strokeBottomWeight', 'strokeLeftWeight']

function SideField({ ids, index, value }: { ids: number[]; index: number; value: number | null }) {
  const { t } = useDesignKit()
  const prop = useNumberProp(ids, SIDE_PROPS[index], t('Stroke weight'))
  return <NumberField value={value} label={['T', 'R', 'B', 'L'][index]} min={0} width={columnWidth(240)} onCommit={prop.onCommit} scrub={prop.scrub} testId={`design-stroke-side-${index}`} />
}

function SideWeights({ data, sides }: { data: SelectionData; sides: number[] }) {
  return (
    <>
      {[0, 2].map((first) => (
        <Row key={first}>
          <SideField ids={data.ids} index={first} value={sides[first] ?? null} />
          <SideField ids={data.ids} index={first + 1} value={sides[first + 1] ?? null} />
        </Row>
      ))}
    </>
  )
}

function Advanced({ data }: { data: SelectionData }) {
  const { t, run } = useDesignKit()
  const ids = data.ids
  const set = (props: Record<string, unknown>) => void run('node.set', { ids, props })
  const dashes = v(common<number[]>(data, 'dashes')) ?? []
  const weight = Math.max(1, Number(v(common<number>(data, 'strokeWeight')) ?? 1))
  const dashed = dashes.length > 0
  const [dash, gap] = [dashes[0] ?? 4, dashes[1] ?? dashes[0] ?? 4]
  const profile = v(common<Profile>(data, 'widthProfile'))
  const profileMixed = data.mixed.includes('widthProfile')
  const profileLabel = profileMixed ? t('Mixed') : !profile ? t('Uniform') : t(PRESETS.find(([id]) => id === profile.preset)?.[1] ?? 'Custom')
  const brush = v(common<Record<string, unknown>>(data, 'brush'))
  const brushMixed = data.mixed.includes('brush')
  const brushPreset = BRUSHES.find(([id]) => id === brush?.preset)
  const join = data.mixed.includes('strokeJoin') ? null : (v(common<string>(data, 'strokeJoin')) ?? 'miter')
  const miter = v(common<number>(data, 'miterAngle')) ?? 28.96
  const open = data.nodes.every((n) => n.type === 'LINE' || n.type === 'VECTOR')
  // The engine answers camelCase (`lineArrow`); `node.set` takes either case.
  const caps = (key: string) => {
    if (data.mixed.includes(key)) return null
    const cap = String(v(common<string>(data, key)) ?? 'none').toLowerCase()
    return CAPS.find(([id]) => id.toLowerCase() === cap)?.[0] ?? 'None'
  }
  const pts = profileMixed ? null : profilePoints(profile)
  return (
    <div data-testid="design-stroke-advanced-panel">
      <SubLabels labels={[t('Stroke style')]} />
      <Row>
        <Dropdown checks={false}
          value={dashed ? 'dash' : 'solid'}
          options={[
            { value: 'solid', label: t('Solid') },
            { value: 'dash', label: t('Dash') },
          ]}
          width="calc((100% - 8px) / 2)"
          onChange={(style) => set({ dashes: style === 'dash' ? [weight * 4, weight * 4] : [] })}
          testId="design-stroke-style"
        />
      </Row>
      {dashed ? (
        <>
          <SubLabels labels={[t('Dash'), t('Gap')]} />
          <Row>
            <NumberField value={dash} width="calc((100% - 8px) / 2)" min={0} onCommit={(e) => e.kind === 'set' && set({ dashes: [Math.max(0, e.value), gap] })} testId="design-stroke-dash" />
            <NumberField value={gap} width="calc((100% - 8px) / 2)" min={0} onCommit={(e) => e.kind === 'set' && set({ dashes: [dash, Math.max(0, e.value)] })} testId="design-stroke-gap" />
          </Row>
        </>
      ) : null}
      <SubLabels labels={[t('Width profile')]} />
      <Row>
        <ArtDropdown
          label={profileLabel}
          mixed={profileMixed}
          art={profileMixed ? null : <ProfileArt points={pts} />}
          width="calc(100% - 32px)"
          testId="design-stroke-profile"
          items={PRESETS.map(([id, label, points]) => ({ label: t(label), art: <ProfileArt points={points} width={64} />, onSelect: () => void run('stroke.widthProfile', { ids, preset: id }), testId: `design-stroke-profile-${id}` }))}
        />
        <IconButton
          icon="FlipH"
          label={t('Flip width profile')}
          testId="design-stroke-profile-flip"
          onClick={() => {
            if (!pts) return
            void run('stroke.widthProfile', { ids, points: [...pts].reverse().map(([x, s]) => [1 - x, s]) })
          }}
        />
      </Row>
      <SubLabels labels={[t('Brush')]} />
      <Row>
        <ArtDropdown
          label={brushMixed ? t('Mixed') : t(brushPreset?.[1] ?? 'None')}
          mixed={brushMixed}
          art={!brushMixed && brushPreset ? <BrushArt preset={brushPreset[0]} width={56} /> : null}
          width="100%"
          testId="design-stroke-brush"
          items={[
            { label: t('None'), onSelect: () => void run('stroke.brush', { ids, preset: null }), testId: 'design-stroke-brush-none' },
            ...BRUSHES.map(([id, label]) => ({ label: t(label), art: <BrushArt preset={id} width={64} />, onSelect: () => void run('stroke.brush', { ids, preset: id }), testId: `design-stroke-brush-${id}` })),
          ]}
        />
      </Row>
      {brush && brushPreset?.[2] === 'scatter' ? <ScatterFields ids={ids} brush={brush} /> : null}
      <SubLabels labels={join === 'miter' ? [t('Join|stroke').split('|')[0], t('Miter angle')] : [t('Join|stroke').split('|')[0]]} />
      <Row>
        <Dropdown checks={false}
          value={join}
          options={[
            { value: 'miter', label: t('Miter') },
            { value: 'bevel', label: t('Bevel') },
            { value: 'round', label: t('Round') },
          ]}
          width="calc((100% - 8px) / 2)"
          onChange={(strokeJoin) => set({ strokeJoin })}
          testId="design-stroke-join"
        />
        {join === 'miter' ? <NumberField value={Math.round(miter * 100) / 100} suffix="°" width="calc((100% - 8px) / 2)" onCommit={(e) => e.kind === 'set' && set({ miterAngle: e.value })} testId="design-stroke-miter" /> : null}
      </Row>
      {open ? (
        <>
          <SubLabels labels={[t('Start point'), t('End point')]} />
          <Row>
            {(['startCap', 'endCap'] as const).map((key) => (
              <ArtDropdown
                key={key}
                label={caps(key) === null ? t('Mixed') : t(CAPS.find(([id]) => id === caps(key))?.[1] ?? 'None')}
                mixed={caps(key) === null}
                width="calc((100% - 8px) / 2)"
                testId={`design-stroke-${key}`}
                items={CAPS.map(([id, l]) => ({ label: t(l), art: <CapArt cap={id} start={key === 'startCap'} />, onSelect: () => set({ [key]: id }),
                testId: `design-stroke-${key}-${id}`,
              }))}
              />
            ))}
          </Row>
        </>
      ) : null}
      <div style={{ height: 4 }} />
    </div>
  )
}

/** A dropdown whose button and menu rows draw art at the right (width profiles, caps). */
function ArtDropdown({ label, mixed, art, items, width, testId }: { label: string; mixed?: boolean; art?: ReactNode; items: MenuEntry[]; width: string | number; testId: string }) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null)
  return (
    <>
      <button type="button" className="design-dropdown design-art-dropdown" style={{ width }} aria-haspopup="menu" aria-expanded={Boolean(anchor)} data-testid={testId} onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}>
        <span className="design-dropdown-label" data-mixed={mixed || undefined}>
          {label}
        </span>
        {art ? <span className="design-art-dropdown-art">{art}</span> : null}
        <Icon name="Chevron" size={12} className="design-chevron" />
      </button>
      {anchor ? <Menu tone="light" anchor={anchor} gap={0} minWidth={anchor.offsetWidth} onClose={() => setAnchor(null)} testId={`${testId}-menu`} items={items} /> : null}
    </>
  )
}

function ScatterFields({ ids, brush }: { ids: number[]; brush: Record<string, unknown> }) {
  const { t, run } = useDesignKit()
  const set = (patch: Record<string, unknown>) => void run('node.set', { ids, props: { brush: { ...brush, ...patch } } })
  const n = (key: string, fallback: number) => Number(brush[key] ?? fallback)
  const half = 'calc((100% - 8px) / 2)'
  return (
    <>
      <SubLabels labels={[t('Spacing'), t('Scatter')]} />
      <Row>
        <NumberField value={n('spacing', 1)} icon="Gap" tip={t('Brush spacing')} suffix="×" step={0.1} width={half} onCommit={(e) => e.kind === 'set' && set({ spacing: e.value })} />
        <NumberField value={n('scatter', 0)} icon="Dots" tip={t('Brush scatter')} suffix="×" step={0.1} width={half} onCommit={(e) => e.kind === 'set' && set({ scatter: e.value })} />
      </Row>
      <SubLabels labels={[t('Size jitter'), t('Rotation jitter')]} />
      <Row>
        <NumberField value={Math.round(n('sizeJitter', 0) * 100)} icon="Scale" tip={t('Brush size jitter')} suffix="%" width={half} onCommit={(e) => e.kind === 'set' && set({ sizeJitter: e.value / 100 })} />
        <NumberField value={Math.round(n('rotationJitter', 0) * 100)} icon="Rotate" tip={t('Brush rotation jitter')} suffix="%" width={half} onCommit={(e) => e.kind === 'set' && set({ rotationJitter: e.value / 100 })} />
      </Row>
    </>
  )
}
