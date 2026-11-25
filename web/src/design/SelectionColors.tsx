/**
 * Selection colors (2.3, upstream `panels/selection_colors.rs`): shown when the selection's layers
 * and their descendants use two or more colours. Each row: the swatch (opens the colour picker on
 * the group's paints), the hex (or the linked style's or variable's name, or the paint's kind),
 * the opacity, detach or apply a style or variable, and select the layers using it. Rows are
 * frozen while an interaction runs.
 */
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { dispatchUiRequests } from '../kit/requests'
import { useDesignKit, useInspect, useQuery } from '../kit/hooks'
import { IconButton, SectionHeader, Separator, TextField, type MenuEntry } from '../ui'
import { useDesign } from './hooks'
import { ActionField, Line, Space, menuHeading, useMenu, useVariables } from './fields'
import { hexOf, parseHex, type Rgba } from './model'
import { PaintSwatch } from '../paint/Swatch'
import { readPaint } from '../paint/paint'

type Paint = { type: string; color?: Rgba; opacity?: number; [key: string]: unknown }
type Group = { paint: Paint; style: string | null; variable: string | null; targets: unknown[] }
type Style = { id: string; name: string; type: string }

const paintName = (paint: Paint) => (paint.type === 'image' ? 'Image' : paint.type === 'gradient' ? 'Gradient' : paint.type === 'pattern' ? 'Pattern' : 'Solid')

/**
 * Asks the colour picker (the paints package) to open for `targets`: UI request `picker.open`
 * with `{ target: 'selectionColor', targets, selection, paint, anchor }`.
 */
function openPicker(targets: unknown[], paint: Paint, anchor: Element | null, selection: readonly number[]) {
  const rect = anchor?.getBoundingClientRect()
  dispatchUiRequests([{ kind: 'picker.open', data: { target: 'selectionColor', targets, selection: [...selection], paint, anchor: rect ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null } }])
}

export function SelectionColorsSection() {
  const { t } = useDesignKit()
  const { ids } = useDesign()
  const inspect = useInspect()
  const answer = useQuery<{ colors: Group[] }>('selectionColors', { ids })
  const frozen = useRef<Group[]>([])
  const interacting = Boolean((inspect as { interaction?: string | null }).interaction)
  if (!interacting && answer) frozen.current = answer.colors
  const groups = interacting ? frozen.current : (answer?.colors ?? frozen.current)
  if (groups.length < 2) return null
  return (
    <div data-testid="design-design-selection-colors">
      <SectionHeader title={t('Selection colors')} />
      <ColorList rows={groups.length} disabled={interacting}>
        {groups.map((group, i) => (
          <ColorRow key={`${i}-${JSON.stringify(group.targets[0])}`} group={group} index={i} />
        ))}
      </ColorList>
      <Space />
      <Separator />
    </div>
  )
}

/**
 * The rows' scroll area, sized as upstream's nested egui `ScrollArea` (`max_height(240)`): its
 * height is the room left in the panel's viewport below the list's place in the panel's content
 * (not the scrolled position), at least egui's `min_scrolled_height` 64 and at most 240, and no
 * taller than its rows. So a long panel shows two rows. egui's scroll fades (20 px, half the panel
 * colour at the edge) mark rows above and below; upstream's rows scroll 20 px past the last one
 * (measured: two rows in a 64 px area already fade at full strength), hence the bottom padding.
 */
function ColorList({ rows, disabled, children }: { rows: number; disabled: boolean; children: ReactNode }) {
  const list = useRef<HTMLDivElement>(null)
  const [height, setHeight] = useState(64)
  const [fade, setFade] = useState({ top: 0, bottom: 0 })
  const content = rows * 32
  const overflow = content > height
  useLayoutEffect(() => {
    const el = list.current
    const scroller = el?.closest('.design-panel-scroll')
    if (!el || !scroller) return
    const measure = () => {
      const top = el.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop
      setHeight(Math.min(content, Math.min(240, Math.max(64, scroller.clientHeight - top))))
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(scroller)
    return () => observer.disconnect()
  }, [content])
  const onScroll = () => {
    const el = list.current
    if (!el) return
    const below = el.scrollHeight - el.clientHeight - el.scrollTop
    setFade({ top: Math.min(1, el.scrollTop / 20) * 0.5, bottom: Math.min(1, below / 20) * 0.5 })
  }
  useLayoutEffect(onScroll, [height, content])
  return (
    <div className="design-d-colors-frame" style={{ ['--fade-top' as string]: fade.top, ['--fade-bottom' as string]: fade.bottom }}>
      <div ref={list} className="design-d-colors" style={{ height, paddingBottom: overflow ? 20 : 0 }} data-disabled={disabled || undefined} onScroll={onScroll} data-testid="design-design-colors-list">
        {children}
      </div>
    </div>
  )
}

function ColorRow({ group, index }: { group: Group; index: number }) {
  const { t, run, toast } = useDesignKit()
  const { ids } = useDesign()
  const variables = useVariables()
  const styles = useQuery<Style[]>('styles')
  const swatch = useRef<HTMLButtonElement>(null)
  const stylesMenu = useMenu()
  const { targets, paint } = group
  const linked = group.style ? (styles?.find((s) => s.id === group.style)?.name ?? group.style) : group.variable ? (variables.find((v) => v.id === group.variable)?.name ?? group.variable) : null
  const width = 'max(40px, calc(100% - 148px))'
  const setPaint = (next: Paint) => void run('select.setPaint', { targets, paint: next })
  const colorStyles = (styles ?? []).filter((s) => /paint|fill|color/i.test(s.type))
  const colorVariables = variables.filter((v) => v.resolvedType === 'color')
  const items: MenuEntry[] = [
    ...(colorStyles.length
      ? colorStyles.map((s) => ({ label: s.name, onSelect: () => void run('select.applyColorStyle', { targets, style: s.id }) }))
      : [{ type: 'custom' as const, render: () => <div className="design-d-menu-note">{t('No color styles')}</div> }]),
    ...(colorVariables.length ? [{ type: 'separator' as const }, menuHeading(t('Color variables')), ...colorVariables.map((v) => ({ label: v.name, onSelect: () => void run('select.bindColorVariable', { targets, variable: v.id }) }))] : []),
  ]
  return (
    <Line testId={`design-design-color-${index}`}>
      <button ref={swatch} type="button" className="design-d-swatch-button" data-tip={t('Edit colour')} aria-label={t('Edit colour')} onClick={() => openPicker(targets, paint, swatch.current, ids)}>
        <PaintSwatch paint={readPaint(paint) ?? undefined} size={24} />
      </button>
      <span className="design-d-color-value" style={{ width }}>
        {linked ? (
          <ActionField label={linked} width="100%" onClick={() => openPicker(targets, paint, swatch.current, ids)} />
        ) : paint.type === 'solid' && paint.color ? (
          <TextField
            value={hexOf(paint.color)}
            width="100%"
            ariaLabel={t('Color')}
            testId={`design-design-color-${index}-hex`}
            onCommit={(text) => {
              const c = parseHex(text)
              if (!c) return toast(t('Invalid color'))
              setPaint({ ...paint, color: { ...c, a: 1 }, ...(c.a < 1 ? { opacity: c.a } : {}) })
            }}
          />
        ) : (
          <ActionField label={t(paintName(paint))} width="100%" onClick={() => openPicker(targets, paint, swatch.current, ids)} />
        )}
      </span>
      <TextField
        value={`${Math.round((paint.opacity ?? 1) * 10000) / 100}%`}
        width={44}
        ariaLabel={t('Opacity')}
        testId={`design-design-color-${index}-opacity`}
        onCommit={(text) => {
          const v = Number(text.trim().replace(/%$/, ''))
          if (Number.isFinite(v)) setPaint({ ...paint, opacity: Math.min(1, Math.max(0, v / 100)) })
        }}
      />
      {linked ? (
        <IconButton iconSize={16} icon="Detach" label={group.style ? t('Detach style') : t('Detach variable')} onClick={() => void run('select.detachColor', { targets })} />
      ) : (
        <span ref={stylesMenu.ref} className="design-d-anchor">
          <IconButton iconSize={16} icon="Styles" label={t('Color styles')} onClick={stylesMenu.toggle} testId={`design-design-color-${index}-styles`} />
        </span>
      )}
      {stylesMenu.show(items, 200)}
      <IconButton iconSize={16} icon="Move" label={t('Select layers with this color')} onClick={() => void run('select.selectColor', { targets })} testId={`design-design-color-${index}-select`} />
    </Line>
  )
}
