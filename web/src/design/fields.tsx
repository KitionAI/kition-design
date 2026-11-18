/**
 * The Design panel's field building blocks: sublabels on the panel's two columns, bindable number
 * fields (`variable_ui::number_field*`: the variable button while hovered, the bound chip with its
 * detach button), the add-limit slot, and the full-width "wide" row.
 */
import { useRef, useState, type CSSProperties, type ReactNode } from 'react'
import { useDesignKit, useQuery } from '../kit/hooks'
import { Icon, type IconName } from '../icons/Icon'
import { dispatchUiRequests } from '../kit/requests'
import { Menu, NumberField, type MenuEntry, type NumberEdit, type NumberFieldProps, type ScrubHandlers } from '../ui'
import { common, type NodeJson } from './model'

/** The width of one field column (`col_width`), from the panel's width. */
export const COL = 'var(--design-col)'
/** Both columns and the gap between them (`col * 2 + 8`). */
export const COL2 = 'calc(var(--design-col) * 2 + 8px)'

/** Sublabels over the two columns (`sublabels`). */
export function Sub({ a, b }: { a: string; b?: string | null }) {
  return (
    <div className="design-sublabels">
      <span style={b ? { width: COL, flex: 'none' } : undefined} data-tip={a.length > 14 ? a : undefined}>
        {a}
      </span>
      {b ? <span data-tip={b.length > 14 ? b : undefined}>{b}</span> : null}
    </div>
  )
}

/** A panel row (`row(h)`): content inset 16, items 8 apart, centred. */
export function Line({ children, h = 32, style, testId }: { children: ReactNode; h?: number; style?: CSSProperties; testId?: string }) {
  return (
    <div className="design-row design-d-row" style={{ height: h, minHeight: h, ...style }} data-testid={testId}>
      {children}
    </div>
  )
}

export function Space({ h = 8 }: { h?: number }) {
  return <div style={{ height: h }} />
}

type Variable = { id: string; name: string; resolvedType: string }

/** The local variables (`query('variables')`). */
export function useVariables(): Variable[] {
  return useQuery<{ variables: Variable[] }>('variables')?.variables ?? []
}

export type BindableProps = Omit<NumberFieldProps, 'onCommit' | 'scrub' | 'addon'> & {
  ids: readonly number[]
  nodes: readonly NodeJson[]
  /** The bound-variable key (`width`, `opacity`, `paddingLeft`, `layoutGrids/0/size`). */
  field: string
  /** The variable type the field takes. */
  kind?: 'number' | 'boolean' | 'string' | 'color'
  onCommit: (edit: NumberEdit) => void
  scrub?: ScrubHandlers
}

/**
 * A number field that can be bound to a variable: while hovered an `Apply variable` button opens
 * the matching variables; bound, the field is a chip with the variable's name and a detach button.
 */
export function BindableField({ ids, nodes, field, kind = 'number', ...props }: BindableProps) {
  const { run, t } = useDesignKit()
  const variables = useVariables()
  const bound = common(nodes, (n) => n.boundVariables?.[field] ?? null)
  const name = bound ? (variables.find((v) => v.id === bound)?.name ?? bound) : null
  const pick = (event: { currentTarget: Element }) => openVariables(ids, field, kind, event.currentTarget)
  if (bound && name) {
    return (
      <div className="design-field design-d-bound" style={{ width: props.width }} data-testid={props.testId ? `${props.testId}-bound` : undefined} data-tip={name}>
        <span className="design-d-bound-lead">{props.icon ? <Icon name={props.icon} size={14} /> : props.label}</span>
        <button type="button" className="design-d-chip" onClick={pick} aria-label={props.tip ?? props.label}>
          {name.split('/').pop()}
        </button>
        <button type="button" className="design-d-detach" data-tip={t('Detach variable')} aria-label={t('Detach variable')} onClick={() => void run('variable.bind', { ids, field, variable: null })}>
          <Icon name="Detach" size={12} />
        </button>
      </div>
    )
  }
  return (
    <NumberField
      {...props}
      addon={
        <button type="button" className="design-field-addon" data-tip={t('Apply variable')} aria-label={t('Apply variable')} onClick={pick}>
          <Icon name="Variable" size={12} />
        </button>
      }
    />
  )
}

/**
 * Opens the variable picker (the left panel package's, UI request `variablePicker.open`) under
 * `anchor`, binding field `field` of layers `ids` to a variable of `kind`.
 */
export function openVariables(ids: readonly number[], field: string, kind: string, anchor: Element) {
  const rect = anchor.closest('.design-field')?.getBoundingClientRect() ?? anchor.getBoundingClientRect()
  dispatchUiRequests([{ kind: 'variablePicker.open', data: { target: { bind: { ids: [...ids], field } }, kind, x: rect.left, y: rect.bottom } }])
}

/** A quiet button the size of a field ("+ Max W"), offering to add what is missing. */
export function AddSlot({ label, tip, onClick, testId }: { label: string; tip: string; onClick: () => void; testId?: string }) {
  return (
    <button type="button" className="design-d-add-slot" style={{ width: COL }} data-tip={tip} aria-label={tip} onClick={onClick} data-testid={testId}>
      <Icon name="Plus" size={10} />
      <span>{label}</span>
    </button>
  )
}

/** A full-width outlined button in a 40 tall row (`wide_button`). */
export function WideButton({ label, onClick, testId, disabled }: { label: string; onClick: () => void; testId?: string; disabled?: boolean }) {
  return (
    <div className="design-row" style={{ height: 40 }}>
      <button type="button" className="design-d-wide" onClick={onClick} data-testid={testId} disabled={disabled}>
        {label}
      </button>
    </div>
  )
}

/** A dropdown-looking button that runs an action (`dropdown(...).clicked()`): Combine as variants, Add variant, Open variables. */
export function ActionField({ label, icon, width, onClick, testId }: { label: string; icon?: IconName; width: CSSProperties['width']; onClick: () => void; testId?: string }) {
  return (
    <button type="button" className="design-dropdown" style={{ width }} onClick={onClick} data-testid={testId}>
      {icon ? (
        <span className="design-dropdown-lead">
          <Icon name={icon} size={14} />
        </span>
      ) : null}
      <span className="design-dropdown-label">{label}</span>
      <Icon name="Chevron" size={12} className="design-chevron" />
    </button>
  )
}

/** A light menu opened from an anchor element, while `open`. */
export function useMenu() {
  const anchor = useRef<HTMLElement | null>(null)
  const [open, setOpen] = useState(false)
  return {
    ref: (el: HTMLElement | null) => {
      anchor.current = el
    },
    open,
    toggle: () => setOpen((now) => !now),
    show: (items: MenuEntry[], minWidth?: number, testId?: string) =>
      open ? <Menu tone="light" items={items} anchor={anchor.current} placement="below-start" gap={0} minWidth={minWidth ?? anchor.current?.offsetWidth} onClose={() => setOpen(false)} testId={testId} /> : null,
  }
}

/** A heading row inside a light menu (frame preset categories, component pages). */
export function menuHeading(text: string): MenuEntry {
  return { type: 'custom', render: () => <div className="design-d-menu-heading">{text}</div> }
}

/**
 * A dropdown whose button shows `label` (which may differ from the menu's wording, as "Fixed gap"
 * for "Fixed") and whose menu is `items`.
 */
export function MenuField({ label, items, width, icon, mixed, disabled, menuWidth, ariaLabel, testId, tip }: { label: string; items: MenuEntry[]; width: CSSProperties['width']; icon?: IconName; mixed?: boolean; disabled?: boolean; menuWidth?: number; ariaLabel?: string; testId?: string; tip?: string }) {
  const menu = useMenu()
  return (
    <>
      <button ref={menu.ref} type="button" className="design-dropdown" style={{ width }} disabled={disabled} aria-haspopup="menu" aria-expanded={menu.open} aria-label={ariaLabel} data-tip={tip} onClick={menu.toggle} data-testid={testId}>
        {icon ? (
          <span className="design-dropdown-lead">
            <Icon name={icon} size={14} />
          </span>
        ) : null}
        <span className="design-dropdown-label" data-mixed={mixed || undefined}>
          {label}
        </span>
        <Icon name="Chevron" size={12} className="design-chevron" />
      </button>
      {menu.show(items, menuWidth, testId ? `${testId}-menu` : undefined)}
    </>
  )
}
