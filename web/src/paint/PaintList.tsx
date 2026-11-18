/**
 * Fill and Stroke (2.4, upstream `panels/right/paints.rs` `paints`, `paint_row`, `range_fill`): the
 * section header (Styles, Add), the linked style's chip, one row per paint (the top paint first:
 * swatch, hex or kind or bound variable, opacity, visibility, remove; right-click copies and
 * pastes; rows drag to reorder), "Click + to replace mixed content" when the layers differ, and the
 * stroke's settings. While text is edited with characters selected, Fill edits their colour.
 */
import { useRef, useState } from 'react'
import { useDesignKit, useInspect, useInteraction, type Interaction } from '../kit/hooks'
import { IconButton, Note, NumberField, Row, SectionHeader, Separator, type ScrubHandlers } from '../ui'
import { fromHexOrName, toCssHex, toHex } from './color'
import { type SelectionData, common, MIXED, textRange } from './data'
import { movedIndex, type Paint, paintLabel, readPaints, removedIndex } from './paint'
import { BoundChip, StyleChip, clipboardState, useContextMenu, useListDrag, openStyles, useVariables } from './shared'
import { StrokeSettings } from './StrokeSettings'
import { closePicker, openPicker, paintState, updatePicker, usePaintState } from './store'
import { PaintSwatch } from './Swatch'

/** An inline text that turns into an editor on click (`text_field_inline`): Enter or blur commits, Escape cancels. */
export function InlineText({ value, onCommit, ariaLabel, testId }: { value: string; onCommit: (text: string) => void; ariaLabel: string; testId?: string }) {
  const [editing, setEditing] = useState<string | null>(null)
  const cancelled = useRef(false)
  if (editing === null) {
    return (
      <button type="button" className="design-inline-text" aria-label={ariaLabel} data-testid={testId} onClick={() => setEditing(value)}>
        {value}
      </button>
    )
  }
  return (
    <input
      className="design-inline-text"
      data-editing="true"
      autoFocus
      spellCheck={false}
      aria-label={ariaLabel}
      data-testid={testId}
      value={editing}
      onFocus={(event) => event.currentTarget.select()}
      onChange={(event) => setEditing(event.target.value)}
      onBlur={() => {
        if (!cancelled.current && editing.trim() && editing !== value) onCommit(editing)
        cancelled.current = false
        setEditing(null)
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
  )
}

/** An opacity field's scrub as one undo step (`Opacity`). */
export function useScrub(label: string, write: (value: number, i: Interaction) => void): ScrubHandlers {
  const interaction = useInteraction()
  const { t } = useDesignKit()
  const active = useRef<Interaction | null>(null)
  const latest = useRef(write)
  latest.current = write
  return {
    begin: () => {
      active.current = interaction(t(label))
    },
    move: (value) => {
      if (active.current) latest.current(value, active.current)
    },
    end: (cancelled) => {
      const i = active.current
      active.current = null
      void (cancelled ? i?.cancel() : i?.commit())
    },
  }
}

export function PaintSection({ data, fills }: { data: SelectionData; fills: boolean }) {
  const designKit = useDesignKit()
  const { t, run } = designKit
  const inspect = useInspect()
  const ids = data.ids
  const key = fills ? 'fills' : 'strokes'
  const kind = fills ? 'fill' : 'stroke'
  const range = fills ? textRange(inspect) : null
  const value = common<unknown[]>(data, key)
  const mixed = value === MIXED
  const list = mixed ? [] : readPaints(value)
  const empty = mixed || list.length === 0
  const testId = `design-${kind}`
  const header = useContextMenu(() => (clipboardState()[kind] ? [{ label: t(fills ? 'Paste fill' : 'Paste stroke'), onSelect: () => void run(`${kind}.paste`, { ids }) }] : []), `${testId}-header-menu`)
  const drag = useListDrag(list.length, (from, to) => {
    void run(`${kind}.move`, { ids, from, to })
    const p = paintState().picker
    if (p?.kind === 'paint' && p.fills === fills) updatePicker({ ...p, index: movedIndex(p.index, from, to) })
  })

  if (range && ids.includes(range.id)) return <RangeFill data={data} range={range} />

  const add = async () => {
    const answer = await run(`${kind}.add`, { ids })
    // The new paint goes on top; its picker opens (a mixed selection has no one index).
    if (answer !== null && !mixed) openPicker({ kind: 'paint', fills, index: list.length, ids })
  }

  return (
    <div data-testid={`${testId}-section`}>
      <SectionHeader
        title={t(fills ? 'Fill' : 'Stroke')}
        dim={empty}
        dimAction={1}
        onContextMenu={header.open}
        testId={`${testId}-header`}
        actions={[
          { icon: 'Styles', label: t('Styles'), onClick: () => openStyles(ids, kind, `${testId}-styles`), testId: `${testId}-styles` },
          { icon: 'Plus', label: t(fills ? 'Add fill' : 'Add stroke'), onClick: () => void add(), testId: `${testId}-add` },
        ]}
      />
      {header.menu}
      <StyleChip ids={ids} slot={kind} nodes={data.nodes} testId={`${testId}-style`} />
      {mixed ? <Note>{t('Click + to replace mixed content')}</Note> : null}
      {list
        .map((paint, index) => ({ paint, index }))
        .reverse()
        .map(({ paint, index }) => (
          <PaintRow key={index} data={data} fills={fills} index={index} paint={paint} count={list.length} drag={drag} />
        ))}
      {!fills && !empty ? <StrokeSettings data={data} /> : null}
      <Separator />
    </div>
  )
}

function PaintRow({ data, fills, index, paint, drag }: { data: SelectionData; fills: boolean; index: number; paint: Paint; count: number; drag: ReturnType<typeof useListDrag> }) {
  const { t, run } = useDesignKit()
  const state = usePaintState()
  const variables = useVariables()
  const ids = data.ids
  const kind = fills ? 'fill' : 'stroke'
  const cmd = `${kind}.set`
  const testId = `design-${kind}-${index}`
  const opacity = useScrub('Opacity', (v, i) => i.update(cmd, { ids, index, opacity: Math.min(1, Math.max(0, v / 100)) }))
  // The variable bound to this paint's colour (`fill` or `fills/<i>/color`) on the first layer.
  const bound = (data.nodes[0]?.boundVariables ?? {}) as Record<string, string>
  const bindingKey = Object.keys(bound).find((k) => {
    if (fills && index === 0 && (k === 'fill' || k === 'fills' || k === 'fills/0')) return true
    if (!fills && index === 0 && k === 'stroke') return true
    return k === `${fills ? 'fills' : 'strokes'}/${index}/color`
  })
  const variable = bindingKey ? variables.find((v) => v.id === bound[bindingKey]) : undefined
  const open = state.picker?.kind === 'paint' && state.picker.fills === fills && state.picker.index === index
  const menu = useContextMenu(() => [
    {
      label: t(fills ? 'Copy fill' : 'Copy stroke'),
      onSelect: () => void run(`${kind}.copy`, { ids, index }),
    },
    ...(clipboardState()[kind] ? [{ label: t(fills ? 'Paste fill' : 'Paste stroke'), onSelect: () => void run(`${kind}.paste`, { ids, index }) }] : []),
  ], `${testId}-menu`)
  return (
    <div className="design-paint-row" {...drag.row(index)} onContextMenu={menu.open} data-testid={testId}>
      {drag.grip(index, paintLabel(paint))}
      <Row>
        <div className="design-paint-field">
          <button
            type="button"
            className="design-paint-swatch"
            aria-label={t('Color')}
            aria-expanded={open}
            data-testid={`${testId}-swatch`}
            onClick={() => (open ? closePicker() : openPicker({ kind: 'paint', fills, index, ids }))}
          >
            <PaintSwatch paint={paint} size={16} />
          </button>
          <span className="design-paint-text">
            {paint.type === 'solid' && variable && bindingKey ? (
              <BoundChip name={variable.name} onDetach={() => void run('variable.bind', { ids, field: bindingKey, variable: null })} testId={`${testId}-variable`} />
            ) : paint.type === 'solid' ? (
              <InlineText
                value={toHex(paint.color)}
                ariaLabel={t('Name')}
                testId={`${testId}-hex`}
                onCommit={(text) => {
                  const c = fromHexOrName(text)
                  if (c) void run(cmd, { ids, index, color: toCssHex(c) })
                }}
              />
            ) : (
              <span className="design-paint-label">{t(paintLabel(paint))}</span>
            )}
          </span>
          <span className="design-paint-divider" />
          <span className="design-paint-opacity">
            <NumberField
              value={Math.round(paint.opacity * 100)}
              suffix="%"
              width={48}
              min={0}
              max={100}
              onCommit={(edit) => {
                const v = edit.kind === 'set' ? edit.value : edit.apply(paint.opacity * 100)
                void run(cmd, { ids, index, opacity: Math.min(1, Math.max(0, v / 100)) })
              }}
              scrub={opacity}
              ariaLabel={t('Opacity')}
              testId={`${testId}-opacity`}
            />
          </span>
        </div>
        <IconButton iconSize={16} icon={paint.visible ? 'Eye' : 'EyeOff'} label={t('Toggle visibility')} onClick={() => void run(cmd, { ids, index, visible: !paint.visible })} testId={`${testId}-visible`} />
        <IconButton
          icon="Minus"
          label={t('Remove')}
          testId={`${testId}-remove`}
          onClick={async () => {
            if ((await run(`${kind}.remove`, { ids, index })) === null) return
            // An open picker keeps editing its paint, or closes with it.
            const p = paintState().picker
            if (p?.kind === 'paint' && p.fills === fills) {
              const k = removedIndex(p.index, index)
              if (k === null) closePicker()
              else updatePicker({ ...p, index: k })
            }
          }}
        />
      </Row>
      {menu.menu}
    </div>
  )
}

/** Fill while characters of the edited text are selected: their colour, or the layer's. */
function RangeFill({ data, range }: { data: SelectionData; range: { id: number; start: number; end: number } }) {
  const { t, run } = useDesignKit()
  const node = data.nodes.find((n) => n.id === range.id)
  const runs = data.text?.id === range.id ? data.text.runs.filter((r) => r.start < range.end && r.end > range.start) : []
  const fills = runs.map((r) => JSON.stringify(r.style.fills ?? node?.fills ?? []))
  const same = fills.length > 0 && fills.every((f) => f === fills[0])
  const paint = same ? readPaints(JSON.parse(fills[0]))[0] : undefined
  const open = () => openPicker({ kind: 'textRange', id: range.id, start: range.start, end: range.end })
  return (
    <div data-testid="design-fill-section">
      <SectionHeader title={t('Fill')} testId="design-fill-header" />
      <Row>
        <div className="design-paint-field" style={{ width: 'calc(100% - 28px)' }}>
          {paint ? (
            <>
              <button type="button" className="design-paint-swatch" aria-label={t('Color')} onClick={open} data-testid="design-range-swatch">
                <PaintSwatch paint={paint} size={16} />
              </button>
              <span className="design-paint-text">
                {paint.type === 'solid' ? (
                  <InlineText
                    value={toHex(paint.color)}
                    ariaLabel={t('Name')}
                    testId="design-range-hex"
                    onCommit={(text) => {
                      const c = fromHexOrName(text)
                      if (c) void run('text.style', { id: range.id, start: range.start, end: range.end, textFills: [toCssHex(c)] })
                    }}
                  />
                ) : (
                  <span className="design-paint-label">{t('Gradient')}</span>
                )}
              </span>
            </>
          ) : (
            <button type="button" className="design-paint-mixed" onClick={open}>
              {t('Mixed')}
            </button>
          )}
        </div>
        <IconButton iconSize={16} icon="Detach" label={t("Use the layer's fill")} onClick={() => void run('text.style', { id: range.id, start: range.start, end: range.end, textFills: null })} />
      </Row>
      <Separator />
    </div>
  )
}
