/**
 * Typography (2.4, upstream `panels/typography.rs`, `typography/fields.rs`, `spacing.rs`): the
 * header (Text styles), the text style chip, the font family (searchable, previewed), style and
 * size rows, line height and letter spacing, paragraph spacing and resizing, alignment and the
 * Type settings popover, and text on a path. While characters of the edited text are selected,
 * character edits apply to them (`text.style`); otherwise to whole layers (`node.set`). Values
 * that differ show "Mixed".
 */
import { useCallback, useRef, useState } from 'react'
import { useDesignKit, useInspect, useInteraction, useQuery, type Interaction } from '../kit/hooks'
import { Button, IconButton, Menu, NumberField, Row, scrubDeltas, SectionHeader, Segmented, Separator, type MenuEntry, type ScrubHandlers } from '../ui'
import { dispatchUiRequests } from '../kit/requests'
import { type SelectionData, textRange } from './data'
import { FontPicker, rememberFamily } from './FontPicker'
import { StyleChip, openStyles } from './shared'
import { parseLetterSpacing, parseLineHeight, spacingLabel, trimNum } from './spacing'
import { TypeSettings } from './TypeSettings'

const SIZES = [10, 11, 12, 13, 14, 16, 20, 24, 32, 36, 40, 48, 64, 96, 128]

/** The character styles the panel shows: the edited range's or single layer's runs, else each layer's own. */
export function textStyles(data: SelectionData, ids: number[]): Array<Record<string, unknown>> {
  if (data.text && ids.includes(data.text.id) && data.text.runs.length) {
    const r = data.text.range
    const runs = r ? data.text.runs.filter((x) => x.start < r[1] && x.end > r[0]) : data.text.runs
    if (runs.length) return runs.map((x) => x.style)
  }
  return data.nodes.filter((n) => ids.includes(n.id)).map((n) => n as Record<string, unknown>)
}

/** The value every style shares (`uniform`), or null when they differ. */
export function uniform<T>(styles: Array<Record<string, unknown>>, of: (s: Record<string, unknown>) => T): T | null {
  if (!styles.length) return null
  const first = JSON.stringify(of(styles[0]))
  return styles.every((s) => JSON.stringify(of(s)) === first) ? of(styles[0]) : null
}


/** Writes character props to the selected characters, else to whole layers; previews and scrubs. */
export function useTextWriter(ids: number[]) {
  const { run, inspect } = useDesignKit()
  const range = textRange(inspect)
  const commit = useCallback(
    (props: Record<string, unknown>) => {
      if (range && ids.includes(range.id)) return run('text.style', { id: range.id, start: range.start, end: range.end, ...props })
      return run('node.set', { ids, props })
    },
    [ids, range, run],
  )
  const params = useCallback((props: Record<string, unknown>) => (range && ids.includes(range.id) ? { id: range.id, start: range.start, end: range.end, ...props } : { ids, ...props }), [ids, range])
  const preview = useCallback((props: Record<string, unknown>) => void run('text.preview', { command: 'text.style', params: params(props) }), [params, run])
  const endPreview = useCallback(() => void run('text.endPreview', {}), [run])
  return { commit, preview, endPreview, range, params }
}

/** A typography field's scrub: live `node.set` in one interaction ("Font size", …). */
function useTextScrub(ids: number[], label: string, make: (v: number) => Record<string, unknown>): ScrubHandlers {
  const interaction = useInteraction()
  const { t } = useDesignKit()
  const writer = useTextWriter(ids)
  const active = useRef<Interaction | null>(null)
  const latest = useRef({ make, writer })
  latest.current = { make, writer }
  return {
    begin: () => {
      // While text is edited its editing session already groups undo.
      active.current = latest.current.writer.range ? null : interaction(t(label))
    },
    move: (v) => {
      const props = latest.current.make(v)
      if (active.current) active.current.update('node.set', { ids, props })
      else void latest.current.writer.commit(props)
    },
    end: (cancelled) => {
      const i = active.current
      active.current = null
      void (cancelled ? i?.cancel() : i?.commit())
    },
  }
}

export function TypographySection({ data }: { data: SelectionData }) {
  const { t } = useDesignKit()
  const ids = data.nodes.filter((n) => n.type === 'TEXT').map((n) => n.id)
  const styles = textStyles(data, ids)
  const writer = useTextWriter(ids)
  const linked = data.nodes.filter((n) => ids.includes(n.id)).map((n) => (n.styles as Record<string, string> | undefined)?.text ?? null)
  const allLinked = linked.length > 0 && linked.every((s) => s !== null && s === linked[0])
  if (!ids.length) return null
  return (
    <div data-testid="design-typography-section">
      <SectionHeader
        title={t('Typography')}
        testId="design-typography-header"
        actions={[
          {
            icon: 'Variable',
            label: t('Apply variable to text'),
            // The variable picker (the left panel's package) answers `variablePicker.open`.
            onClick: () => {
              const r = document.querySelector('[data-testid="design-text-variable"]')?.getBoundingClientRect()
              dispatchUiRequests([{ kind: 'variablePicker.open', data: { target: { bind: { ids, field: 'characters' } }, kind: 'string', x: r ? r.right - 240 : 200, y: r ? r.bottom + 4 : 200 } }])
            },
            testId: 'design-text-variable',
          },
          { icon: 'Styles', label: t('Text styles'), onClick: () => openStyles(ids, 'text', 'design-text-styles'), testId: 'design-text-styles' },
        ]}
      />
      <StyleChip ids={ids} slot="text" nodes={data.nodes.filter((n) => ids.includes(n.id))} testId="design-text-style" />
      {!allLinked ? <FontRows ids={ids} styles={styles} writer={writer} /> : null}
      <ParagraphRows data={data} ids={ids} styles={styles} writer={writer} />
      <PathRow data={data} ids={ids} />
      <Separator />
    </div>
  )
}

function FontRows({ ids, styles, writer }: { ids: number[]; styles: Array<Record<string, unknown>>; writer: ReturnType<typeof useTextWriter> }) {
  const { t } = useDesignKit()
  const family = uniform(styles, (s) => String(s.fontFamily ?? ''))
  const style = uniform(styles, (s) => String(s.fontStyle ?? ''))
  const size = uniform(styles, (s) => Number(s.fontSize ?? 0))
  const lineHeight = uniform(styles, (s) => spacingLabel(s.lineHeight))
  const letterSpacing = uniform(styles, (s) => spacingLabel(s.letterSpacing))
  const [picker, setPicker] = useState<Element | null>(null)
  const [styleMenu, setStyleMenu] = useState<Element | null>(null)
  const [sizeMenu, setSizeMenu] = useState<Element | null>(null)
  const fontInfo = useQuery<{ styles: string[]; found: boolean }>('font', { family: family ?? '' }, { skip: !family || !styleMenu })
  const sizeScrub = useTextScrub(ids, 'Font size', (v) => ({ fontSize: Math.min(5000, Math.max(1, v)) }))
  const lineHeightScrub = useTextScrub(ids, 'Line height', (v) => ({ lineHeight: lineHeight?.endsWith('%') ? `${trimNum(Math.max(0, v))}%` : Math.max(0, v) }))
  const letterSpacingScrub = useTextScrub(ids, 'Letter spacing', (v) => ({ letterSpacing: letterSpacing === null || letterSpacing.endsWith('%') ? `${trimNum(v)}%` : v }))
  const previewed = useRef(false)
  const preview = (props: Record<string, unknown>) => {
    previewed.current = true
    writer.preview(props)
  }
  const endPreview = () => {
    if (previewed.current) writer.endPreview()
    previewed.current = false
  }

  /** "Apply variable" atop the family and style menus (not for a selected range): binds the field to a string variable. */
  const applyVariable = (field: 'fontFamily' | 'fontStyle', anchor: Element | null): MenuEntry[] =>
    writer.range
      ? []
      : [
          {
            label: t('Apply variable'),
            onSelect: () => {
              const r = anchor?.getBoundingClientRect()
              dispatchUiRequests([{ kind: 'variablePicker.open', data: { target: { bind: { ids, field } }, kind: 'string', x: r?.left ?? 200, y: r?.bottom ?? 200 } }])
            },
            testId: `design-${field}-variable`,
          },
          { type: 'separator' },
        ]
  const styleList: MenuEntry[] =
    family === null
      ? [{ type: 'custom', render: () => <div className="design-paint-menu-note">{t('Pick one font family first')}</div> }]
      : fontInfo && fontInfo.styles.length === 0
        ? [{ type: 'custom', render: () => <div className="design-paint-menu-note">{t('Font not installed')}</div> }]
        : (fontInfo?.styles ?? []).map((s) => ({
            label: s,
            checked: s === style,
            onHover: () => preview({ fontStyle: s }),
            onSelect: () => {
              previewed.current = false
              void writer.commit({ fontStyle: s })
            },
            testId: `design-font-style-${s}`,
          }))
  const styleItems: MenuEntry[] = [...applyVariable('fontStyle', styleMenu), ...styleList]

  return (
    <>
      <Row>
        <button type="button" className="design-dropdown" style={{ width: '100%' }} aria-haspopup="listbox" aria-expanded={Boolean(picker)} data-testid="design-font-family" onClick={(event) => setPicker(picker ? null : event.currentTarget)}>
          <span className="design-dropdown-label" data-mixed={family === null || undefined}>
            {family ?? t('Mixed')}
          </span>
          <ChevronIcon />
        </button>
      </Row>
      {picker ? (
        <FontPicker
          anchor={picker}
          current={family}
          onPreview={(f) => preview({ fontFamily: f })}
          onApplyVariable={
            writer.range
              ? undefined
              : () => {
                  const item = applyVariable('fontFamily', picker)[0]
                  setPicker(null)
                  if (item && 'onSelect' in item) item.onSelect()
                }
          }
          onPick={(f) => {
            previewed.current = false
            rememberFamily(f)
            void writer.commit({ fontFamily: f })
            setPicker(null)
          }}
          onClose={() => {
            endPreview()
            setPicker(null)
          }}
        />
      ) : null}
      <Row>
        <button type="button" className="design-dropdown" style={{ width: 'calc((100% - 8px) / 2)' }} aria-haspopup="menu" aria-expanded={Boolean(styleMenu)} data-testid="design-font-style" onClick={(event) => setStyleMenu(styleMenu ? null : event.currentTarget)}>
          <span className="design-dropdown-label" data-mixed={style === null || undefined}>
            {style ?? t('Mixed')}
          </span>
          <ChevronIcon />
        </button>
        <span className="design-font-size">
          <NumberField
            value={size}
            mixed={size === null}
            min={1}
            max={5000}
            width="100%"
            onCommit={(edit) => void writer.commit({ fontSize: Math.min(5000, Math.max(1, edit.kind === 'set' ? edit.value : edit.apply(size ?? 0))) })}
            scrub={sizeScrub}
            ariaLabel={t('Font size')}
            testId="design-font-size"
          />
          <IconButton icon="Chevron" label={t("Font size")} size={20} iconSize={12} onClick={() => setSizeMenu(document.querySelector('[data-testid="design-font-size-menu-button"]'))} testId="design-font-size-menu-button" />
        </span>
      </Row>
      {styleMenu ? (
        <Menu
          tone="light"
          items={styleItems}
          anchor={styleMenu}
          minWidth={200}
          maxHeight={320}
          onClose={() => {
            endPreview()
            setStyleMenu(null)
          }}
          testId="design-font-style-menu"
        />
      ) : null}
      {sizeMenu ? (
        <Menu
          tone="light"
          items={SIZES.map((s) => ({ label: String(s), checked: s === size, onSelect: () => void writer.commit({ fontSize: s }), testId: `design-font-size-${s}` }))}
          anchor={sizeMenu}
          placement="below-end"
          minWidth={90}
          maxHeight={360}
          onClose={() => setSizeMenu(null)}
          testId="design-font-size-menu"
        />
      ) : null}
      <Row>
        <SpacingField
          glyph="lineHeight"
          tip={t('Line height')}
          value={lineHeight}
          onCommit={(text) => {
            const v = parseLineHeight(text, lineHeight)
            if (v !== null) void writer.commit({ lineHeight: v })
          }}
          scrub={lineHeightScrub}
          testId="design-line-height"
        />
        <SpacingField
          glyph="letterSpacing"
          tip={t('Letter spacing')}
          value={letterSpacing}
          onCommit={(text) => {
            const v = parseLetterSpacing(text, letterSpacing)
            if (v !== null) void writer.commit({ letterSpacing: v })
          }}
          scrub={letterSpacingScrub}
          testId="design-letter-spacing"
        />
      </Row>
    </>
  )
}

function ChevronIcon() {
  return (
    <svg className="design-chevron" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  )
}

/** The lead glyphs of the spacing fields (original drawings, upstream `paint_lead`). */
function Glyph({ kind }: { kind: 'lineHeight' | 'letterSpacing' | 'paragraphSpacing' }) {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" aria-hidden="true">
      {kind === 'lineHeight' ? (
        <>
          <line x1="7" y1="6.5" x2="17" y2="6.5" stroke="currentColor" />
          <line x1="7" y1="17.5" x2="17" y2="17.5" stroke="currentColor" />
          <text x="12" y="15.3" textAnchor="middle" fontSize="9.5" fill="currentColor" fontFamily="var(--design-font)">
            A
          </text>
        </>
      ) : kind === 'letterSpacing' ? (
        <>
          <line x1="6.5" y1="7" x2="6.5" y2="17" stroke="currentColor" />
          <line x1="17.5" y1="7" x2="17.5" y2="17" stroke="currentColor" />
          <text x="12" y="15.3" textAnchor="middle" fontSize="9.5" fill="currentColor" fontFamily="var(--design-font)">
            A
          </text>
        </>
      ) : (
        <>
          {[-5, -2.5, 2.5, 5].map((dy) => (
            <line key={dy} x1="7" y1={12 + dy + 0.5} x2="17" y2={12 + dy + 0.5} stroke="currentColor" />
          ))}
        </>
      )}
    </svg>
  )
}

/** A spacing field (`spacing.rs`): a glyph lead that scrubs (not Auto or Mixed), text that takes `Auto`, `24`, `150%`. */
function SpacingField({ glyph, tip, value, onCommit, scrub, testId }: { glyph: 'lineHeight' | 'letterSpacing' | 'paragraphSpacing'; tip: string; value: string | null; onCommit: (text: string) => void; scrub?: ScrubHandlers; testId: string }) {
  const { t } = useDesignKit()
  const [text, setText] = useState<string | null>(null)
  const cancelled = useRef(false)
  const numeric = value !== null ? Number.parseFloat(value) : NaN
  const shown = value === null ? t('Mixed') : value === 'Auto' ? t('Auto') : value
  const lead = (event: React.PointerEvent<HTMLSpanElement>) => {
    if (event.button !== 0 || !scrub || !Number.isFinite(numeric)) return
    event.preventDefault()
    let total = 0
    let began = false
    let last = numeric
    const deltaOf = scrubDeltas(event.clientX)
    const move = (e: PointerEvent) => {
      const dx = deltaOf(e)
      total += dx * (e.shiftKey ? 10 : e.altKey ? 0.1 : 1)
      if (!began) {
        if (Math.abs(total) < 2) return
        began = true
        scrub.begin()
      }
      const next = Math.round((numeric + total) * (e.altKey ? 10 : 1)) / (e.altKey ? 10 : 1)
      if (next !== last) {
        last = next
        scrub.move(next, next - numeric)
      }
    }
    const up = () => {
      document.removeEventListener('pointermove', move, true)
      document.removeEventListener('pointerup', up, true)
      if (began) scrub.end(false)
    }
    document.addEventListener('pointermove', move, true)
    document.addEventListener('pointerup', up, true)
  }
  return (
    <div className="design-field design-spacing" style={{ width: 'calc((100% - 8px) / 2)' }} data-tip={tip} data-testid={testId} data-editing={text !== null || undefined}>
      <span className="design-field-lead" data-icon="true" data-scrub={Boolean(scrub && Number.isFinite(numeric))} onPointerDown={lead}>
        <Glyph kind={glyph} />
      </span>
      {text === null ? (
        <button type="button" className="design-spacing-value" data-mixed={value === null || undefined} aria-label={tip} onClick={() => setText(value === null ? '' : shown)}>
          {shown}
        </button>
      ) : (
        <input
          autoFocus
          spellCheck={false}
          aria-label={tip}
          value={text}
          onFocus={(event) => event.currentTarget.select()}
          onChange={(event) => setText(event.target.value)}
          onBlur={() => {
            if (!cancelled.current && text !== shown) onCommit(text)
            cancelled.current = false
            setText(null)
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') event.currentTarget.blur()
            else if (event.key === 'Escape') {
              cancelled.current = true
              event.currentTarget.blur()
            } else return
            event.stopPropagation()
          }}
        />
      )}
    </div>
  )
}

function ParagraphRows({ data, ids, styles, writer }: { data: SelectionData; ids: number[]; styles: Array<Record<string, unknown>>; writer: ReturnType<typeof useTextWriter> }) {
  const { t, run } = useDesignKit()
  const nodes = data.nodes.filter((n) => ids.includes(n.id))
  const resize = uniform(nodes, (n) => String(n.textAutoResize ?? 'none'))
  const align = uniform(nodes, (n) => String(n.textAlign ?? 'left'))
  // The engine names the middle `middle` (older answers: `center`).
  const alignV = uniform(nodes, (n) => (n.textAlignVertical === 'center' ? 'middle' : String(n.textAlignVertical ?? 'top')))
  const ps = uniform(nodes, (n) => Number(n.paragraphSpacing ?? 0))
  const [settings, setSettings] = useState<Element | null>(null)
  const set = (props: Record<string, unknown>) => void run('node.set', { ids, props })
  const psScrub = useTextScrub(ids, 'Paragraph spacing', (v) => ({ paragraphSpacing: Math.min(10000, Math.max(0, v)) }))
  return (
    <>
      <Row>
        <SpacingField
          glyph="paragraphSpacing"
          tip={t('Paragraph spacing')}
          value={ps === null ? null : trimNum(ps)}
          onCommit={(text) => {
            const v = Number.parseFloat(text)
            if (Number.isFinite(v)) set({ paragraphSpacing: Math.min(10000, Math.max(0, v)) })
          }}
          scrub={psScrub}
          testId="design-paragraph-spacing"
        />
        <Segmented
          value={resize}
          cell={25}
          segments={[
            { value: 'widthAndHeight', label: t('Auto width'), icon: 'Hug' },
            { value: 'height', label: t('Auto height'), icon: 'LayoutV' },
            { value: 'none', label: t('Fixed size'), icon: 'Fixed' },
            { value: 'truncate', label: t('Truncate text'), icon: 'Dots' },
          ]}
          onChange={(textAutoResize) => set({ textAutoResize })}
          testId="design-text-resize"
        />
      </Row>
      <Row>
        <Segmented
          value={align}
          segments={[
            { value: 'left', label: t('Align left'), icon: 'TextAlignLeft' },
            { value: 'center', label: t('Align center'), icon: 'TextAlignCenter' },
            { value: 'right', label: t('Align right'), icon: 'TextAlignRight' },
            { value: 'justified', label: t('Justify'), icon: 'TextAlignJustify' },
          ]}
          onChange={(textAlign) => set({ textAlign })}
          testId="design-text-align"
        />
        <Segmented
          value={alignV}
          segments={[
            { value: 'top', label: t('Align top'), icon: 'TextTop' },
            { value: 'middle', label: t('Align middle'), icon: 'TextMiddle' },
            { value: 'bottom', label: t('Align bottom'), icon: 'TextBottom' },
          ]}
          onChange={(textAlignVertical) => set({ textAlignVertical })}
          testId="design-text-align-v"
        />
        <IconButton iconSize={16} icon="Dots" label={t('Type settings')} pressed={Boolean(settings)} onClick={() => setSettings(settings ? null : document.querySelector('[data-testid="design-type-settings-button"]'))} testId="design-type-settings-button" />
      </Row>
      {settings ? <TypeSettings anchor={settings} data={data} ids={ids} styles={styles} writer={writer} onClose={() => setSettings(null)} /> : null}
    </>
  )
}

function PathRow({ data, ids }: { data: SelectionData; ids: number[] }) {
  const { t, run } = useDesignKit()
  const nodes = data.nodes.filter((n) => ids.includes(n.id))
  const paths = nodes.map((n) => n.textPath as { start?: number; startOffset?: number; flip?: boolean } | undefined)
  if (paths.some((p) => !p)) {
    const other = data.nodes.find((n) => !ids.includes(n.id))
    if (ids.length === 1 && data.ids.length === 2 && other && ['VECTOR', 'LINE', 'ELLIPSE', 'RECTANGLE', 'POLYGON', 'STAR', 'BOOLEAN_OPERATION'].includes(other.type)) {
      return (
        <Row>
          <Button onClick={() => void run('text.onPath', { ids: data.ids })} testId="design-text-on-path">
            {t('Put text on path')}
          </Button>
        </Row>
      )
    }
    return null
  }
  const starts = paths.map((p) => Number(p?.start ?? p?.startOffset ?? 0))
  const start = starts.every((s) => s === starts[0]) ? starts[0] : null
  const flip = paths.every((p) => p?.flip === true)
  return (
    <Row>
      <NumberField value={start === null ? null : Math.round(start * 100)} mixed={start === null} icon="Pen" tip={t('Start on path')} suffix="%" width={96} onCommit={(e) => e.kind === 'set' && void run('text.pathOptions', { ids, start: e.value / 100 })} />
      <IconButton iconSize={16} icon="FlipV" label={t('Flip text to the other side')} pressed={flip} onClick={() => void run('text.pathOptions', { ids, flip: !flip })} />
      <IconButton iconSize={16} icon="Detach" label={t('Remove text from path')} onClick={() => void run('text.removeFromPath', { ids })} />
    </Row>
  )
}
