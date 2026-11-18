/**
 * The Type settings popover (2.4, upstream `typography/fields.rs` `type_settings`, `axes.rs`,
 * `glyphs.rs`): Basics (decoration and how it looks, letter case, list style and spacing,
 * paragraph indent, hanging punctuation and lists, vertical trim, truncation and max lines),
 * Details (the font's OpenType features, from `query('font')`) and, for a variable font, Variable
 * (named instances and an axis slider per visible axis, previewed while dragged).
 */
import { useRef, useState, type ReactNode } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon } from '../icons/Icon'
import { Checkbox, NumberField, Popover, TextField } from '../ui'
import { parseCss } from './color'
import type { SelectionData } from './data'
import { trimNum } from './spacing'
import { uniform, type useTextWriter } from './Typography'

const FEATURES: Array<[string, string, boolean]> = [
  ['liga', 'Standard ligatures', true],
  ['calt', 'Contextual alternates', true],
  ['tnum', 'Tabular numbers', false],
  ['frac', 'Fractions', false],
  ['case', 'Case-sensitive forms', false],
  ['zero', 'Slashed zero', false],
  ['ss01', 'Stylistic set 1', false],
  ['ss02', 'Stylistic set 2', false],
  ['ss03', 'Stylistic set 3', false],
  ['ss04', 'Stylistic set 4', false],
  ['ss05', 'Stylistic set 5', false],
  ['smcp', 'Small capitals', false],
]

/** The tab is remembered across openings. */
let lastTab = 0

type Seg = { label: ReactNode; tip: string; value: string }

/** `text_seg`: a full-width segmented control of short text labels. */
function TextSeg({ items, value, onChange, testId }: { items: Seg[]; value: string | null; onChange: (value: string) => void; testId: string }) {
  const { t } = useDesignKit()
  return (
    <div className="design-text-seg" role="group" data-testid={testId}>
      {items.map((item) => (
        <button key={item.value} type="button" className="design-text-seg-cell" aria-pressed={item.value === value} aria-label={t(item.tip)} data-tip={t(item.tip)} data-testid={`${testId}-${item.value}`} onClick={() => onChange(item.value)}>
          <span>{item.label}</span>
        </button>
      ))}
    </div>
  )
}

const Caption = ({ children }: { children: ReactNode }) => <div className="design-ts-caption">{children}</div>

function FieldRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="design-ts-field">
      <span className="design-ts-field-label">{label}</span>
      <span className="design-ts-field-control">{children}</span>
    </div>
  )
}

type FontInfo = { family: string; style: string; found: boolean; styles: string[]; features: string[]; axes: Array<{ tag: string; name: string; min: number; default: number; max: number; hidden: boolean }>; instances: Array<{ name: string; variations: Record<string, number> }> }

export function TypeSettings({ anchor, data, ids, styles, writer, onClose }: { anchor: Element; data: SelectionData; ids: number[]; styles: Array<Record<string, unknown>>; writer: ReturnType<typeof useTextWriter>; onClose: () => void }) {
  const { t } = useDesignKit()
  const family = uniform(styles, (s) => String(s.fontFamily ?? ''))
  const style = uniform(styles, (s) => String(s.fontStyle ?? ''))
  const font = useQuery<FontInfo>('font', { family: family ?? '', style: style ?? '' }, { skip: !family })
  const variable = Boolean(font?.found && font.axes.length > 0)
  const tabs = variable ? ['Basics', 'Details', 'Variable'] : ['Basics', 'Details']
  const [tab, setTab] = useState(Math.min(lastTab, tabs.length - 1))
  return (
    <Popover open anchor={anchor} placement="below-end" gap={0} width={264} onClose={onClose} className="design-type-settings" testId="design-type-settings" label={t('Type settings')}>
      <div className="design-ts-tabs" role="tablist">
        {tabs.map((label, i) => (
          <button
            key={label}
            type="button"
            role="tab"
            aria-selected={tab === i}
            className="design-ts-tab"
            data-testid={`design-ts-tab-${label.toLowerCase()}`}
            onClick={() => {
              lastTab = i
              setTab(i)
            }}
          >
            {t(label)}
          </button>
        ))}
      </div>
      {tab === 0 ? <Basics data={data} ids={ids} styles={styles} writer={writer} /> : tab === 1 ? <Details ids={ids} data={data} font={font} family={family} style={style} /> : <Variable ids={ids} styles={styles} font={font} writer={writer} />}
    </Popover>
  )
}

function Basics({ data, ids, styles, writer }: { data: SelectionData; ids: number[]; styles: Array<Record<string, unknown>>; writer: ReturnType<typeof useTextWriter> }) {
  const { t, run, toast } = useDesignKit()
  const nodes = data.nodes.filter((n) => ids.includes(n.id))
  const set = (props: Record<string, unknown>) => void run('node.set', { ids, props })
  const deco = uniform(styles, (s) => String(s.decoration ?? s.textDecoration ?? 'none').toLowerCase())
  const look = (key: string, fallback: unknown) => uniform(nodes, (n) => n[key] ?? fallback)
  const decoStyle = look('textDecorationStyle', 'solid') as string | null
  const thickness = look('textDecorationThickness', 'auto') as string | number | null
  const offset = look('textDecorationOffset', 'auto') as string | number | null
  const skipInk = look('textDecorationSkipInk', true) as boolean | null
  const decoColor = look('textDecorationColor', 'auto') as string | null
  const textCase = uniform(styles, (s) => String(s.case ?? s.textCase ?? 'original'))
  const list = uniform(nodes, (n) => String(n.listStyle ?? (Array.isArray(n.lineTypes) && new Set(n.lineTypes as string[]).size === 1 ? (n.lineTypes as string[])[0] : n.lineTypes ? 'mixed' : 'none')).toLowerCase())
  const listSpacing = uniform(nodes, (n) => Number(n.listSpacing ?? 0))
  const indent = uniform(nodes, (n) => Number(n.paragraphIndent ?? 0))
  const startAligned = uniform(nodes, (n) => !['center', 'justified'].includes(String(n.textAlign ?? 'left')))
  const trim = uniform(nodes, (n) => String(n.leadingTrim ?? 'none'))
  const resize = uniform(nodes, (n) => String(n.textAutoResize ?? 'none'))
  const truncating = uniform(nodes, (n) => n.textAutoResize === 'truncate' || (n.textAutoResize === 'height' && Number(n.maxLines ?? 0) > 0))
  const maxLines = uniform(nodes, (n) => Number(n.maxLines ?? 0))
  const lengthField = (key: string, label: string, value: string | number | null) => {
    const auto = typeof value === 'string' && value.toLowerCase() === 'auto'
    const px = typeof value === 'number' ? value : null
    const empty = value === null ? t('Mixed') : auto ? t('Auto') : typeof value === 'string' ? value : ''
    return (
      <FieldRow label={label}>
        <NumberField
          value={px}
          placeholder={empty}
          step={0.5}
          width={auto ? '100%' : 'calc(100% - 36px)'}
          onCommit={(edit) => {
            if (edit.kind !== 'set') return
            const v = key === 'textDecorationThickness' && edit.value <= 0 ? 'auto' : Math.min(1000, Math.max(-1000, edit.value))
            void writer.commit({ [key]: v })
          }}
          testId={`design-ts-${key}`}
        />
        {!auto ? (
          <button type="button" className="design-ts-link" data-tip={t("Back to the font's value")} onClick={() => void writer.commit({ [key]: 'auto' })}>
            {t('Auto')}
          </button>
        ) : null}
      </FieldRow>
    )
  }
  return (
    <div className="design-ts-basics">
      <Caption>{t('Decoration')}</Caption>
      <TextSeg
        testId="design-ts-decoration"
        value={deco}
        onChange={(v) => void writer.commit({ textDecoration: v })}
        items={[
          { value: 'none', label: '—', tip: 'None' },
          { value: 'underline', label: <span style={{ textDecoration: 'underline' }}>U</span>, tip: 'Underline' },
          { value: 'strikethrough', label: <span style={{ textDecoration: 'line-through' }}>S</span>, tip: 'Strikethrough' },
        ]}
      />
      {deco === 'underline' || deco === 'strikethrough' ? (
        <>
          <Caption>{t('Decoration style')}</Caption>
          <TextSeg
            testId="design-ts-decoration-style"
            value={decoStyle}
            onChange={(v) => void writer.commit({ textDecorationStyle: v })}
            items={[
              { value: 'solid', label: t('Solid'), tip: 'Solid' },
              { value: 'dotted', label: t('Dotted'), tip: 'Dotted' },
              { value: 'wavy', label: t('Wavy'), tip: 'Wavy' },
            ]}
          />
          <div style={{ height: 4 }} />
          {lengthField('textDecorationThickness', t('Thickness'), thickness)}
          {deco === 'underline' ? lengthField('textDecorationOffset', t('Offset'), offset) : null}
          {deco === 'underline' ? (
            <div className="design-ts-check">
              <Checkbox checked={skipInk === true} label={t('Skip ink')} onChange={() => void writer.commit({ textDecorationSkipInk: skipInk !== true })} testId="design-ts-skip-ink" />
            </div>
          ) : null}
          <FieldRow label={t('Colour')}>
            <TextField
              value={decoColor === null || decoColor === 'auto' ? '' : decoColor}
              placeholder={decoColor === null ? t('Mixed') : t('Auto')}
              width="100%"
              testId="design-ts-decoration-color"
              onCommit={(text) => {
                const s = text.trim()
                if (!s || s.toLowerCase() === 'auto') void writer.commit({ textDecorationColor: 'auto' })
                else {
                  const c = parseCss(s)
                  if (c) void writer.commit({ textDecorationColor: `#${[c.r, c.g, c.b].map((x) => Math.round(x * 255).toString(16).padStart(2, '0')).join('').toUpperCase()}` })
                  else toast(t('{value} isn\'t a colour', { value: s }))
                }
              }}
            />
          </FieldRow>
        </>
      ) : null}
      <Caption>{t('Letter case')}</Caption>
      <TextSeg
        testId="design-ts-case"
        value={textCase}
        onChange={(v) => void writer.commit({ textCase: v })}
        items={[
          { value: 'original', label: '—', tip: 'As typed' },
          { value: 'upper', label: 'AG', tip: 'Uppercase' },
          { value: 'lower', label: 'ag', tip: 'Lowercase' },
          { value: 'title', label: 'Ag', tip: 'Title case' },
          {
            value: 'smallCaps',
            label: (
              <span>
                A<span style={{ fontSize: 8.5 }}>G</span>
              </span>
            ),
            tip: 'Small caps',
          },
        ]}
      />
      <Caption>{t('List style')}</Caption>
      <TextSeg
        testId="design-ts-list"
        value={list}
        onChange={(style) => void run('text.listStyle', { ids, style })}
        items={[
          { value: 'none', label: '—', tip: 'No list' },
          { value: 'bulleted', label: '•', tip: 'Bulleted list' },
          { value: 'numbered', label: '1.', tip: 'Numbered list' },
        ]}
      />
      <div style={{ height: 6 }} />
      <FieldRow label={t('List spacing')}>
        <NumberField value={listSpacing} mixed={listSpacing === null} width="100%" onCommit={(e) => e.kind === 'set' && set({ listSpacing: Math.min(10000, Math.max(0, e.value)) })} testId="design-ts-list-spacing" />
      </FieldRow>
      <div style={{ height: 4 }} />
      <div data-tip={startAligned === false ? t('Paragraph indent applies to left-aligned text') : undefined}>
        <FieldRow label={t('Paragraph indent')}>
          <NumberField
            value={indent}
            width="100%"
            disabled={startAligned === false}
            onCommit={(e) => {
              if (e.kind !== 'set') return
              set({ paragraphIndent: Math.min(10000, Math.max(0, e.value)) })
            }}
            testId="design-ts-indent"
          />
        </FieldRow>
      </div>
      <div style={{ height: 4 }} />
      {(['hangingPunctuation', 'hangingList'] as const).map((key) => {
        const on = nodes.every((n) => n[key] === true)
        return (
          <div key={key} className="design-ts-check">
            <Checkbox checked={on} label={t(key === 'hangingList' ? 'Hanging list' : 'Hanging punctuation')} onChange={() => set({ [key]: !on })} testId={`design-ts-${key}`} />
          </div>
        )
      })}
      <Caption>{t('Vertical trim')}</Caption>
      <TextSeg
        testId="design-ts-trim"
        value={trim === null ? null : trim === 'capHeight' ? 'capHeight' : 'none'}
        onChange={(v) => set({ leadingTrim: v })}
        items={[
          { value: 'none', label: t('Standard'), tip: 'Standard' },
          { value: 'capHeight', label: 'Ag', tip: 'Cap height to baseline' },
        ]}
      />
      <Caption>{t('Truncate text')}</Caption>
      <TextSeg
        testId="design-ts-truncate"
        value={truncating === null ? null : truncating ? 'on' : 'off'}
        onChange={(v) => {
          if (v === 'on' && truncating !== true) set({ textAutoResize: 'truncate' })
          else if (v === 'off' && resize === 'height') {
            if (truncating === true) set({ maxLines: 0 })
          } else if (v === 'off' && resize !== 'widthAndHeight') set({ textAutoResize: 'none' })
        }}
        items={[
          { value: 'off', label: t('Off'), tip: "Don't truncate" },
          { value: 'on', label: '…', tip: 'Truncate with an ellipsis' },
        ]}
      />
      {truncating === true ? (
        <>
          <div style={{ height: 4 }} />
          <FieldRow label={t('Max lines')}>
            <NumberField
              value={maxLines && maxLines > 0 ? maxLines : null}
              placeholder={maxLines === null ? t('Mixed') : t('Auto')}
              width="100%"
              onCommit={(e) => e.kind === 'set' && void run('text.maxLines', { ids, lines: Math.min(10000, Math.max(0, Math.round(e.value))) })}
              testId="design-ts-max-lines"
            />
          </FieldRow>
        </>
      ) : null}
    </div>
  )
}

function Details({ ids, data, font, family, style }: { ids: number[]; data: SelectionData; font: FontInfo | null; family: string | null; style: string | null }) {
  const { t, run } = useDesignKit()
  // The OpenType toggles set on the layers (`features`); a tag they set differently shows its default.
  const nodes = data.nodes.filter((n) => ids.includes(n.id))
  const features: Record<string, boolean | null> = {}
  for (const tag of new Set(nodes.flatMap((n) => Object.keys((n.features as Record<string, boolean> | undefined) ?? {})))) {
    features[tag] = uniform(nodes, (n) => (n.features as Record<string, boolean> | undefined)?.[tag] ?? null)
  }
  const name = family && style ? `${font?.family ?? family} ${font?.style ?? style}` : t('Mixed fonts')
  return (
    <div className="design-ts-details">
      <div className="design-ts-face">{name}</div>
      <div className="design-ts-features">
        {FEATURES.map(([tag, label, defaultOn]) => {
          const set = features[tag]
          const on = set === undefined || set === null ? defaultOn : set
          const supported = !font?.found || font.features.includes(tag)
          return (
            <button
              key={tag}
              type="button"
              className="design-ts-feature"
              role="checkbox"
              aria-checked={on}
              data-unsupported={!supported || undefined}
              data-tip={supported ? undefined : t('Not in this font')}
              data-testid={`design-ts-feature-${tag}`}
              onClick={() => {
                const want = !on
                const value = want === defaultOn ? null : want
                void run('text.setFeatures', { ids, features: { [tag]: value } })
              }}
            >
              <span className="design-check-box">{on ? <Icon name="Check" size={11} /> : null}</span>
              <span className="design-ts-feature-label">{t(label)}</span>
              <span className="design-ts-feature-tag">{tag}</span>
            </button>
          )
        })}
      </div>
    </div>
  )
}

function Variable({ ids, styles, font, writer }: { ids: number[]; styles: Array<Record<string, unknown>>; font: FontInfo | null; writer: ReturnType<typeof useTextWriter> }) {
  const { t, run } = useDesignKit()
  const [listOpen, setListOpen] = useState(false)
  if (!font) return null
  const variations = uniform(styles, (s) => JSON.stringify(s.fontVariations ?? s.variations ?? {}))
  const custom = variations !== null && variations !== '{}'
  const style = uniform(styles, (s) => String(s.fontStyle ?? ''))
  const instance = font.instances.find((i) => i.name === style)
  const label = variations === null ? t('Mixed') : custom || !instance ? t('Custom') : instance.name
  const own = variations ? (JSON.parse(variations) as Record<string, number>) : {}
  const params = (v: Record<string, unknown>) => ({ ...writer.params({}), variations: v })
  return (
    <div className="design-ts-variable">
      <div className="design-ts-face">{font.family}</div>
      {font.instances.length ? (
        <>
          <button type="button" className="design-dropdown" style={{ width: '100%' }} data-tip={t('Named instance')} aria-expanded={listOpen} onClick={() => setListOpen(!listOpen)} data-testid="design-ts-instance">
            <span className="design-dropdown-label">{label}</span>
            <Icon name="Chevron" size={12} className="design-chevron" />
          </button>
          {listOpen ? (
            <div className="design-ts-instances" onPointerLeave={() => writer.endPreview()}>
              {font.instances.map((i) => (
                <button
                  key={i.name}
                  type="button"
                  className="design-ts-instance"
                  onPointerEnter={() => writer.preview({ fontStyle: i.name })}
                  onClick={() => {
                    setListOpen(false)
                    void writer.commit({ fontStyle: i.name, fontVariations: null })
                  }}
                >
                  {i.name === style && !custom ? '✓ ' : '   '}
                  {i.name}
                </button>
              ))}
            </div>
          ) : null}
        </>
      ) : null}
      {font.axes
        .filter((a) => !a.hidden)
        .map((axis) => {
          const implied = instance?.variations[axis.tag] ?? axis.default
          const value = own[axis.tag] ?? implied
          return <AxisRow key={axis.tag} axis={axis} value={variations === null ? null : value} implied={implied} onPreview={(v) => void run('text.preview', { command: 'text.setVariations', params: params({ [axis.tag]: v }) })} onCancel={() => writer.endPreview()} onCommit={(v) => void run('text.setVariations', { ...params({ [axis.tag]: v }) })} />
        })}
      {custom ? (
        <button type="button" className="design-ts-link" style={{ marginTop: 8 }} data-tip={t('Back to the values the font style implies')} onClick={() => void run('text.setVariations', { ids, variations: {}, replace: true })}>
          {t('Reset to style')}
        </button>
      ) : null}
    </div>
  )
}

type Axis = FontInfo['axes'][number]

/** Values snap to integers on wide axes, tenths otherwise (`round_axis`). */
const roundAxis = (axis: Axis, v: number) => (axis.max - axis.min > 20 ? Math.round(v) : Math.round(v * 10) / 10)

function AxisRow({ axis, value, implied, onPreview, onCommit, onCancel }: { axis: Axis; value: number | null; implied: number; onPreview: (v: number) => void; onCommit: (v: number | null) => void; onCancel: () => void }) {
  const { t } = useDesignKit()
  const [live, setLive] = useState<number | null>(null)
  const track = useRef<HTMLDivElement>(null)
  const shown = live ?? value
  const at = (v: number) => ((v - axis.min) / Math.max(1e-9, axis.max - axis.min)) * 100
  const tip = t('{name} ({tag}): {min} – {max}, default {default}. Double-click the slider to go back to the style\'s value; Escape while dragging cancels.', { name: axis.name, tag: axis.tag, min: trimNum(axis.min), max: trimNum(axis.max), default: trimNum(axis.default) })
  return (
    <div className="design-ts-axis">
      <span className="design-ts-axis-label" data-tip={tip}>
        {axis.name}
      </span>
      <div
        ref={track}
        className="design-ts-axis-track"
        data-testid={`design-ts-axis-${axis.tag}`}
        onDoubleClick={() => onCommit(null)}
        onPointerDown={(event) => {
          if (event.button !== 0) return
          event.preventDefault()
          const r = event.currentTarget.getBoundingClientRect()
          let last = shown ?? implied
          let cancelled = false
          const value = (x: number) => roundAxis(axis, axis.min + Math.min(1, Math.max(0, (x - r.left) / r.width)) * (axis.max - axis.min))
          const move = (e: PointerEvent) => {
            if (cancelled) return
            last = value(e.clientX)
            setLive(last)
            onPreview(last)
          }
          const key = (e: KeyboardEvent) => {
            if (e.key !== 'Escape') return
            e.stopPropagation()
            cancelled = true
            setLive(null)
            onCancel()
          }
          const up = () => {
            document.removeEventListener('pointermove', move, true)
            document.removeEventListener('pointerup', up, true)
            window.removeEventListener('keydown', key, true)
            setLive(null)
            if (!cancelled) onCommit(last)
          }
          move(event.nativeEvent)
          document.addEventListener('pointermove', move, true)
          document.addEventListener('pointerup', up, true)
          window.addEventListener('keydown', key, true)
        }}
      >
        <span className="design-ts-axis-fill" style={{ width: `${at(shown ?? implied)}%` }} />
        <span className="design-ts-axis-tick" style={{ left: `${at(axis.default)}%` }} />
        <span className="design-ts-axis-knob" style={{ left: `${at(shown ?? implied)}%` }} />
      </div>
      <NumberField value={shown} mixed={shown === null} width={52} step={axis.max - axis.min > 20 ? 1 : 0.1} onCommit={(e) => e.kind === 'set' && onCommit(Math.min(axis.max, Math.max(axis.min, roundAxis(axis, e.value))))} />
    </div>
  )
}
